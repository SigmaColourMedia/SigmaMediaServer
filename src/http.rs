use std::io::Error;
use std::num::ParseIntError;
use std::string::FromUtf8Error;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::net::TcpStream;

pub async fn handle_http_request(mut stream: TcpStream) {
    match parse_http_request(&mut stream).await {
        Ok(body) => {
            println!("{}", body)
        }
        Err(_) => {}
    };
}

pub async fn parse_http_request(stream: &mut TcpStream) -> Result<String, HttpError> {
    let buf_reader = BufReader::new(stream);
    let mut lines = buf_reader.lines();

    let request_line = lines.next_line().await.ok().flatten().ok_or(HttpError::MalformedRequest)?;

    match &request_line[..] {
        "POST /whip HTTP/1.1" => {
            let mut headers: Vec<Header> = Vec::new();
            while let Some(line) = lines.next_line().await.ok().flatten() {
                if line.is_empty() {
                    break;
                }
                headers.push(parse_header(&line)?)
            }

            let (_, content_length) = headers.iter().find(|(key, _)| key.eq_ignore_ascii_case("content-length")).ok_or(HttpError::MalformedRequest)?;
            let content_length = content_length.parse::<usize>()?;
            let mut body = vec![0; content_length];
            lines.get_mut().read_exact(&mut body).await?;
            let body = String::from_utf8(body)?;


            Ok(body)
        }
        _ => Err(HttpError::NotFound)
    }
}

struct Request {
    path: String,
    search: Option<String>,
    method: HTTPMethod,
    body: Option<String>,
}

enum HTTPMethod {
    GET,
    POST,
    OPTIONS,
    DELETE,
}

type Header = (String, String);

fn parse_header(header: &str) -> Result<Header, HttpError> {
    let (key, value) = header.split_once(":").ok_or(HttpError::BadRequest)?;
    let key = key.trim();
    let value = value.trim();

    Ok((key.to_owned(), value.to_owned()))
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