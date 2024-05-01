use std::fmt::{Display, Formatter};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;

use crate::ice_registry::{Session, SessionCredentials};
use crate::rnd::get_random_string;
use crate::sdp::parse_sdp;

pub struct HTTPServer {
    fingerprint: String,
    session_commands_sender: Sender<SessionCommand>,
}


impl HTTPServer {
    pub fn new(fingerprint: String, sender: Sender<SessionCommand>) -> Self {
        HTTPServer {
            fingerprint,
            session_commands_sender: sender,
        }
    }
    pub async fn handle_http_request(&self, mut stream: TcpStream) {
        match parse_http_request(&mut stream).await {
            Some(req) => {
                println!("got req {}", req);
                match &req.path[..] {
                    "/whip" => {
                        match &req.method {
                            HTTPMethod::POST => {
                                if let Err(e) = self.register_streamer(&mut stream, req.body).await {
                                    eprint!("Error writing a HTTP response {}", e)
                                }
                            }
                            _ => {
                                if let Err(err) = write_405_response(&mut stream).await {
                                    eprint!("Error writing a HTTP response {}", err)
                                }
                            }
                        }
                    }
                    _ => {
                        if let Err(err) = write_404_response(&mut stream).await {
                            eprint!("Error writing a HTTP response {}", err)
                        }
                    }
                };
            }
            None => {
                if let Err(err) = write_400_response(&mut stream).await {
                    eprint!("Error writing a HTTP response {}", err)
                }
            }
        }
    }

    async fn register_streamer(&self, stream: &mut TcpStream, body: Option<String>) -> Result<(), HttpError> {
        let sdp = body.and_then(parse_sdp).ok_or(HttpError::MalformedRequest)?;
        let host_username = get_random_string(4);
        let host_password = get_random_string(24);
        let session_credentials = SessionCredentials {
            remote_username: sdp.ice_username.clone(),
            host_username,
            host_password,
        };

        let session = Session::new_streamer(session_credentials, sdp);

        self.session_commands_sender.send(SessionCommand::AddStreamer(session)).await.or(Err(HttpError::InternalServerError))?;


        // println!("sdp {:?}", sdp);

        Ok(())
    }
}

pub async fn handle_whip_request(request: Request, fingerprint: &str) -> Result<String, HttpError> {
    Err(HttpError::MalformedRequest)
}

pub async fn write_404_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 404 NOT FOUND";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

pub async fn write_400_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 400 BAD REQUEST";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

pub async fn write_405_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 405 METHOD NOT ALLOWED";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

pub async fn parse_http_request(stream: &mut TcpStream) -> Option<Request> {
    let buf_reader = BufReader::new(stream);
    let mut lines = buf_reader.lines();

    let request_line = lines.next_line().await.ok().flatten()?;

    let req = request_line.split(" ").collect::<Vec<&str>>();
    let (method, path) = (req[0].to_owned(), req[1].to_owned());
    let method = match &method[..] {
        "GET" => HTTPMethod::GET,
        "POST" => HTTPMethod::POST,
        "OPTIONS" => HTTPMethod::OPTIONS,
        "DELETE" => HTTPMethod::DELETE,
        _ => {
            return None;
        }
    };
    let search = path.split_once("?").map(|(_, search)| search.to_owned());


    let mut headers: Vec<Header> = Vec::new();
    while let Some(line) = lines.next_line().await.ok().flatten() {
        if line.is_empty() {
            break;
        }
        headers.push(parse_header(&line)?)
    }

    let content_length = headers.iter().find(|(key, _)| key.eq_ignore_ascii_case("content-length")).map(|(key, value)| value.parse::<usize>()).map(|result| result.ok()).flatten();

    let body: Option<String> = match content_length {
        None => None,
        Some(length) => {
            let mut body = vec![0; length];
            lines.get_mut().read_exact(&mut body).await.ok()?;
            String::from_utf8(body).ok()
        }
    };

    Some(Request {
        method,
        path,
        headers,
        search,
        body,
    })
}

type Header = (String, String);

fn parse_header(header: &str) -> Option<Header> {
    let (key, value) = header.split_once(":")?;
    let key = key.trim();
    let value = value.trim();

    Some((key.to_owned(), value.to_owned()))
}

// todo Don't hold the entire body in memory
#[derive(Debug)]
struct Request {
    path: String,
    search: Option<String>,
    method: HTTPMethod,
    headers: Vec<Header>,
    body: Option<String>,
}

impl Display for Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = format!("{} {}", self.method, self.path);
        write!(f, "{}", &msg)
    }
}


#[derive(Debug)]
enum HTTPMethod {
    GET,
    POST,
    OPTIONS,
    DELETE,
}

impl Display for HTTPMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HTTPMethod::GET => write!(f, "GET"),
            HTTPMethod::POST => write!(f, "POST"),
            HTTPMethod::OPTIONS => write!(f, "OPTIONS"),
            HTTPMethod::DELETE => write!(f, "DELETE"),
        }
    }
}


#[derive(Debug)]
enum HttpError {
    NotFound,
    InternalServerError,
    BadRequest,
    MethodNotAllowed,
    MalformedRequest,
}


impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::NotFound => write!(f, "404 Not Found"),
            HttpError::InternalServerError => write!(f, "500 Internal Server Error"),
            HttpError::BadRequest => write!(f, "400 Bad Request"),
            HttpError::MethodNotAllowed => write!(f, "405 Method Not Allowed"),
            HttpError::MalformedRequest => write!(f, "405 Malformed request"),
        }
    }
}

pub enum SessionCommand {
    AddStreamer(Session),
    AddViewer(Session),
}