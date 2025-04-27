use std::collections::VecDeque;
use std::io;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::SocketAddr;

use crate::actors::{EventProducer, MessageEvent};

type Sender = tokio::sync::mpsc::Sender<Message>;
type Receiver = tokio::sync::mpsc::Receiver<Message>;

enum Message {}

struct DTLSActor {
    receiver: Receiver,
    event_producer: EventProducer,
}

impl DTLSActor {
    fn new(receiver: Receiver, event_producer: EventProducer) -> Self {
        Self {
            receiver,
            event_producer,
        }
    }

    pub async fn handle_message(&self, message: Message) {}
}

struct DTLSActorHandle {
    pub sender: Sender,
}

impl DTLSActorHandle {
    pub fn new(event_producer: EventProducer) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel::<Message>(100);
        let actor = DTLSActor::new(receiver, event_producer);
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: DTLSActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
}

struct DTLSNegotiator {
    event_producer: EventProducer,
    socket_addr: SocketAddr,
    buffer: VecDeque<Vec<u8>>,
}

impl Write for DTLSNegotiator {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.event_producer
            .blocking_send(MessageEvent::ForwardPacket((
                buf.to_vec(),
                self.socket_addr,
            )))
            .unwrap();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Read for DTLSNegotiator {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(packet) = self.buffer.pop_front() {
            if packet.len() > buf.len() {
                return Err(Error::from(ErrorKind::InvalidData));
            }
            buf[0..packet.len()].copy_from_slice(&packet);
            Ok(packet.len())
        } else {
            Err(ErrorKind::WouldBlock.into())
        }
    }
}
