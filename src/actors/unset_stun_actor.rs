use std::net::SocketAddr;

use log::{debug, trace, warn};

use sdp::NegotiatedSession;

use crate::actors::{get_event_bus, MessageEvent, SessionPointer};
use crate::actors::udp_io_actor::UDPIOActorHandle;
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
    udp_io_actor_handle: UDPIOActorHandle,
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
                        Ok(bytes_written) => self
                            .udp_io_actor_handle
                            .sender
                            .send(crate::actors::udp_io_actor::Message::ForwardPacket((
                                packet[..bytes_written].to_vec(),
                                remote_addr,
                            )))
                            .unwrap(),
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
                            self.udp_io_actor_handle
                                .sender
                                .send(crate::actors::udp_io_actor::Message::ForwardPacket((
                                    packet[..bytes_written].to_vec(),
                                    remote_addr,
                                )))
                                .unwrap();

                            get_event_bus()
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
    pub fn new(media_session: NegotiatedSession, socket_io_actor_handle: UDPIOActorHandle) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = UnsetSTUNActor {
            media_session,
            udp_io_actor_handle: socket_io_actor_handle,
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
