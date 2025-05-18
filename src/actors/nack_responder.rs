use bytes::Bytes;
use log::{debug, trace};

use rtcp::Unmarshall;

use crate::actors::session_socket_actor::SessionSocketActorHandle;
use crate::media_header::RTPHeader;
use crate::rtp_replay_buffer::ReplayBuffer;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    RegisterPacket(Vec<u8>),
    ResendPacket(u16),
}

struct NackResponderActor {
    receiver: Receiver,
    replay_buffer: ReplayBuffer,
    socket_handle: SessionSocketActorHandle,
}

impl NackResponderActor {
    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::RegisterPacket(packet) => self.replay_buffer.insert(Bytes::from(packet)),
            Message::ResendPacket(seq) => {
                if let Some(packet) = self.replay_buffer.get(seq) {
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
            replay_buffer: ReplayBuffer::default(),
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
