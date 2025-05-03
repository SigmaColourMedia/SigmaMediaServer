use std::io::Error;
use std::net::SocketAddr;

use log::{debug, trace, warn};

use sdp::{ICECredentials, NegotiatedSession};

use crate::actors::{EventProducer, MessageEvent, SessionPointer};
use crate::stun::{create_stun_success, get_stun_packet, ICEStunMessageType, ICEStunPacket};

type Sender = tokio::sync::mpsc::Sender<Message>;
type Receiver = tokio::sync::mpsc::Receiver<Message>;

#[derive(Debug)]
pub struct STUNMessage {
    pub packet: ICEStunPacket,
    pub socket_addr: SocketAddr,
    pub ice_credentials: ICECredentials,
}

#[derive(Debug)]
pub enum Message {
    LiveCheck(STUNMessage),
    Nominate(STUNMessage),
}

struct STUNActor {
    receiver: Receiver,
    event_producer: EventProducer,
    media_session: NegotiatedSession,
}

impl STUNActor {
    fn new(
        receiver: Receiver,
        event_producer: EventProducer,
        media_session: NegotiatedSession,
    ) -> Self {
        Self {
            receiver,
            event_producer,
            media_session,
        }
    }

    /*
    Start & update session TTL, respond to STUN requests
     */
    pub async fn handle_message(&self, message: Message) {
        match message {
            Message::LiveCheck(stun_message) => {
                trace!(target: "STUN","Incoming Live Check: {:#?}", stun_message);

                let mut packet = vec![0u8; 200];
                match create_stun_success(
                    &stun_message.ice_credentials,
                    stun_message.packet.transaction_id,
                    &stun_message.socket_addr,
                    &mut packet,
                ) {
                    Ok(bytes_written) => self
                        .event_producer
                        .send(MessageEvent::ForwardPacket((
                            packet[..bytes_written].to_vec(),
                            stun_message.socket_addr,
                        )))
                        .await
                        .unwrap(),
                    Err(_) => {
                        warn!(target: "STUN", "Error creating a STUN success response for STUN message {:#?}", stun_message)
                    }
                }
            }
            Message::Nominate(stun_message) => {
                trace!(target: "STUN","Incoming Nominate request: {:#?}", stun_message);

                let mut packet = [0u8; 200];
                match create_stun_success(
                    &stun_message.ice_credentials,
                    stun_message.packet.transaction_id,
                    &stun_message.socket_addr,
                    &mut packet,
                ) {
                    Ok(bytes_written) => {
                        self.event_producer
                            .send(MessageEvent::ForwardPacket((
                                packet[..bytes_written].to_vec(),
                                stun_message.socket_addr,
                            )))
                            .await
                            .unwrap();
                        self.event_producer
                            .send(MessageEvent::NominateSession(SessionPointer {
                                socket_address: stun_message.socket_addr,
                                ice_credentials: stun_message.ice_credentials,
                            }))
                            .await
                            .unwrap()
                    }
                    Err(_) => {
                        warn!(target: "STUN", "Error creating a STUN success response for STUN message {:#?}", stun_message)
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct STUNActorHandle {
    pub sender: Sender,
}

impl STUNActorHandle {
    pub fn new(event_producer: EventProducer, media_session: NegotiatedSession) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel::<Message>(100);
        let actor = STUNActor::new(receiver, event_producer, media_session);
        tokio::spawn(run(actor));

        Self { sender }
    }
}

async fn run(mut actor: STUNActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
}
