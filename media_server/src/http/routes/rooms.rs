use crate::http::parsers::map_http_err_to_response;
use crate::http::response_builder::ResponseBuilder;
use crate::http::{HTTPMethod, HttpError, Request, SessionCommand};
use std::future::IntoFuture;
use tokio::sync::mpsc::Sender;

pub async fn rooms(request: Request, _fingerprint: &str, sender: Sender<SessionCommand>) -> String {
    println!("{}", request);
    match &request.method {
        HTTPMethod::GET => get_handle(request, sender)
            .await
            .unwrap_or_else(map_http_err_to_response),
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}

async fn get_handle(request: Request, sender: Sender<SessionCommand>) -> Result<String, HttpError> {
    let (tx, mut rx) = tokio::sync::oneshot::channel::<Vec<String>>();
    sender.send(SessionCommand::GetRooms(tx)).await.unwrap();
    let rooms = rx.into_future().await.unwrap();

    // todo add JSON parsers
    let rooms_string = rooms
        .into_iter()
        .map(|room_id| format!("\"{}\"", room_id))
        .collect::<Vec<String>>()
        .join(",");
    let body = format!("{{\"rooms\":[{}]}}", rooms_string);

    // //todo Remove this funny business
    // let request_origin = request.headers.get("origin").ok_or(HttpError::BadRequest)?;
    // let cors_allowed_origin = match request_origin.as_str() {
    //     "http://localhost:9000" => "http://localhost:9000",
    //     _ => "https://nynon.work",
    // };

    Ok(ResponseBuilder::new()
        .set_status(200)
        .set_header("content-type", "application/json")
        .set_header("Access-Control-Allow-Methods", "GET")
        .set_header("Access-Control-Allow-Origin", "https://nynon.work")
        .set_body(body)
        .build())
}