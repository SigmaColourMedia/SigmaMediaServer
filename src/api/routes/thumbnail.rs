use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Incoming as IncomingBody, Request, Response};
use uuid::Uuid;
use webp::PixelLayout;

use thumbnail_image_extractor::ImageData;

use crate::actors::{get_main_bus, MessageEvent};
use crate::api::HTTPError;
use crate::api::routes::{HTTPResponse, RouteResult};
use crate::api::routes::error::error_route;

pub async fn thumbnail_get(req: Request<IncomingBody>) -> RouteResult {
    let res = thumbnail_resolver(req).await;
    match res {
        Ok(res) => Ok(res),
        Err(err) => error_route(err).await,
    }
}

async fn thumbnail_resolver(req: Request<IncomingBody>) -> Result<HTTPResponse, HTTPError> {
    let room_id = req
        .uri()
        .query()
        .and_then(|query| query.split("&").find(|item| item.starts_with("room_id=")))
        .and_then(|param| param.split_once("room_id="))
        .and_then(|(_, id)| Uuid::try_parse(id).ok())
        .ok_or(HTTPError::BadRequest)?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<ImageData>>();
    get_main_bus()
        .send(MessageEvent::GetRoomThumbnail(room_id, tx))
        .unwrap();
    let image_data = rx.await.unwrap().ok_or(HTTPError::NotFound)?;
    // todo Encoding takes some time. See if moved to thumbnail_generator_actor would help. Maybe create thumbnail only once in a while?
    let encoder = webp::Encoder::new(
        &image_data.data_buffer,
        PixelLayout::Rgb,
        image_data.width as u32,
        image_data.height as u32,
    );

    let encoded = encoder.encode(75.0);
    Ok(Response::builder()
        .body(Full::new(Bytes::from(encoded.to_vec())))
        .unwrap())
}
