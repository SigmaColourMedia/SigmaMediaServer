use futures::TryFutureExt;

use crate::http::{HttpError, HTTPMethod, Request, Response, SessionCommand};
use crate::http::parsers::map_http_err_to_response;
use crate::http::response_builder::ResponseBuilder;
use crate::http::server_builder::Context;
use crate::ice_registry::Session;
use crate::sdp::{create_streaming_sdp_answer, SDP};

pub async fn whep_route(request: Request, context: Context) -> Response {
    match &request.method {
        HTTPMethod::GET => register_viewer(request, context)
            .await
            .unwrap_or_else(map_http_err_to_response),
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}

async fn register_viewer(request: Request, context: Context) -> Result<Response, HttpError> {
    let target_id = request
        .search
        .get("target_id")
        .ok_or(HttpError::BadRequest)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<SDP>>();

    context
        .sender
        .send(SessionCommand::GetStreamSDP((tx, target_id.clone())))
        .await
        .unwrap();

    let stream_sdp = rx.await.unwrap().ok_or(HttpError::NotFound)?;
    let (sdp_answer, credentials) = create_streaming_sdp_answer(&stream_sdp, &context.fingerprint)
        .ok_or(HttpError::BadRequest)?;

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

    context
        .sender
        .send(SessionCommand::AddViewer(viewer_session))
        .await
        .unwrap();

    Ok(response)
}
