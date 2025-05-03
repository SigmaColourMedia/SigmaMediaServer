use std::net::SocketAddr;

use sdp::NegotiatedSession;

use crate::EVENT_BUS;
use crate::ice_registry::SessionUsername;

pub mod dtls_actor;
pub mod get_packet_type;
pub mod media_digest_actor;
pub mod nominated_stun_actor;
pub mod rust_hyper;
pub mod session_master;
pub mod unset_stun_actor;

#[derive(Debug)]
pub enum MessageEvent {
    NominateSession(SessionPointer),
    Test,
    InitStreamer(NegotiatedSession),
    ForwardPacket(Datagram),
}

#[derive(Debug)]
pub struct SessionPointer {
    pub socket_address: SocketAddr,
    pub session_username: SessionUsername,
}

pub fn get_event_bus() -> &'static tokio::sync::mpsc::Sender<MessageEvent> {
    EVENT_BUS.get().unwrap()
}
pub type Datagram = (Vec<u8>, SocketAddr);
pub type EventProducer = tokio::sync::mpsc::Sender<MessageEvent>;
