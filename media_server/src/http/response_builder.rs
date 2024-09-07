use std::collections::HashMap;

use crate::config::get_global_config;
use crate::http::Response;

pub struct ResponseBuilder {
    status: Option<usize>,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
}

impl ResponseBuilder {
    pub fn new() -> Self {
        ResponseBuilder {
            body: None,
            status: None,
            headers: HashMap::new(),
        }
    }

    pub fn set_status(mut self, status: usize) -> Self {
        self.status = Some(status);
        self
    }

    pub fn set_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn set_body(mut self, body: &[u8]) -> Self {
        self.body = Some(Vec::from(body));
        self
    }

    pub fn build(mut self) -> Response {
        let status = self.status.expect("No status provided for response");

        let status_text = match status {
            200 => "OK",
            201 => "CREATED",
            400 => "BAD REQUEST",
            401 => "UNAUTHORIZED",
            404 => "NOT FOUND",
            405 => "METHOD NOT ALLOWED",
            _ => "",
        };

        let mut response = format!("HTTP/1.1 {status} {status_text}\r\n");

        let concat_headers = |headers: HashMap<String, String>| {
            headers
                .into_iter()
                .map(|(key, value)| format!("{}: {}\r\n", key, value))
                .collect::<String>()
        };

        self.headers.insert(
            "Access-Control-Allow-Origin".to_string(),
            get_global_config().frontend_url.clone(),
        );

        match self.body {
            None => {
                let headers = concat_headers(self.headers);
                response.push_str(&headers);
                response.push_str("\r\n");

                Response {
                    status,
                    _inner: response.into_bytes(),
                }
            }
            Some(mut body) => {
                self.headers
                    .insert("content-length".to_string(), body.len().to_string());
                let headers = concat_headers(self.headers);
                response.push_str(&headers);
                response.push_str("\r\n");

                let mut response_bytes = response.into_bytes();
                response_bytes.append(&mut body);

                Response {
                    status,
                    _inner: response_bytes,
                }
            }
        }
    }
}
