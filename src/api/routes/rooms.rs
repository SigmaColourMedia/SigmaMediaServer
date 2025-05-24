use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Incoming as IncomingBody, Request, Response};

use crate::actors::{get_main_bus, MessageEvent, RoomData};
use crate::api::routes::RouteResult;

pub async fn rooms_get(_: Request<IncomingBody>) -> RouteResult {
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<RoomData>>();

    get_main_bus().send(MessageEvent::GetRooms(tx)).unwrap();

    let rooms_data = rx.await.unwrap();

    Ok(Response::builder()
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(
            serde_json::to_vec(&rooms_data).unwrap(),
        )))
        .unwrap())
}
