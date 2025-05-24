use log::trace;

use crate::actors::rtp_cache::RTPCache;
use crate::actors::session_socket_actor::SessionSocketActorHandle;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    RegisterPacket(Vec<u8>),
    ResendPacket(u16),
}

struct NackResponderActor {
    receiver: Receiver,
    rtp_cache: RTPCache,
    socket_handle: SessionSocketActorHandle,
}

impl NackResponderActor {
    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::RegisterPacket(packet) => self.rtp_cache.insert_packet(packet),
            Message::ResendPacket(seq) => {
                if let Some(packet) = self.rtp_cache.get_packet(seq) {
                    self.socket_handle
                        .sender
                        .send(crate::actors::session_socket_actor::Message::ForwardPacket(
                            packet.to_vec(),
                        ))
                        .unwrap()
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct NackResponderActorHandle {
    pub sender: Sender,
}

impl NackResponderActorHandle {
    pub fn new(socket_handle: SessionSocketActorHandle) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = NackResponderActor {
            socket_handle,
            rtp_cache: RTPCache::new(),
            receiver,
        };
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: NackResponderActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }

    trace!(target: "Nack Responder Actor", "Dropping Actor");
}
