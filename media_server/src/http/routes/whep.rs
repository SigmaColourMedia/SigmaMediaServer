use futures::TryFutureExt;

use crate::http::parsers::map_http_err_to_response;
use crate::http::response_builder::ResponseBuilder;
use crate::http::{HTTPMethod, HttpError, Request, Response, SessionCommand};
use crate::ice_registry::Session;
use crate::sdp::{create_streaming_sdp_answer, SDP};
use crate::{get_global_config, GLOBAL_CONFIG};

pub async fn whep_route(request: Request) -> Response {
    match &request.method {
        HTTPMethod::GET => register_viewer(request)
            .await
            .unwrap_or_else(map_http_err_to_response),
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}

async fn register_viewer(request: Request) -> Result<Response, HttpError> {
    let target_id = request
        .search
        .get("target_id")
        .ok_or(HttpError::BadRequest)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<SDP>>();

    let config = get_global_config();

    config
        .session_command_channel
        .send(SessionCommand::GetStreamSDP((tx, target_id.clone())))
        .await
        .unwrap();

    let stream_sdp = rx.await.unwrap().ok_or(HttpError::NotFound)?;
    let (sdp_answer, credentials) =
        create_streaming_sdp_answer(&stream_sdp).ok_or(HttpError::BadRequest)?;

    let viewer_session = Session::new_viewer(target_id.to_owned(), credentials);

    let response_builder = ResponseBuilder::new();
    let response = response_builder
        .set_status(200)
        .set_header("content-type", "application/sdp")
        .set_header("Access-Control-Allow-Methods", "GET")
        .set_header("Access-Control-Allow-Origin", "http://localhost:9000")
        .set_header("location", "http://localhost:8080/whep")
        .set_body(sdp_answer.as_bytes())
        .build();

    config
        .session_command_channel
        .send(SessionCommand::AddViewer(viewer_session))
        .await
        .unwrap();

    Ok(response)
}
