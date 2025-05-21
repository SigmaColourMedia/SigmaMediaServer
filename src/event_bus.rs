use std::sync::OnceLock;

use log::debug;
use uuid::Uuid;

use crate::actors::RoomData;

pub enum ServerEvent {
    NewThumbnail(Uuid),
    NewRoom(Uuid),
    RoomChange(RoomData),
    TerminateRoom(Uuid),
}

static EVENT_BUS: OnceLock<tokio::sync::mpsc::UnboundedSender<ServerEvent>> = OnceLock::new();

pub fn get_event_bus() -> &'static tokio::sync::mpsc::UnboundedSender<ServerEvent> {
    EVENT_BUS
        .get()
        .expect("Attempted to access EVENT_BUS before initialization")
}

pub fn init_event_bus() {
    tokio::task::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ServerEvent>();
        EVENT_BUS.set(tx).unwrap();

        loop {
            if let Some(event) = rx.recv().await {
                match event {
                    ServerEvent::NewThumbnail(uuid) => {
                        debug!(target: "Event Bus","new thumb {}", uuid);
                    }
                    ServerEvent::NewRoom(id) => {
                        debug!(target: "Event Bus","new room {}", id)
                    }
                    ServerEvent::RoomChange(room_data) => {
                        debug!(target: "Event Bus","room data {:?}", room_data);
                    }
                    ServerEvent::TerminateRoom(uuid) => {
                        debug!(target: "Event Bus","room termination {:?}", uuid);
                    }
                }
            }
        }
    });
}
