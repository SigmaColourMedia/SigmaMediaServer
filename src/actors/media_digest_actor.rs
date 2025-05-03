use std::net::SocketAddr;

use log::{debug, warn};

use crate::actors;
use crate::actors::dtls_actor::{DTLSActorHandle, SRTPDecodeResult};

type Sender = tokio::sync::mpsc::Sender<Message>;
type Receiver = tokio::sync::mpsc::Receiver<Message>;

pub enum Message {
    ReadPacket(Vec<u8>),
}

struct MediaDigestActor {
    receiver: Receiver,
    dtls_actor_handle: DTLSActorHandle,
}

impl MediaDigestActor {
    pub async fn handle_message(&self, message: Message) {
        match message {
            Message::ReadPacket(packet) => {
                let (tx, rx) = tokio::sync::oneshot::channel::<SRTPDecodeResult>();

                self.dtls_actor_handle
                    .sender
                    .send(actors::dtls_actor::Message::DecodeSRTP(packet, tx))
                    .unwrap();
                let decode_result = rx.await.unwrap();

                match decode_result {
                    Ok(packet) => {}
                    Err(err) => {
                        warn!(target: "Media Digest Actor", "Error decoding SRTP packet {:?}", err);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct MediaDigestActorHandle {
    pub sender: Sender,
}

impl MediaDigestActorHandle {
    pub fn new(dtls_actor_handle: DTLSActorHandle) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel::<Message>(100);
        let actor = MediaDigestActor {
            receiver,
            dtls_actor_handle,
        };
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: MediaDigestActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
}
