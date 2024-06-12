use crate::ice_registry::Session;
use crate::sdp::SDP;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use tokio::sync::mpsc::Sender;

pub mod parsers;
pub mod response_builder;
pub mod router;
pub mod routes;

#[derive(Debug)]
pub struct Request {
    pub path: String,
    pub method: HTTPMethod,
    pub search: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
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

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
pub enum SessionCommand {
    AddStreamer(Session),
    AddViewer(Session),
    GetStreamSDP((tokio::sync::oneshot::Sender<Option<SDP>>, String)),
    GetRooms(tokio::sync::oneshot::Sender<Vec<String>>),
}
