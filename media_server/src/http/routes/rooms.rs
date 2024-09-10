use std::sync::mpsc::Sender;

use crate::config::get_global_config;
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
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    command_sender.send(ServerCommand::GetRooms(tx)).unwrap();
    let rooms = rx.recv().unwrap();
    let cors_origin = &get_global_config().frontend_url;

    Ok(ResponseBuilder::new()
        .set_status(200)
        .set_header("content-type", "application/json")
        .set_header("Access-Control-Allow-Methods", "GET")
        .set_header("Access-Control-Allow-Origin", cors_origin)
        .set_body(rooms.as_bytes())
        .build())
}
