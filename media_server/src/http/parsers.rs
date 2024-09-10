use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;

use crate::http::{HttpError, HTTPMethod, Request, Response};
use crate::http::response_builder::ResponseBuilder;

pub fn parse_http(stream: &mut TcpStream) -> Option<Request> {
    let mut buff_reader =
        BufReader::new(stream.try_clone().expect("Should clone TCP stream socket")).take(15000);

    let mut request_line = String::new();
    buff_reader.read_line(&mut request_line).ok()?;

    let mut request_line = request_line.split(" ");

    let method = request_line.next()?;
    let pathname = request_line.next()?;
    let method = match method {
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
        Some((path, search)) => (path.to_string(), parse_search(search)?),
        None => (pathname.to_string(), HashMap::new()),
    };

    let mut headers: HashMap<String, String> = HashMap::new();

    loop {
        let mut header_line = String::new();
        buff_reader.read_line(&mut header_line).ok()?;

        if header_line.trim().is_empty() {
            break;
        }
        let (key, value) = header_line.split_once(":")?;
        let key = key.trim().to_lowercase();
        let value = value.trim().to_string();
        headers.insert(key, value);
    }

    let content_length = headers
        .get("content-length")
        .map(|length| length.parse::<usize>().ok())
        .flatten();

    let body = content_length.and_then(|length| {
        let mut body = vec![0u8; length];
        buff_reader.read_exact(&mut body).ok()?;
        Some(body)
    });

    Some(Request {
        method,
        headers,
        search,
        body,
        path,
    })
}

fn parse_search(search: &str) -> Option<HashMap<String, String>> {
    let mut search_map = HashMap::new();
    let split_iter = search.split("&");
    for split in split_iter {
        let (key, value) = split.split_once("=")?;
        search_map.insert(key.to_string(), value.to_string());
    }

    Some(search_map)
}
pub fn map_http_err_to_response(err: HttpError) -> Response {
    let status = match err {
        HttpError::NotFound => 404,
        HttpError::Unauthorized => 401,
        HttpError::InternalServerError => 500,
        HttpError::BadRequest => 404,
        HttpError::MethodNotAllowed => 405,
    };

    ResponseBuilder::new().set_status(status).build()
}
