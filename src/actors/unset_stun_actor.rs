use std::net::SocketAddr;

use log::{debug, trace, warn};

use sdp::NegotiatedSession;

use crate::actors::{get_main_bus, MessageEvent, SessionPointer};
use crate::socket::{get_socket, send_packet};
use crate::stun::{create_stun_success, ICEStunMessageType};

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ReadPacket(ICEStunMessageType, SocketAddr),
}
/*
Respond to authorized ICE Live Check & Nominate Requests
 */
struct UnsetSTUNActor {
    receiver: Receiver,
    media_session: NegotiatedSession,
}

impl UnsetSTUNActor {
    pub async fn handle_message(&self, message: Message) {
        match message {
            Message::ReadPacket(stun_message_type, remote_addr) => match stun_message_type {
                ICEStunMessageType::LiveCheck(stun_packet) => {
                    trace!(target: "Unset STUN","Incoming Live Check: {:#?}", stun_packet);

                    let mut packet = vec![0u8; 200];
                    match create_stun_success(
                        &self.media_session.ice_credentials,
                        stun_packet.transaction_id,
                        &remote_addr,
                        &mut packet,
                    ) {
                        Ok(bytes_written) => {
                            send_packet(&packet[..bytes_written], &remote_addr).await
                        }
                        Err(_) => {
                            warn!(target: "Unset STUN", "Error creating a STUN success response for STUN message {:#?}", stun_packet);
                        }
                    }
                }
                ICEStunMessageType::Nomination(stun_packet) => {
                    trace!(target: "Unset STUN","Incoming Nominate request: {:#?}", stun_packet);

                    let mut packet = [0u8; 200];
                    match create_stun_success(
                        &self.media_session.ice_credentials,
                        stun_packet.transaction_id,
                        &remote_addr,
                        &mut packet,
                    ) {
                        Ok(bytes_written) => {
                            send_packet(&packet[..bytes_written], &remote_addr).await;
                            get_main_bus()
                                .send(MessageEvent::NominateSession(SessionPointer {
                                    socket_address: remote_addr,
                                    session_username: stun_packet.username_attribute,
                                }))
                                .unwrap()
                        }
                        Err(_) => {
                            warn!(target: "Unset STUN", "Error creating a STUN success response for STUN message {:#?}", stun_packet)
                        }
                    }
                }
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct UnsetSTUNActorHandle {
    pub sender: Sender,
}
impl UnsetSTUNActorHandle {
    pub fn new(media_session: NegotiatedSession) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = UnsetSTUNActor {
            media_session,
            receiver,
        };
        tokio::spawn(run(actor));

        Self { sender }
    }
}

async fn run(mut actor: UnsetSTUNActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
    trace!(target: "Unset STUN", "Dropping actor")
}
