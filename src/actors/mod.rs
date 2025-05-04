use std::net::SocketAddr;

use sdp::NegotiatedSession;

use crate::EVENT_BUS;
use crate::ice_registry::SessionUsername;

pub mod dtls_actor;
pub mod get_packet_type;
pub mod media_ingest_actor;
pub mod nominated_stun_actor;
pub mod receiver_report_actor;
pub mod rust_hyper;
pub mod session_master;
pub mod unset_stun_actor;

#[derive(Debug)]
pub enum MessageEvent {
    NominateSession(SessionPointer),
    InitStreamer(NegotiatedSession),
    ForwardPacket(Datagram),
    DebugSession(tokio::sync::oneshot::Sender<String>),
}

#[derive(Debug)]
pub struct SessionPointer {
    pub socket_address: SocketAddr,
    pub session_username: SessionUsername,
}

pub fn get_event_bus() -> &'static tokio::sync::mpsc::UnboundedSender<MessageEvent> {
    EVENT_BUS.get().unwrap()
}
pub type Datagram = (Vec<u8>, SocketAddr);
