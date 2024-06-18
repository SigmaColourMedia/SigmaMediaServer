use crate::http::parsers::map_http_err_to_response;
use crate::http::response_builder::ResponseBuilder;
use crate::http::{HTTPMethod, HttpError, Request, SessionCommand};
use crate::ice_registry::{Session, SessionCredentials};
use crate::rnd::get_random_string;
use crate::sdp::{create_sdp_receive_answer, parse_sdp};
use crate::WHIP_TOKEN;

use crate::http::server_builder::{Context, ServerContext};
use tokio::sync::mpsc::Sender;

pub async fn whip_route(request: Request, ctx: Context) -> String {
    match &request.method {
        HTTPMethod::POST => post_handle(request, &ctx.fingerprint, ctx.sender.clone())
            .await
            .unwrap_or_else(map_http_err_to_response),
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}

async fn post_handle(
    request: Request,
    fingerprint: &str,
    sender: Sender<SessionCommand>,
) -> Result<String, HttpError> {
    let bearer_token = request
        .headers
        .get("authorization")
        .ok_or(HttpError::Unauthorized)?;

    if !bearer_token.eq(&format!("Bearer {}", WHIP_TOKEN)) {
        return Err(HttpError::Unauthorized);
    }

    let sdp = request
        .body
        .and_then(parse_sdp)
        .ok_or(HttpError::BadRequest)?;
    let host_username = get_random_string(4);
    let host_password = get_random_string(24);
    let session_credentials = SessionCredentials {
        host_username,
        host_password,
    };
    let answer = create_sdp_receive_answer(&sdp, &session_credentials, fingerprint);
    let session = Session::new_streamer(session_credentials, sdp);

    sender
        .send(SessionCommand::AddStreamer(session))
        .await
        .or(Err(HttpError::InternalServerError))?;

    Ok(ResponseBuilder::new()
        .set_status(201)
        .set_header("content-type", "application/sdp")
        .set_header("location", "http://localhost:8080/whip")
        .set_body(answer)
        .build())
}
