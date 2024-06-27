use std::future::IntoFuture;

use crate::GLOBAL_CONFIG;
use crate::http::{HttpError, HTTPMethod, Request, Response, SessionCommand};
use crate::http::parsers::map_http_err_to_response;
use crate::http::response_builder::ResponseBuilder;

pub async fn rooms_route(request: Request) -> Response {
    match &request.method {
        HTTPMethod::GET => get_handle(request)
            .await
            .unwrap_or_else(map_http_err_to_response),
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}

async fn get_handle(request: Request) -> Result<Response, HttpError> {
    let (tx, mut rx) = tokio::sync::oneshot::channel::<Vec<String>>();
    let config = GLOBAL_CONFIG.get().unwrap();

    config
        .session_command_sender
        .send(SessionCommand::GetRooms(tx))
        .await
        .unwrap();
    let rooms = rx.into_future().await.unwrap();

    // todo add JSON parsers
    let rooms_string = rooms
        .into_iter()
        .map(|room_id| format!("\"{}\"", room_id))
        .collect::<Vec<String>>()
        .join(",");
    let body = format!("{{\"rooms\":[{}]}}", rooms_string);

    //todo Remove this funny business
    let request_origin = request.headers.get("origin").ok_or(HttpError::BadRequest)?;
    let cors_allowed_origin = match request_origin.as_str() {
        "http://localhost:9000" => "http://localhost:9000",
        _ => "https://nynon.work",
    };

    Ok(ResponseBuilder::new()
        .set_status(200)
        .set_header("content-type", "application/json")
        .set_header("Access-Control-Allow-Methods", "GET")
        .set_header("Access-Control-Allow-Origin", cors_allowed_origin)
        .set_body(body.as_bytes())
        .build())
}
