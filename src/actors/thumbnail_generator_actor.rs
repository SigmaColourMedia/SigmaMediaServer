use log::trace;
use uuid::Uuid;

use thumbnail_image_extractor::{ImageData, ThumbnailExtractor};

use crate::event_bus::{get_event_bus, ServerEvent};

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ReadPacket(Vec<u8>),
    GetPicture(tokio::sync::oneshot::Sender<Option<ImageData>>),
}
#[derive(Debug)]
struct ThumbnailGeneratorActor {
    receiver: Receiver,
    thumbnail_extractor: ThumbnailExtractor,
    owner_id: Uuid,
}

impl ThumbnailGeneratorActor {
    fn new(receiver: Receiver, owner_id: Uuid) -> Self {
        Self {
            thumbnail_extractor: ThumbnailExtractor::new(),
            receiver,
            owner_id,
        }
    }
    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ReadPacket(packet) => {
                if let Some(_) = self.thumbnail_extractor.try_extract_thumbnail(&packet) {
                    get_event_bus()
                        .send(ServerEvent::NewThumbnail(self.owner_id))
                        .unwrap()
                };
            }
            Message::GetPicture(sender) => sender
                .send(self.thumbnail_extractor.last_picture.clone())
                .unwrap(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct ThumbnailGeneratorActorHandle {
    pub sender: Sender,
}
impl ThumbnailGeneratorActorHandle {
    pub fn new(owner_id: Uuid) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = ThumbnailGeneratorActor::new(receiver, owner_id);
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: ThumbnailGeneratorActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
    trace!(target: "Thumbnail Generator Actor", "Dropping Actor: {}", actor.owner_id)
}
