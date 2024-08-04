use std::future::IntoFuture;
use std::sync::mpsc::Sender;

use crate::http::{HttpError, HTTPMethod, Request, Response, ServerCommand};
use crate::http::parsers::map_http_err_to_response;
use crate::http::response_builder::ResponseBuilder;

pub fn rooms_route(request: Request, command_sender: Sender<ServerCommand>) -> Response {
    match &request.method {
        HTTPMethod::GET => {
            get_handle(request, command_sender).unwrap_or_else(map_http_err_to_response)
        }
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}

fn get_handle(
    request: Request,
    command_sender: Sender<ServerCommand>,
) -> Result<Response, HttpError> {
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u32>>();
    command_sender.send(ServerCommand::GetRooms(tx)).unwrap();
    let rooms = rx.recv().unwrap();

    // todo add JSON parsers
    let rooms_string = rooms
        .into_iter()
        .map(|room_id| format!("\"{}\"", room_id))
        .collect::<Vec<String>>()
        .join(",");
    let body = format!("{{\"rooms\":[{}]}}", rooms_string);

    //todo Remove this funny business
    // let request_origin = request.headers.get("origin").ok_or(HttpError::BadRequest)?;
    // let cors_allowed_origin = match request_origin.as_str() {
    //     "http://localhost:9000" => "http://localhost:9000",
    //     _ => "https://nynon.work",
    // };

    Ok(ResponseBuilder::new()
        .set_status(200)
        .set_header("content-type", "application/json")
        .set_header("Access-Control-Allow-Methods", "GET")
        .set_header("Access-Control-Allow-Origin", "http://localhost:9000")
        .set_body(body.as_bytes())
        .build())
}
