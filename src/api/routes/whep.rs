use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming as IncomingBody, Request, Response};
use uuid::Uuid;

use crate::actors::{get_main_bus, MessageEvent};
use crate::api::HTTPError;
use crate::api::routes::{HTTPResponse, RouteResult};
use crate::api::routes::error::error_route;
use crate::config::get_global_config;

pub async fn whep_post(req: Request<IncomingBody>) -> RouteResult {
    let res = whep_resolver(req).await;
    match res {
        Ok(res) => Ok(res),
        Err(err) => error_route(err).await,
    }
}

pub async fn whep_options() -> RouteResult {
    Ok(Response::builder()
        .status(200)
        .header("Access-Control-Allow-Method", "POST")
        .header("Access-Control-Allow-Headers", "content-type")
        .header(
            "Access-Control-Allow-Origin",
            &get_global_config().frontend_url,
        )
        .body(Full::new(Bytes::new()))
        .unwrap())
}

async fn whep_resolver(req: Request<IncomingBody>) -> Result<HTTPResponse, HTTPError> {
    let room_id = req
        .uri()
        .query()
        .and_then(|query| query.split("&").find(|item| item.starts_with("target_id=")))
        .and_then(|param| param.split_once("target_id="))
        .and_then(|(_, id)| Uuid::try_parse(id).ok())
        .ok_or(HTTPError::BadRequest)?;

    let sdp = req
        .into_body()
        .collect()
        .await
        .or(Err(HTTPError::BadRequest))
        .map(|item| item.to_bytes().to_vec())
        .and_then(|bytes| String::from_utf8(bytes).or(Err(HTTPError::BadRequest)))?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    get_main_bus()
        .send(MessageEvent::InitViewer(sdp, room_id, tx))
        .unwrap();
    let sdp_response = rx.await.unwrap();

    match sdp_response {
        None => Err(HTTPError::NotFound),
        Some(sdp_answer) => Ok(Response::builder()
            .status(200)
            .header("content-type", "application/sdp")
            .header(
                "Access-Control-Allow-Origin",
                &get_global_config().frontend_url,
            )
            .header(
                "location",
                format!("{}/whep", get_global_config().tcp_server_config.address),
            )
            .body(Full::new(Bytes::from(sdp_answer)))
            .unwrap()),
    }
}
