use log::{debug, trace};

use thumbnail_image_extractor::{ImageData, ThumbnailExtractor};

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
}

impl ThumbnailGeneratorActor {
    fn new(receiver: Receiver) -> Self {
        Self {
            thumbnail_extractor: ThumbnailExtractor::new(),
            receiver,
        }
    }
    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ReadPacket(packet) => {
                if let Some(_) = self.thumbnail_extractor.try_extract_thumbnail(&packet) {};
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
    pub fn new() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = ThumbnailGeneratorActor::new(receiver);
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: ThumbnailGeneratorActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
    trace!(target: "Thumbnail Generator Actor", "Dropping Actor")
}
