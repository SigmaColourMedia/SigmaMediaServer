use std::fmt::{Display, Formatter};

use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{channel, Sender};

use crate::ice_registry::{Session, SessionCredentials};
use crate::rnd::get_random_string;
use crate::sdp::{create_sdp_receive_answer, create_streaming_sdp_answer, parse_sdp, SDP};

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
                    "/whip" => match &req.method {
                        HTTPMethod::POST => {
                            if let Err(e) = self.register_streamer(&mut stream, req.body).await {
                                eprint!("Error writing a HTTP response {}", e)
                            }
                        }
                        HTTPMethod::OPTIONS => {
                            if let Err(e) = write_cors_ok(&mut stream).await {
                                eprint!("Error writing a HTTP response {}", e)
                            }
                        }
                        _ => {
                            if let Err(err) = write_405_response(&mut stream).await {
                                eprint!("Error writing a HTTP response {}", err)
                            }
                        }
                    },
                    "/whep" => match &req.method {
                        HTTPMethod::POST => {
                            if let Err(err) = self
                                .register_viewer(&mut stream, req.body, req.search)
                                .await
                            {
                                eprint!("Error writing a HTTP response {}", err)
                            }
                        }
                        _ => {
                            if let Err(err) = write_405_response(&mut stream).await {
                                eprint!("Error writing a HTTP response {}", err)
                            }
                        }
                    },
                    "/rooms" => match &req.method {
                        HTTPMethod::GET => {
                            if let Err(err) = self.get_rooms(&mut stream).await {
                                eprint!("Error writing a HTTP response {}", err)
                            }
                        }
                        _ => {
                            if let Err(e) = write_webpage(&mut stream).await {
                                eprint!("Error writing a HTTP response {}", e)
                            }
                        }
                    },
                    "/" => match &req.method {
                        HTTPMethod::GET => {
                            if let Err(e) = write_webpage(&mut stream).await {
                                eprint!("Error writing a HTTP response {}", e)
                            }
                        }
                        _ => {
                            if let Err(err) = write_405_response(&mut stream).await {
                                eprint!("Error writing a HTTP response {}", err)
                            }
                        }
                    },
                    _ => {
                        if let Err(err) = write_404_response(&mut stream).await {
                            eprint!("Error writing a HTTP response {}", err)
                        }
                    }
                };
            }
            None => {
                println!("not a http rquest");
                if let Err(err) = write_400_response(&mut stream).await {
                    eprint!("Error writing a HTTP response {}", err)
                }
            }
        }
    }

    async fn register_streamer(
        &self,
        stream: &mut TcpStream,
        body: Option<String>,
    ) -> Result<(), HttpError> {
        let sdp = body
            .and_then(parse_sdp)
            .ok_or(HttpError::MalformedRequest)?;
        let host_username = get_random_string(4);
        let host_password = get_random_string(24);
        let session_credentials = SessionCredentials {
            remote_username: sdp.ice_username.clone(),
            host_username,
            host_password,
        };

        let answer = create_sdp_receive_answer(&sdp, &session_credentials, &self.fingerprint);
        let session = Session::new_streamer(session_credentials, sdp);

        let response = format!(
            "HTTP/1.1 201 CREATED\r\n\
        content-length:{content_length}\r\n\
        content-type:application/sdp\r\n\
        location: http://localhost:8080/whip?id={resource_id}\r\n\
        access-control-allow-origin:*\r\n\
        access-control-allow-methods:OPTIONS,POST,GET\r\n\
        access-control-max-age:2592000\r\n\
        access-control-allow-headers:*\r\n\r\n\
        {answer}",
            content_length = answer.len(),
            resource_id = &session.id
        );

        stream
            .write_all(response.as_bytes())
            .await
            .or(Err(HttpError::InternalServerError))?;

        self.session_commands_sender
            .send(SessionCommand::AddStreamer(session))
            .await
            .or(Err(HttpError::InternalServerError))?;

        Ok(())
    }
    async fn register_viewer(
        &self,
        stream: &mut TcpStream,
        body: Option<String>,
        search: Option<String>,
    ) -> Result<(), HttpError> {
        let body = body.ok_or(HttpError::MalformedRequest)?;
        let search = search.ok_or(HttpError::MalformedRequest)?;

        let target_id = search
            .split("&")
            .find(|param| param.starts_with("target_id="))
            .and_then(|param| param.split_once("="))
            .map(|(_, value)| value.to_owned())
            .ok_or(HttpError::MalformedRequest)?;

        let (tx, mut rx) = tokio::sync::oneshot::channel::<Option<SDP>>();

        self.session_commands_sender
            .send(SessionCommand::GetStreamSDP((tx, target_id.clone())))
            .await
            .unwrap();

        let stream_sdp = rx.await.unwrap().ok_or(HttpError::NotFound)?;
        let (sdp_answer, credentials) =
            create_streaming_sdp_answer(&stream_sdp, &body, &self.fingerprint)
                .ok_or(HttpError::MalformedRequest)?;

        let viewer_session = Session::new_viewer(target_id.to_owned(), credentials);
        let response = format!(
            "HTTP/1.1 201 CREATED\r\n\
        content-type: application/sdp\r\n\
        content-length:{content_length}\r\n\
        location:http://localhost:8080/whep?id={viewer_id}\r\n\r\n\
        {payload}",
            content_length = sdp_answer.len(),
            viewer_id = &viewer_session.id,
            payload = sdp_answer
        );

        self.session_commands_sender
            .send(SessionCommand::AddViewer(viewer_session))
            .await
            .unwrap();

        stream
            .write_all(response.as_bytes())
            .await
            .map_err(|_| HttpError::InternalServerError)?;

        Ok(())
    }

    async fn get_rooms(&self, stream: &mut TcpStream) -> Result<(), HttpError> {
        let (tx, mut rx) = channel::<Vec<String>>(1000);
        self.session_commands_sender
            .send(SessionCommand::GetRooms(tx))
            .await
            .unwrap();

        let rooms = rx.recv().await.unwrap();

        let rooms_string = rooms
            .into_iter()
            .map(|room_id| format!("\"{}\"", room_id))
            .collect::<Vec<String>>()
            .join(",");
        let body = format!("{{\"rooms\":[{}]}}", rooms_string);

        let response = format!(
            "HTTP/1.1 200 OK\r\n\
            content-type: application/json\r\n\
            content-length: {content_length}\r\n\r\n\
            {body}",
            content_length = body.len(),
            body = body
        );

        match stream.write_all(response.as_bytes()).await {
            Ok(_) => Ok(()),
            Err(_) => Err(HttpError::InternalServerError),
        }
    }
}

