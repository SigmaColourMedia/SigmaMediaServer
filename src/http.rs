use std::fmt::{Display, Formatter};
use std::io::ErrorKind;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;

use crate::ice_registry::Session;

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
                                if let Err(e) = self.register_streamer(&mut stream, req.body) {
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

    fn register_streamer(&self, stream: &mut TcpStream, body: Option<String>) -> std::io::Result<()> {
        let body = body.ok_or(ErrorKind::Other)?;
        let sdp = parse_sdp(body);
        println!("sdp {:?}", sdp);

        Ok(())
    }
}

const ICE_USERNAME_ATTRIBUTE_PREFIX: &str = "a=ice-ufrag:";
const ICE_PASSWORD_ATTRIBUTE_PREFIX: &str = "a=ice-pwd:";
const GROUP_ATTRIBUTE_PREFIX: &str = "a=group:";
const MEDIA_LINE_PREFIX: &str = "m=";

const WHITELISTED_ATTRIBUTES: [&str; 8] = ["m=", "a=ssrc", "a=msid", "a=rtcp-mux", "a=rtpmap", "a=fmtp", "a=mid", "a=rtcp"];

fn parse_sdp(data: String) -> Option<SDP> {
    let mut lines = data.lines();
    let remote_username = lines.clone().find(|line| line.starts_with(ICE_USERNAME_ATTRIBUTE_PREFIX)).and_then(|line| line.split_once(":").map(|line| line.1))?.to_owned();
    let remote_password = lines.clone().find(|line| line.starts_with(ICE_PASSWORD_ATTRIBUTE_PREFIX)).and_then(|line| line.split_once(":").map(|line| line.1))?.to_owned();
    let bundle = lines.clone().find(|line| line.starts_with(GROUP_ATTRIBUTE_PREFIX)).and_then(|line| line.split_once(":").map(|line| line.1))?.to_owned();

    let mut media_lines = lines.skip_while(|line| !line.starts_with(MEDIA_LINE_PREFIX)).filter(|line| WHITELISTED_ATTRIBUTES.iter().any(|item| line.starts_with(item)));

    let mut media_descriptors = vec![vec![]];
    let mut media_index = 0;
    media_descriptors[media_index].push(media_lines.next()?);
    while let Some(line) = media_lines.next() {
        if line.starts_with("m=") {
            media_index += 1;
            media_descriptors.push(vec![])
        }
        media_descriptors[media_index].push(line)
    }

    let media_descriptors = media_descriptors.into_iter().map(|descriptor| {
        let mut iterator = descriptor.into_iter();
        let media_attribute: Vec<&str> = iterator.next()?.splitn(4, " ").collect();
        let media_type = media_attribute[0].split_once("=")?.1.to_owned();

        let attributes = iterator.map(|str| str.to_owned()).collect::<Vec<String>>();
        Some(MediaDescription {
            media_type,
            protocol: media_attribute[2].to_owned(),
            format: media_attribute[3].to_owned(),
            attributes,
        })
    }).collect::<Option<Vec<MediaDescription>>>()?;


    Some(SDP {
        ice_pwd: remote_password,
        ice_username: remote_username,
        group: bundle,
        media_descriptions: media_descriptors,
    })
}

#[derive(Debug)]
struct SDP {
    ice_username: String,
    ice_pwd: String,
    group: String,
    media_descriptions: Vec<MediaDescription>,
}

#[derive(Debug)]
struct MediaDescription {
    media_type: String,
    protocol: String,
    format: String,
    attributes: Vec<String>,
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