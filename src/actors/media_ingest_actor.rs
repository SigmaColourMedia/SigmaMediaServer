use std::io::Read;

use bytes::Bytes;
use log::{debug, warn};

use rtcp::Unmarshall;

use crate::actors;
use crate::actors::dtls_actor::{CryptoResult, DTLSActorHandle};
use crate::actors::receiver_report_actor::ReceiverReportActorHandle;
use crate::media_header::RTPHeader;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ReadPacket(Vec<u8>),
}

struct MediaIngestActor {
    receiver: Receiver,
    dtls_actor_handle: DTLSActorHandle,
    rr_actor_handle: ReceiverReportActorHandle,
}

impl MediaIngestActor {
    pub async fn handle_message(&self, message: Message) {
        match message {
            Message::ReadPacket(packet) => {
                let (tx, rx) = tokio::sync::oneshot::channel::<CryptoResult>();

                self.dtls_actor_handle
                    .sender
                    .send(actors::dtls_actor::Message::DecodeSRTP(packet, tx))
                    .unwrap();
                let decode_result = rx.await.unwrap();

                match decode_result {
                    Ok(packet) => {
                        let bytes = Bytes::from(packet);
                        let header = RTPHeader::unmarshall(bytes).unwrap();
                        self.rr_actor_handle
                            .sender
                            .send(actors::receiver_report_actor::Message::FeedRTP(header))
                            .unwrap()
                    }
                    Err(err) => {
                        warn!(target: "Media Ingest Actor", "Error decoding SRTP packet {:?}", err);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct MediaIngestActorHandle {
    pub sender: Sender,
}

impl MediaIngestActorHandle {
    pub fn new(
        dtls_actor_handle: DTLSActorHandle,
        rr_actor_handle: ReceiverReportActorHandle,
    ) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = MediaIngestActor {
            receiver,
            dtls_actor_handle,
            rr_actor_handle,
        };
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: MediaIngestActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    };
    debug!(target: "Media Ingest Actor", "Dropping Actor")
}
