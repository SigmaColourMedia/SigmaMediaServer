use std::net::SocketAddr;

use log::{debug, trace};

use crate::actors::udp_io_actor::UDPIOActorHandle;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ForwardPacket(Vec<u8>),
}
struct SessionSocketActor {
    receiver: Receiver,
    io_handle: UDPIOActorHandle,
    socket_addr: SocketAddr,
}

impl SessionSocketActor {
    pub async fn handle_message(&self, message: Message) {
        match message {
            Message::ForwardPacket(packet) => {
                self.io_handle
                    .sender
                    .send(crate::actors::udp_io_actor::Message::ForwardPacket((
                        packet,
                        self.socket_addr.clone(),
                    )))
                    .unwrap();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionSocketActorHandle {
    pub sender: Sender,
}
impl SessionSocketActorHandle {
    pub fn new(io_handle: UDPIOActorHandle, socket_addr: SocketAddr) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = SessionSocketActor {
            receiver,
            io_handle,
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
