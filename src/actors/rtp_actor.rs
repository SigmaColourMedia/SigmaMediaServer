type Sender = tokio::sync::mpsc::Sender<Message>;
type Receiver = tokio::sync::mpsc::Receiver<Message>;

pub enum Message {}

struct RTPActor {
    receiver: Receiver,
}
