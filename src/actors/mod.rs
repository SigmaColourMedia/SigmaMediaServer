use std::net::SocketAddr;

use sdp::{ICECredentials, NegotiatedSession};

pub mod dtls_actor;
pub mod get_packet_type;
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

pub type Datagram = (Vec<u8>, SocketAddr);
pub type EventProducer = tokio::sync::mpsc::Sender<MessageEvent>;
