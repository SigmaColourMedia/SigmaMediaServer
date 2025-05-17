use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{body::Incoming as IncomingBody, Request, Response};

use sdp::SDPResolver;

use crate::actors::{get_event_bus, MessageEvent};
use crate::api::HTTPError;
use crate::api::routes::error::error_route;
use crate::api::routes::{HTTPResponse, RouteResult};
use crate::config::get_global_config;

pub async fn whip_post(req: Request<IncomingBody>, sdp_resolver: Arc<SDPResolver>) -> RouteResult {
    let res = whip_resolver(req, sdp_resolver).await;
    match res {
        Ok(res) => Ok(res),
        Err(err) => error_route(err).await,
    }
}

async fn whip_resolver(
    req: Request<IncomingBody>,
    sdp_resolver: Arc<SDPResolver>,
) -> Result<HTTPResponse, HTTPError> {
    let bearer_token = req.headers().get("Authorization");
    let is_authorized = bearer_token
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.split_once("Bearer "))
        .is_some_and(|(_, key)| key == get_global_config().tcp_server_config.whip_token);

    if !is_authorized {
        return Err(HTTPError::NotAuthorized);
    }

    let negotiated_session = Limited::new(req.into_body(), 15000)
        .collect()
        .await
        .or(Err(HTTPError::BadRequest))
        .map(|body| body.to_bytes())
        .and_then(|data| String::from_utf8(data.to_vec()).or(Err(HTTPError::BadRequest)))
        .and_then(|data| {
            sdp_resolver
                .accept_stream_offer(&data)
                .or(Err(HTTPError::BadRequest))
        })?;

    let sdp = negotiated_session.sdp_answer.clone();

    get_event_bus()
        .send(MessageEvent::InitStreamer(negotiated_session))
        .unwrap();

    Ok(Response::builder()
        .status(201)
        .header("content-type", "application/sdp")
        .header(
            "location",
            format!("{}/whip", get_global_config().tcp_server_config.address),
        )
        .body(Full::new(Bytes::from(String::from(sdp))))
        .unwrap())
}
