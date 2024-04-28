use std::fmt::{Display, Formatter};
use std::io::Error;
use std::num::ParseIntError;
use std::string::FromUtf8Error;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::net::TcpStream;

pub async fn handle_http_request(mut stream: TcpStream) {
    match parse_http_request(&mut stream).await {
        Some(req) => {
            println!("{}", req)
        }
        None => {}
    };
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

impl From<FromUtf8Error> for HttpError {
    fn from(value: FromUtf8Error) -> Self {
        HttpError::MalformedRequest
    }
}

impl From<std::io::Error> for HttpError {
    fn from(value: Error) -> Self {
        HttpError::MalformedRequest
    }
}

impl From<ParseIntError> for HttpError {
    fn from(value: ParseIntError) -> Self {
        HttpError::MalformedRequest
    }
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