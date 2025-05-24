use std::sync::OnceLock;

use serde_json::json;
use uuid::Uuid;

use crate::actors::RoomData;
use crate::config::get_global_config;
use crate::socket::send_packet;

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
                        let event_data = RPCEvent {
                            method: Method::NewThumbnail(uuid),
                        }
                        .marshall();
                        send_packet(&event_data, &get_global_config().rtc_event_sink_addr).await;
                    }
                    ServerEvent::NewRoom(uuid) => {
                        let event_data = RPCEvent {
                            method: Method::NewRoom(uuid),
                        }
                        .marshall();
                        send_packet(&event_data, &get_global_config().rtc_event_sink_addr).await;
                    }
                    ServerEvent::RoomChange(room_data) => {
                        let event_data = RPCEvent {
                            method: Method::RoomChange(room_data),
                        }
                        .marshall();
                        send_packet(&event_data, &get_global_config().rtc_event_sink_addr).await;
                    }
                    ServerEvent::TerminateRoom(uuid) => {
                        let event_data = RPCEvent {
                            method: Method::TerminateRoom(uuid),
                        }
                        .marshall();
                        send_packet(&event_data, &get_global_config().rtc_event_sink_addr).await;
                    }
                }
            }
        }
    });
}

struct RPCEvent {
    method: Method,
}

impl RPCEvent {
    pub fn marshall(self) -> Vec<u8> {
        match self.method {
            Method::NewRoom(uuid) => json!({"jsonrpc": "2.0","method": "new_room", "params": {
                "uuid": uuid
            }})
            .to_string()
            .into_bytes(),
            Method::RoomChange(RoomData {
                room_id,
                viewer_count,
            }) => json!({"jsonrpc": "2.0",
                "method": "room_change", "params": {
                "uuid": room_id,
                "viewer_count": viewer_count
            }})
            .to_string()
            .into_bytes(),
            Method::TerminateRoom(uuid) => {
                json!({"jsonrpc": "2.0","method": "terminate_room", "params": {
                    "uuid": uuid
                }})
                .to_string()
                .into_bytes()
            }
            Method::NewThumbnail(uuid) => {
                json!({"jsonrpc": "2.0","method": "new_thumbnail", "params": {
                    "uuid": uuid
                }})
                .to_string()
                .into_bytes()
            }
        }
    }
}

enum Method {
    NewRoom(Uuid),
    RoomChange(RoomData),
    TerminateRoom(Uuid),
    NewThumbnail(Uuid),
}
