use std::net::SocketAddr;

use sdp::{ICECredentials, NegotiatedSession};

use crate::EVENT_BUS;

pub mod dtls_actor;
pub mod get_packet_type;
mod rtp_actor;
pub mod rust_hyper;
pub mod session_master;
pub mod stun_actor;

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
    pub ice_credentials: ICECredentials,
}

pub fn get_event_bus() -> &'static tokio::sync::mpsc::Sender<MessageEvent> {
    EVENT_BUS.get().unwrap()
}
pub type Datagram = (Vec<u8>, SocketAddr);
pub type EventProducer = tokio::sync::mpsc::Sender<MessageEvent>;
