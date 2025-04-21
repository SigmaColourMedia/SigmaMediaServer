use sdp::{ICECredentials, NegotiatedSession};
use std::net::SocketAddr;

pub mod rust_hyper;
pub mod session_master;
mod stun_actor;

#[derive(Debug)]
pub enum MessageEvent {
    NominateSession(SessionPointer),
    Test,
    InitStreamer(NegotiatedSession),
    ForwardPacket(Datagram),
}

#[derive(Debug)]
pub struct SessionPointer {
    socket_address: SocketAddr,
    ice_credentials: ICECredentials,
}

pub type Datagram = (Vec<u8>, SocketAddr);
pub type EventProducer = tokio::sync::mpsc::Sender<MessageEvent>;
