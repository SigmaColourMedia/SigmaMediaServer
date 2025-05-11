use std::net::SocketAddr;
use std::sync::Arc;

use log::{debug, warn};
use tokio::net::UdpSocket;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ForwardPacket((Vec<u8>, SocketAddr)),
}

struct UDPIOActor {
    receiver: Receiver,
    socket_handle: Arc<UdpSocket>,
}

impl UDPIOActor {
    pub async fn handle_message(&self, message: Message) {
        match message {
            Message::ForwardPacket(((packet, remote))) => {
                if let Err(err) = self.socket_handle.send_to(&packet, &remote).await {
                    warn!(target: "UDPIO Actor", "Error writing to remote:{} with error:{}", remote, err);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct UDPIOActorHandle {
    pub sender: Sender,
}

impl UDPIOActorHandle {
    pub fn new(socket_handle: Arc<UdpSocket>) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = UDPIOActor {
            receiver,
            socket_handle,
        };

        tokio::spawn(run(actor));

        Self { sender }
    }
}

async fn run(mut actor: UDPIOActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }

    debug!(target: "UDPIO Actor", "Dropping Actor");
}
