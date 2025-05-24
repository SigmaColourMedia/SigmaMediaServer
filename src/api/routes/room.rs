use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Incoming as IncomingBody, Request, Response};
use uuid::Uuid;

use crate::actors::{get_main_bus, MessageEvent, RoomData};
use crate::api::HTTPError;
use crate::api::routes::{HTTPResponse, RouteResult};
use crate::api::routes::error::error_route;

pub async fn room_get(req: Request<IncomingBody>) -> RouteResult {
    let res = room_resolver(req).await;
    match res {
        Ok(res) => Ok(res),
        Err(err) => error_route(err).await,
    }
}

async fn room_resolver(req: Request<IncomingBody>) -> Result<HTTPResponse, HTTPError> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<RoomData>>();
    let room_id = req
        .uri()
        .query()
        .and_then(|query| query.split("&").find(|item| item.starts_with("room_id=")))
        .and_then(|param| param.split_once("room_id="))
        .and_then(|(_, id)| Uuid::try_parse(id).ok())
        .ok_or(HTTPError::BadRequest)?;
    get_main_bus().send(MessageEvent::GetRooms(tx)).unwrap();

    let rooms_data = rx.await.unwrap();

    let target_room = rooms_data
        .into_iter()
        .find(|room| room.room_id == room_id)
        .ok_or(HTTPError::NotFound)?;

    Ok(Response::builder()
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(
            serde_json::to_vec(&target_room).unwrap(),
        )))
        .unwrap())
}
