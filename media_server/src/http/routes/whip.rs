use crate::{GLOBAL_CONFIG, WHIP_TOKEN};
use crate::http::{HttpError, HTTPMethod, Request, Response, SessionCommand};
use crate::http::parsers::map_http_err_to_response;
use crate::http::response_builder::ResponseBuilder;
use crate::ice_registry::{Session, SessionCredentials};
use crate::rnd::get_random_string;
use crate::sdp::{create_sdp_receive_answer, parse_sdp};

pub async fn whip_route(request: Request) -> Response {
    match &request.method {
        HTTPMethod::POST => post_handle(request)
            .await
            .unwrap_or_else(map_http_err_to_response),
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}

async fn post_handle(request: Request) -> Result<Response, HttpError> {
    let config = GLOBAL_CONFIG.get().unwrap();

    let bearer_token = request
        .headers
        .get("authorization")
        .ok_or(HttpError::Unauthorized)?;

    if !bearer_token.eq(&format!("Bearer {}", WHIP_TOKEN)) {
        return Err(HttpError::Unauthorized);
    }

    let sdp = request
        .body
        .and_then(|body| String::from_utf8(body).ok())
        .and_then(parse_sdp)
        .ok_or(HttpError::BadRequest)?;
    let host_username = get_random_string(4);
    let host_password = get_random_string(24);
    let session_credentials = SessionCredentials {
        host_username,
        host_password,
    };
    let answer =
        create_sdp_receive_answer(&sdp, &session_credentials, &config.ssl_config.fingerprint);
    let session = Session::new_streamer(session_credentials, sdp);

    config
        .session_command_sender
        .send(SessionCommand::AddStreamer(session))
        .await
        .or(Err(HttpError::InternalServerError))?;

    Ok(ResponseBuilder::new()
        .set_status(201)
        .set_header("content-type", "application/sdp")
        .set_header("location", "http://localhost:8080/whip")
        .set_body(answer.as_bytes())
        .build())
}
