use std::net::SocketAddr;

use sdp::NegotiatedSession;
use thumbnail_image_extractor::ImageData;

use crate::EVENT_BUS;
use crate::ice_registry::SessionUsername;

pub mod dtls_actor;
pub mod get_packet_type;
pub mod keepalive_actor;
pub mod media_ingest_actor;
pub mod nominated_stun_actor;
pub mod receiver_report_actor;
pub mod rust_hyper;
pub mod session_master;
pub mod session_socket_actor;
pub mod thumbnail_generator_actor;
pub mod udp_io_actor;
pub mod unset_stun_actor;

type SessionID = usize;
type Oneshot<T> = tokio::sync::oneshot::Sender<T>;

#[derive(Debug)]
pub enum MessageEvent {
    NominateSession(SessionPointer),
    InitStreamer(NegotiatedSession),
    InitViewer(String, SessionID, Oneshot<Option<String>>),
    GetRoomThumbnail(SessionID, Oneshot<Option<ImageData>>),
    TerminateSession(SessionID),
    DebugSession(Oneshot<String>),
}

#[derive(Debug)]
pub struct SessionPointer {
    pub socket_address: SocketAddr,
    pub session_username: SessionUsername,
}

pub fn get_event_bus() -> &'static tokio::sync::mpsc::UnboundedSender<MessageEvent> {
    EVENT_BUS.get().unwrap()
}
