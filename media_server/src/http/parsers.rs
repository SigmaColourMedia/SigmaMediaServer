use crate::http::response_builder::ResponseBuilder;
use crate::http::{HTTPMethod, HttpError, Request};
use std::collections::HashMap;

pub async fn parse_http(data: &[u8]) -> Option<Request> {
    let string_data = std::str::from_utf8(data).ok()?;
    let mut lines = string_data.lines();

    let mut request_line = lines.next()?.split(" ");

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
    while let Some(line) = lines.next() {
        if line.is_empty() {
            // END OF HEADERS
            break;
        }
        let (key, value) = line.split_once(":")?;
        let key = key.trim().to_lowercase();
        let value = value.trim().to_string();
        headers.insert(key, value);
    }

    let content_length = headers.get("content-length");

    let body = content_length.and_then(|length| {
        let length = length.parse::<usize>().ok()?;
        let payload = lines.collect::<Vec<&str>>().join("\r\n").into_bytes();
        let truncated_payload = std::str::from_utf8(&payload[..length]).ok()?.to_string();
        Some(truncated_payload)
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
pub fn map_http_err_to_response(err: HttpError) -> String {
    let status = match err {
        HttpError::NotFound => 404,
        HttpError::Unauthorized => 401,
        HttpError::InternalServerError => 500,
        HttpError::BadRequest => 404,
        HttpError::MethodNotAllowed => 405,
    };

    ResponseBuilder::new().set_status(status).build()
}
