use std::sync::OnceLock;

use crate::actors::MessageEvent;

type ID = usize;
pub enum ServerEvent {
    NewThumbnail(usize),
    NewRoom(RoomData),
    RoomChange(RoomData),
}

struct RoomData {
    room_id: usize,
    viewer_count: usize,
}
pub static EVENT_BUS: OnceLock<tokio::sync::mpsc::UnboundedSender<MessageEvent>> = OnceLock::new();
