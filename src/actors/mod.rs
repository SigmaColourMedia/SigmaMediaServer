use std::net::SocketAddr;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use sdp::NegotiatedSession;
use thumbnail_image_extractor::ImageData;
use crate::stun::SessionUsername;


pub mod dtls_actor;
pub mod get_packet_type;
pub mod keepalive_actor;
mod media_digest_actor;
pub mod media_ingest_actor;
mod nack_responder;
pub mod nominated_stun_actor;
pub mod receiver_report_actor;
mod rtp_cache;
pub mod session_master;
pub mod session_socket_actor;
pub mod thumbnail_generator_actor;
pub mod unset_stun_actor;
pub mod viewer_media_control_actor;

type Oneshot<T> = tokio::sync::oneshot::Sender<T>;

#[derive(Debug)]
pub enum MessageEvent {
    NominateSession(SessionPointer),
    InitStreamer(NegotiatedSession),
    InitViewer(String, Uuid, Oneshot<Option<String>>),
    GetRooms(Oneshot<Vec<RoomData>>),
    GetRoomThumbnail(Uuid, Oneshot<Option<ImageData>>),
    TerminateSession(Uuid),
    ForwardToViewers(Vec<u8>, Uuid),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RoomData {
    pub room_id: Uuid,
    pub viewer_count: usize,
}
#[derive(Debug)]
pub struct SessionPointer {
    pub socket_address: SocketAddr,
    pub session_username: SessionUsername,
}
pub static MAIN_BUS: OnceLock<tokio::sync::mpsc::UnboundedSender<MessageEvent>> = OnceLock::new();

// Get reference to main channel Sender
pub fn get_main_bus() -> &'static tokio::sync::mpsc::UnboundedSender<MessageEvent> {
    MAIN_BUS.get().unwrap()
}
