use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::sync::mpsc::Sender;

pub mod parsers;
pub mod response_builder;
pub mod routes;
pub mod server_builder;

#[derive(Debug)]
pub struct Request {
    pub path: String,
    pub method: HTTPMethod,
    pub search: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

impl Display for Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = format!("{} {}", self.method, self.path);
        write!(f, "{}", &msg)
    }
}

#[derive(Debug)]
pub enum HTTPMethod {
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
pub enum HttpError {
    NotFound,
    Unauthorized,
    InternalServerError,
    BadRequest,
    MethodNotAllowed,
}

impl Display for HttpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::NotFound => write!(f, "404 Not Found"),
            HttpError::InternalServerError => write!(f, "500 Internal Server Error"),
            HttpError::BadRequest => write!(f, "400 Bad Request"),
            HttpError::MethodNotAllowed => write!(f, "405 Method Not Allowed"),
            HttpError::Unauthorized => write!(f, "401 Unauthorized"),
        }
    }
}

#[derive(Debug)]
pub enum ServerCommand {
    AddStreamer(String, Sender<Option<String>>),
    AddViewer(String, u32, Sender<Option<String>>),
    GetRooms(Sender<Vec<u32>>),
    HandlePacket(Vec<u8>, SocketAddr),
    CheckForTimeout,
}

pub struct Response {
    _inner: Vec<u8>,
    pub status: usize,
}

impl Response {
    pub fn as_bytes(&self) -> &[u8] {
        &self._inner
    }
}
