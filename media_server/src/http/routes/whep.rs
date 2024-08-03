use std::sync::mpsc::Sender;

use crate::http::{HttpError, HTTPMethod, Request, Response, ServerCommand};
use crate::http::parsers::map_http_err_to_response;
use crate::http::response_builder::ResponseBuilder;

pub fn whep_route(request: Request, command_sender: Sender<ServerCommand>) -> Response {
    match &request.method {
        HTTPMethod::POST => {
            post_handler(request, command_sender).unwrap_or_else(map_http_err_to_response)
        }
        HTTPMethod::OPTIONS => options_handler(),
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}

fn options_handler() -> Response {
    ResponseBuilder::new()
        .set_status(204)
        .set_header("Access-Control-Allow-Method", "POST")
        .set_header("Access-Control-Allow-Origin", "http://localhost:9000")
        .set_header("Access-Control-Allow-Headers", "content-type")
        .build()
}

fn post_handler(
    request: Request,
    command_sender: Sender<ServerCommand>,
) -> Result<Response, HttpError> {
    let target_id = request
        .search
        .get("target_id")
        .ok_or(HttpError::BadRequest)?
        .to_string();

    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();

    let body = request
        .body
        .and_then(|body| String::from_utf8(body).ok())
        .ok_or(HttpError::BadRequest)?;

    command_sender
        .send(ServerCommand::AddViewer(body, target_id, tx))
        .expect("Session Command channel should remain open");

    // todo Handle unsupported codecs
    let sdp_answer = rx.recv().unwrap().ok_or(HttpError::BadRequest)?;

    println!("answer {}", sdp_answer);

    let response_builder = ResponseBuilder::new();
    let response = response_builder
        .set_status(200)
        .set_header("content-type", "application/sdp")
        .set_header("Access-Control-Allow-Method", "POST")
        .set_header("Access-Control-Allow-Origin", "http://localhost:9000")
        .set_header("location", "http://localhost:8080/whep")
        .set_body(sdp_answer.as_bytes())
        .build();

    Ok(response)
}