async fn write_cors_ok(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 205 OK";

    let response = format!(
        "{status_line}\r\n\
    access-control-allow-origin:*\r\n\
access-control-allow-methods:OPTIONS,POST,GET\r\n\
access-control-max-age:2592000\r\n\
access-control-allow-headers:*\r\n\r\n"
    );
    stream.write_all(response.as_bytes()).await
}

async fn write_webpage(stream: &mut TcpStream) -> std::io::Result<()> {
    let contents = fs::read_to_string("./src/index.html").await?;
    let response = format!(
        "HTTP/1.1 200 OK\r\n\
    content-length: {}\r\n\
    content-type: text/html\r\n\r\n\
    {}",
        contents.len(),
        contents
    );
    stream.write_all(response.as_bytes()).await
}

async fn write_404_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 404 NOT FOUND";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

async fn write_400_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 400 BAD REQUEST";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

async fn write_405_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 405 METHOD NOT ALLOWED";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

pub async fn parse_http_request(stream: &mut TcpStream) -> Option<Request> {
    let buf_reader = BufReader::new(stream);
    let mut lines = buf_reader.lines();
    let request_line = lines.next_line().await.ok().flatten()?;

    let req = request_line.split(" ").collect::<Vec<&str>>();
    let (method, pathname) = (req[0].to_owned(), req[1].to_owned());
    let method = match &method[..] {
        "GET" => HTTPMethod::GET,
        "POST" => HTTPMethod::POST,
        "OPTIONS" => HTTPMethod::OPTIONS,
        "DELETE" => HTTPMethod::DELETE,
        _ => {
            return None;
        }
    };

    let pathname_split = pathname.split_once("?");
    let (path, search) = match &pathname_split {
        Some((path, search)) => (path.to_string(), Some(search.to_string())),
        None => (pathname, None),
    };

    let mut headers: Vec<Header> = Vec::new();
    while let Some(line) = lines.next_line().await.ok().flatten() {
        if line.is_empty() {
            break;
        }
        headers.push(parse_header(&line)?)
    }

    let content_length = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .map(|(key, value)| value.parse::<usize>())
        .map(|result| result.ok())
        .flatten();

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
        headers,
        search,
        path,
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

#[derive(Debug)]
pub enum SessionCommand {
    AddStreamer(Session),
    AddViewer(Session),
    GetStreamSDP((tokio::sync::oneshot::Sender<Option<SDP>>, String)),
    GetRooms(Sender<Vec<String>>),
}
