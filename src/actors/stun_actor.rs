use std::net::SocketAddr;
use sdp::NegotiatedSession;
use crate::actors::{EventProducer, MessageEvent, SessionPointer};
use crate::stun::{create_stun_success, get_stun_packet, ICEStunMessageType};

type Sender = tokio::sync::mpsc::Sender<Message>;
type Receiver = tokio::sync::mpsc::Receiver<Message>;
type Datagram = (Vec<u8>, SocketAddr);

enum Message {
    Packet(Datagram)
}

struct STUNActor {
    receiver: Receiver,
    event_producer: EventProducer,
    media_session: NegotiatedSession,

}

impl STUNActor {
    fn new(receiver: Receiver, event_producer: EventProducer, media_session: NegotiatedSession,
    ) -> Self {
        Self { receiver, event_producer, media_session }
    }

    /*
    Start & update session TTL, respond to STUN requests
     */
    pub async fn handle_message(&self, message: Message) {
        match message {
            Message::Packet(datagram) => {
                let (packet, remote_addr, ) = datagram;
                if let Some(stun_msg) = get_stun_packet(&packet) {
                    match stun_msg {
                        ICEStunMessageType::LiveCheck(message) => {
                            // Send STUN SUCCESS
                            // todo Refactor stun success factory into a more sensible format
                            let mut buffer: [u8; 200] = [0; 200];
                            let bytes_written =
                                create_stun_success(&self.media_session.ice_credentials, message.transaction_id, &remote_addr, &mut buffer)
                                    .expect("Should create STUN success response");
                            let output_buffer = Vec::from(&buffer[0..bytes_written]);

                            self.event_producer.send(MessageEvent::ForwardPacket((output_buffer, remote_addr))).await.unwrap();
                        }
                        ICEStunMessageType::Nomination(message) => {
                            // Send STUN SUCCESS
                            let mut buffer: [u8; 200] = [0; 200];
                            let bytes_written =
                                create_stun_success(&self.media_session.ice_credentials, message.transaction_id, &remote_addr, &mut buffer)
                                    .expect("Should create STUN success response");
                            let output_buffer = Vec::from(&buffer[0..bytes_written]);

                            self.event_producer.send(MessageEvent::ForwardPacket((output_buffer, remote_addr))).await.unwrap();
                            self.event_producer.send(MessageEvent::NominateSession(SessionPointer { ice_credentials: self.media_session.ice_credentials.clone(), socket_address: remote_addr })).await.unwrap();
                        }
                    }
                };
            }
        }
    }
}

#[derive(Clone)]
pub struct STUNActorHandle {
    sender: Sender,
}


impl STUNActorHandle {
    pub fn new(event_producer: EventProducer, media_session: NegotiatedSession) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel::<Message>(100);
        let actor = STUNActor::new(receiver, event_producer, media_session);
        tokio::spawn(run(actor));

        Self {
            sender,
        }
    }
}

async fn run(mut actor: STUNActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
}