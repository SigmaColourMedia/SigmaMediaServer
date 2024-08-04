use std::sync::mpsc::{channel, Sender};

use crate::config::get_global_config;
use crate::http::{HttpError, HTTPMethod, Request, Response, ServerCommand};
use crate::http::parsers::map_http_err_to_response;
use crate::http::response_builder::ResponseBuilder;

pub fn whip_route(request: Request, command_sender: Sender<ServerCommand>) -> Response {
    match &request.method {
        HTTPMethod::POST => {
            post_handle(request, command_sender).unwrap_or_else(map_http_err_to_response)
        }
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}

fn post_handle(
    request: Request,
    command_sender: Sender<ServerCommand>,
) -> Result<Response, HttpError> {
    let config = get_global_config();

    let bearer_token = request
        .headers
        .get("authorization")
        .ok_or(HttpError::Unauthorized)?;

    if !bearer_token.eq(&format!("Bearer {}", config.tcp_server_config.whip_token)) {
        return Err(HttpError::Unauthorized);
    }

    let sdp_offer = request
        .body
        .and_then(|body| String::from_utf8(body).ok())
        .ok_or(HttpError::BadRequest)?;

    let (tx, rx) = channel::<Option<String>>();

    command_sender
        .send(ServerCommand::AddStreamer(sdp_offer, tx))
        .expect("SessionCommand channel should remain open");

    let sdp_answer = rx
        .recv()
        .expect("SessionCommand channel should remain open")
        .ok_or(HttpError::NotFound)?;

    Ok(ResponseBuilder::new()
        .set_status(201)
        .set_header("content-type", "application/sdp")
        .set_header("location", "http://localhost:8080/whip")
        .set_body(sdp_answer.as_bytes())
        .build())
}

// fn post_handle(
//     request: Request,
//     command_sender: Sender<ServerCommand>,
// ) -> Result<Response, HttpError> {
//     let config = get_global_config();
//
//     let bearer_token = request
//         .headers
//         .get("authorization")
//         .ok_or(HttpError::Unauthorized)?;
//
//     if !bearer_token.eq(&format!("Bearer {}", config.tcp_server_config.whip_token)) {
//         return Err(HttpError::Unauthorized);
//     }
//
//     let sdp = request
//         .body
//         .and_then(|body| String::from_utf8(body).ok())
//         .and_then(parse_sdp)
//         .ok_or(HttpError::BadRequest)?;
//     let host_username = get_random_string(4);
//     let host_password = get_random_string(24);
//     let session_credentials = SessionCredentials {
//         host_username,
//         host_password,
//     };
//     let answer = create_sdp_receive_answer(&sdp, &session_credentials);
//     let session = Session::new_streamer(session_credentials, sdp);
//
//     command_sender
//         .send(ServerCommand::AddStreamer(session))
//         .or(Err(HttpError::InternalServerError))?;
//
//     Ok(ResponseBuilder::new()
//         .set_status(201)
//         .set_header("content-type", "application/sdp")
//         .set_header("location", "http://localhost:8080/whip")
//         .set_body(answer.as_bytes())
//         .build())
// }
