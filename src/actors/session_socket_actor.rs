use std::net::SocketAddr;

use log::{debug, trace};

use crate::socket::send_packet;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ForwardPacket(Vec<u8>),
}
struct SessionSocketActor {
    receiver: Receiver,
    socket_addr: SocketAddr,
}

impl SessionSocketActor {
    pub async fn handle_message(&self, message: Message) {
        match message {
            Message::ForwardPacket(packet) => send_packet(&packet, &self.socket_addr).await,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionSocketActorHandle {
    pub sender: Sender,
}
impl SessionSocketActorHandle {
    pub fn new(socket_addr: SocketAddr) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = SessionSocketActor {
            receiver,
            socket_addr,
        };

        tokio::spawn(run(actor));

        Self { sender }
    }
}

async fn run(mut actor: SessionSocketActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }

    trace!(target: "SessionSocket Actor", "Dropping Actor");
}
