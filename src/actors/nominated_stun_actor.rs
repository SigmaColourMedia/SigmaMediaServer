use std::net::SocketAddr;

use log::{debug, trace, warn};

use sdp::NegotiatedSession;

use crate::actors::{get_event_bus, MessageEvent};
use crate::actors::session_socket_actor::SessionSocketActorHandle;
use crate::stun::{create_stun_success, ICEStunMessageType};

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ReadPacket(ICEStunMessageType, SocketAddr),
}

/*
Respond to authorized ICE Live Check requests
 */
struct NominatedSTUNActor {
    session_socket_actor_handle: SessionSocketActorHandle,
    receiver: Receiver,
    media_session: NegotiatedSession,
}

impl NominatedSTUNActor {
    pub async fn handle_message(&self, message: Message) {
        match message {
            Message::ReadPacket(stun_message_type, remote_addr) => match stun_message_type {
                ICEStunMessageType::LiveCheck(stun_packet) => {
                    trace!(target: "Nominated STUN","Incoming Live Check: {:#?}", stun_packet);

                    let mut packet = vec![0u8; 200];
                    match create_stun_success(
                        &self.media_session.ice_credentials,
                        stun_packet.transaction_id,
                        &remote_addr,
                        &mut packet,
                    ) {
                        Ok(bytes_written) => self
                            .session_socket_actor_handle
                            .sender
                            .send(crate::actors::session_socket_actor::Message::ForwardPacket(
                                packet[..bytes_written].to_vec(),
                            ))
                            .unwrap(),
                        Err(_) => {
                            warn!(target: "Nominated STUN", "Error creating a STUN success response for STUN message {:#?}", stun_packet)
                        }
                    }
                }
                ICEStunMessageType::Nomination(_) => {
                    warn!(target: "Nominated STUN", "Unsupported Nomination request");
                }
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct NominatedSTUNActorHandle {
    pub sender: Sender,
}
impl NominatedSTUNActorHandle {
    pub fn new(
        media_session: NegotiatedSession,
        session_socket_actor_handle: SessionSocketActorHandle,
    ) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = NominatedSTUNActor {
            media_session,
            receiver,
            session_socket_actor_handle,
        };
        tokio::spawn(run(actor));

        Self { sender }
    }
}

async fn run(mut actor: NominatedSTUNActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await
    }
    trace!(target: "Nominated STUN", "Dropping actor")
}
