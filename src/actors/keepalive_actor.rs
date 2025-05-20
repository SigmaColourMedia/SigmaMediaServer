use std::time::Duration;

use log::{debug, trace};
use tokio::select;
use tokio::time::Instant;

use crate::actors::{get_main_bus, MessageEvent};

static MAX_TTL: Duration = Duration::from_secs(10);

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    UpdateTTL,
    ReportTermination,
}

struct KeepaliveActor {
    ttl: Instant,
    id: usize,
    receiver: Receiver,
}

impl KeepaliveActor {
    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::UpdateTTL => self.ttl = Instant::now(),
            Message::ReportTermination => get_main_bus()
                .send(MessageEvent::TerminateSession(self.id))
                .unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct KeepaliveActorHandle {
    pub sender: Sender,
}

impl KeepaliveActorHandle {
    pub fn new(id: usize) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = KeepaliveActor {
            ttl: Instant::now(),
            id,
            receiver,
        };
        tokio::spawn(run(actor));

        Self { sender }
    }
}

async fn run(mut actor: KeepaliveActor) {
    let sleep = tokio::time::sleep(Duration::from_secs(1));
    tokio::pin!(sleep);
    loop {
        select! {
            msg_option = actor.receiver.recv() => {
                match msg_option{
                    None => {
                        trace!(target: "Keepalive Actor", "Dropping Actor");
                        break
                    }
                    Some(msg) => {
                        actor.handle_message(msg).await;
                    }
                }
            },
            () = &mut sleep => {
                sleep.as_mut().reset(Instant::now() + Duration::from_secs(1));

                if actor.ttl.elapsed() > MAX_TTL{
                    actor.handle_message(Message::ReportTermination).await;
                }
            },
        }
    }
}
