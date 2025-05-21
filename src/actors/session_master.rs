use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use log::{debug, trace, warn};
use rand::random;
use uuid::{uuid, Uuid};

use sdp::NegotiatedSession;
use thumbnail_image_extractor::ImageData;

use crate::actors::{RoomData, SessionPointer};
use crate::actors::dtls_actor::DTLSActorHandle;
use crate::actors::keepalive_actor::KeepaliveActorHandle;
use crate::actors::media_digest_actor::MediaDigestActorHandle;
use crate::actors::media_ingest_actor::MediaIngestActorHandle;
use crate::actors::nack_responder::NackResponderActorHandle;
use crate::actors::nominated_stun_actor::NominatedSTUNActorHandle;
use crate::actors::receiver_report_actor::ReceiverReportActorHandle;
use crate::actors::session_socket_actor::SessionSocketActorHandle;
use crate::actors::thumbnail_generator_actor::ThumbnailGeneratorActorHandle;
use crate::actors::udp_io_actor::UDPIOActorHandle;
use crate::actors::unset_stun_actor::UnsetSTUNActorHandle;
use crate::actors::viewer_media_control_actor::ViewerMediaControlActorHandle;
use crate::event_bus::{get_event_bus, ServerEvent};
use crate::ice_registry::SessionUsername;

#[derive(Debug)]
pub struct SessionMaster {
    nominated_map: NominatedSessionMap,
    room_map: HashMap<Uuid, Room>,
    unset_map: UnsetSessionMap,
    socket_io_actor_handle: UDPIOActorHandle,
}

impl SessionMaster {
    pub fn new(socket_io_actor_handle: UDPIOActorHandle) -> Self {
        Self {
            nominated_map: NominatedSessionMap::new(),
            unset_map: UnsetSessionMap::new(),
            room_map: HashMap::new(),
            socket_io_actor_handle,
        }
    }

    pub fn remove_session(&mut self, id: Uuid) {
        // Nominated Session removal
        if let Some(session) = self.nominated_map.session_map.remove(&id) {
            self.nominated_map.address_map.remove(session.get_address());

            match session {
                // Remove all Viewers
                NominatedSession::Streamer(_) => {
                    debug!(target: "Session Master", "Removing Nominated Streamer ID {} at address: {}", id, session.get_address());
                    debug!(target: "Session Master", "Removing Room ID {}", id);
                    let viewers = self.room_map.get(&id).unwrap().viewers_ids.clone();
                    self.room_map.remove(&id);
                    for viewer_id in viewers {
                        debug!(target: "Session Master", "Removing Viewer of {}", id);
                        self.remove_session(viewer_id)
                    }
                    get_event_bus()
                        .send(ServerEvent::TerminateRoom(id))
                        .unwrap();
                }
                // Remove viewer from Room
                NominatedSession::Viewer(viewer) => {
                    debug!(target: "Session Master", "Removing Nominated Viewer ID {} at address: {}", id, viewer._socket_address);
                    if let Some(target_room) = self.room_map.get_mut(&viewer._target_room_id) {
                        debug!(target: "Session Master", "Removing Viewer ID {} from Room ID {}", id, viewer._target_room_id);
                        target_room.viewers_ids.remove(&id);
                        get_event_bus()
                            .send(ServerEvent::RoomChange(RoomData {
                                room_id: viewer._target_room_id,
                                viewer_count: target_room.viewers_ids.len(),
                            }))
                            .unwrap();
                    }
                }
            }
        }
        // Unset Session Removal
        else if let Some(session) = self.unset_map.session_map.remove(&id) {
            self.unset_map
                .ice_username_map
                .remove(session.get_username());
            match session {
                UnsetSession::Streamer(_) => {
                    debug!(target: "Session Master", "Removing Unset Streamer Session ID {}", id);
                }
                UnsetSession::Viewer(_) => {
                    debug!(target: "Session Master", "Removing Unset Viewer Session ID {}", id);
                }
            };
        }
    }

    pub fn get_unset_session(&self, session_username: &SessionUsername) -> Option<&UnsetSession> {
        self.unset_map
            .ice_username_map
            .get(session_username)
            .and_then(|id| self.unset_map.session_map.get(id))
    }
    pub async fn get_room_thumbnail(&self, id: Uuid) -> Option<ImageData> {
        let thumbnail_generator_handle =
            self.nominated_map
                .session_map
                .get(&id)
                .and_then(|session| match session {
                    NominatedSession::Streamer(streamer) => {
                        Some(&streamer.thumbnail_generator_handle)
                    }
                    _ => None,
                });

        match thumbnail_generator_handle {
            None => None,
            Some(handle) => {
                let (tx, rx) = tokio::sync::oneshot::channel::<Option<ImageData>>();
                handle
                    .sender
                    .send(crate::actors::thumbnail_generator_actor::Message::GetPicture(tx))
                    .unwrap();
                rx.await.unwrap()
            }
        }
    }

    pub fn get_unset_session_mut(
        &mut self,
        session_username: &SessionUsername,
    ) -> Option<&mut UnsetSession> {
        self.unset_map
            .ice_username_map
            .get_mut(session_username)
            .and_then(|id| self.unset_map.session_map.get_mut(id))
    }
    pub fn get_session_mut(&mut self, remote_addr: &SocketAddr) -> Option<&mut NominatedSession> {
        self.nominated_map
            .address_map
            .get_mut(remote_addr)
            .and_then(|id| self.nominated_map.session_map.get_mut(id))
    }

    pub fn get_room_negotiated_session(&self, id: Uuid) -> Option<&NegotiatedSession> {
        self.room_map.get(&id).map(|room| &room.host_session)
    }

    pub fn get_session(&self, remote_addr: &SocketAddr) -> Option<&NominatedSession> {
        self.nominated_map
            .address_map
            .get(remote_addr)
            .and_then(|id| self.nominated_map.session_map.get(id))
    }
    pub fn add_viewer(&mut self, room_id: Uuid, negotiated_session: NegotiatedSession) {
        let id = Uuid::new_v4();
        let session_username = SessionUsername {
            host: negotiated_session.ice_credentials.host_username.clone(),
            remote: negotiated_session.ice_credentials.remote_username.clone(),
        };
        let unset_session = UnsetSession::Viewer(UnsetViewerSession {
            keepalive_handle: KeepaliveActorHandle::new(id),
            negotiated_session: negotiated_session.clone(),
            stun_actor_handle: UnsetSTUNActorHandle::new(
                negotiated_session,
                self.socket_io_actor_handle.clone(),
            ),
            _target_room_id: room_id,
            _ice_username: session_username.clone(),
        });
        trace!(target: "Session Master", "Created Viewer Unset Session {:#?}", unset_session);

        self.unset_map.ice_username_map.insert(session_username, id);
        self.unset_map.session_map.insert(id, unset_session);
    }

    pub fn add_streamer(&mut self, negotiated_session: NegotiatedSession) {
        let id = Uuid::new_v4();

        let session_username = SessionUsername {
            host: negotiated_session.ice_credentials.host_username.clone(),
            remote: negotiated_session.ice_credentials.remote_username.clone(),
        };
        let unset_session = UnsetSession::Streamer(UnsetStreamerSession {
            keepalive_handle: KeepaliveActorHandle::new(id),
            negotiated_session: negotiated_session.clone(),
            stun_actor_handle: UnsetSTUNActorHandle::new(
                negotiated_session,
                self.socket_io_actor_handle.clone(),
            ),
            _ice_username: session_username.clone(),
        });
        trace!(target: "Session Master", "Created Streamer Unset Session {:#?}", unset_session);

        self.unset_map.ice_username_map.insert(session_username, id);
        self.unset_map.session_map.insert(id, unset_session);
    }

    pub fn nominate_session(&mut self, session_pointer: SessionPointer) {
        let remote_addr = session_pointer.socket_address;

        let unset_session = self
            .unset_map
            .ice_username_map
            .remove(&SessionUsername {
                remote: session_pointer.session_username.remote,
                host: session_pointer.session_username.host,
            })
            .and_then(|id| self.unset_map.session_map.remove(&id))
            .expect("Attempted to nominate a non-existing session");

        match unset_session {
            UnsetSession::Streamer(session_data) => {
                let id = Uuid::new_v4();

                let session_socket_handle = SessionSocketActorHandle::new(
                    self.socket_io_actor_handle.clone(),
                    remote_addr.clone(),
                );
                let dtls_handle = DTLSActorHandle::new(session_socket_handle.clone());
                let rr_handle = ReceiverReportActorHandle::new(
                    session_data.negotiated_session.clone(),
                    session_socket_handle.clone(),
                    dtls_handle.clone(),
                );
                let thumbnail_handle = ThumbnailGeneratorActorHandle::new(id);

                let nominated_session = NominatedSession::Streamer(StreamerSessionData {
                    keepalive_handle: KeepaliveActorHandle::new(id),
                    media_digest_actor_handle: MediaIngestActorHandle::new(
                        dtls_handle.clone(),
                        rr_handle,
                        thumbnail_handle.clone(),
                        session_data.negotiated_session.clone(),
                        id,
                    ),
                    thumbnail_generator_handle: thumbnail_handle,
                    dtls_actor: dtls_handle,
                    stun_actor_handle: NominatedSTUNActorHandle::new(
                        session_data.negotiated_session.clone(),
                        session_socket_handle,
                    ),
                    _socket_address: remote_addr.clone(),
                });
                trace!(target: "Session Master", "Created NominatedSession {:#?}", nominated_session);

                self.nominated_map.address_map.insert(remote_addr, id);
                self.nominated_map.session_map.insert(id, nominated_session);
                self.room_map
                    .insert(id, Room::new(session_data.negotiated_session));
                debug!(target: "Session Master","Nominated Streamer Session with ID: {}", id);
                debug!(target: "Session Master","Opened Room with ID: {}", id);
                get_event_bus().send(ServerEvent::NewRoom(id)).unwrap()
            }
            UnsetSession::Viewer(session_data) => {
                // Check if target Room exists (it could've been removed before Viewer nomination event)
                match self.room_map.get(&session_data._target_room_id) {
                    None => {
                        warn!(target: "Session Master", "Attempted to nominate Viewer registered to undefined Room");
                        return;
                    }
                    Some(room) => {
                        let id = Uuid::new_v4();
                        let socket_handle = SessionSocketActorHandle::new(
                            self.socket_io_actor_handle.clone(),
                            remote_addr.clone(),
                        );
                        let dtls_handle = DTLSActorHandle::new(socket_handle.clone());
                        let nack_handle = NackResponderActorHandle::new(socket_handle.clone());
                        let media_digest_handle = MediaDigestActorHandle::new(
                            socket_handle.clone(),
                            dtls_handle.clone(),
                            nack_handle.clone(),
                            room.host_session.clone(),
                            session_data.negotiated_session.clone(),
                        );
                        let media_control_handle =
                            ViewerMediaControlActorHandle::new(dtls_handle.clone(), nack_handle);

                        let nominated_session = NominatedSession::Viewer(ViewerSessionData {
                            keepalive_handle: KeepaliveActorHandle::new(id),
                            dtls_actor: dtls_handle,
                            stun_actor_handle: NominatedSTUNActorHandle::new(
                                session_data.negotiated_session.clone(),
                                socket_handle,
                            ),
                            media_control_actor: media_control_handle,
                            _socket_address: remote_addr.clone(),
                            _target_room_id: session_data._target_room_id,
                            media_digest_actor_handle: media_digest_handle,
                        });
                        trace!(target: "Session Master", "Created NominatedSession {:#?}", nominated_session);

                        self.nominated_map.address_map.insert(remote_addr, id);
                        self.nominated_map.session_map.insert(id, nominated_session);

                        let target_room = self
                            .room_map
                            .get_mut(&session_data._target_room_id)
                            .expect("Host room must exist for viewer to be nominated");

                        target_room.viewers_ids.insert(id);

                        get_event_bus()
                            .send(ServerEvent::RoomChange(RoomData {
                                room_id: session_data._target_room_id,
                                viewer_count: target_room.viewers_ids.len(),
                            }))
                            .unwrap();

                        debug!(target: "Session Master","Nominated Viewer Session with ID: {}", id);
                    }
                }
            }
        }
    }

    pub fn forward_packet_to_viewers(&self, packet: Vec<u8>, room_id: Uuid) {
        let media_digest_actor_handles = &self
            .room_map
            .get(&room_id)
            .unwrap()
            .viewers_ids
            .iter()
            .filter_map(|id| self.nominated_map.session_map.get(id))
            .filter_map(|session| match session {
                NominatedSession::Streamer(_) => None,
                NominatedSession::Viewer(viewer) => Some(viewer),
            })
            .map(|viewer| &viewer.media_digest_actor_handle)
            .collect::<Vec<&MediaDigestActorHandle>>();

        for handle in media_digest_actor_handles {
            handle
                .sender
                .send(crate::actors::media_digest_actor::Message::ReadPacket(
                    packet.clone(),
                ))
                .unwrap();
        }
    }
    pub fn get_rooms(&self) -> Vec<RoomData> {
        self.room_map
            .iter()
            .map(|(id, room)| RoomData {
                room_id: id.clone(),
                viewer_count: room.viewers_ids.len(),
            })
            .collect()
    }
}

#[derive(Debug)]
struct Room {
    viewers_ids: HashSet<Uuid>,
    pub host_session: NegotiatedSession,
}

impl Room {
    pub fn new(negotiated_session: NegotiatedSession) -> Self {
        Self {
            host_session: negotiated_session,
            viewers_ids: HashSet::new(),
        }
    }
}

#[derive(Debug)]
struct NominatedSessionMap {
    session_map: HashMap<Uuid, NominatedSession>,
    address_map: HashMap<SocketAddr, Uuid>,
}

impl NominatedSessionMap {
    pub fn new() -> Self {
        Self {
            session_map: HashMap::new(),
            address_map: HashMap::new(),
        }
    }
}

#[derive(Debug)]
struct UnsetSessionMap {
    session_map: HashMap<Uuid, UnsetSession>,
    ice_username_map: HashMap<SessionUsername, Uuid>,
}

impl UnsetSessionMap {
    pub fn new() -> Self {
        Self {
            session_map: HashMap::new(),
            ice_username_map: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub enum NominatedSession {
    Streamer(StreamerSessionData),
    Viewer(ViewerSessionData),
}

impl NominatedSession {
    pub fn get_stun_handle(&self) -> &NominatedSTUNActorHandle {
        match self {
            NominatedSession::Streamer(streamer) => &streamer.stun_actor_handle,
            NominatedSession::Viewer(viewer) => &viewer.stun_actor_handle,
        }
    }

    pub fn get_keepalive_handle(&self) -> &KeepaliveActorHandle {
        match self {
            NominatedSession::Streamer(streamer) => &streamer.keepalive_handle,
            NominatedSession::Viewer(viewer) => &viewer.keepalive_handle,
        }
    }

    pub fn get_dtls_handle(&self) -> &DTLSActorHandle {
        match self {
            NominatedSession::Streamer(streamer) => &streamer.dtls_actor,
            NominatedSession::Viewer(viewer) => &viewer.dtls_actor,
        }
    }

    pub fn get_address(&self) -> &SocketAddr {
        match self {
            NominatedSession::Streamer(streamer) => &streamer._socket_address,
            NominatedSession::Viewer(viewer) => &viewer._socket_address,
        }
    }
}

#[derive(Debug)]
pub enum UnsetSession {
    Streamer(UnsetStreamerSession),
    Viewer(UnsetViewerSession),
}

impl UnsetSession {
    pub fn get_stun_handle(&self) -> &UnsetSTUNActorHandle {
        match self {
            UnsetSession::Streamer(streamer) => &streamer.stun_actor_handle,
            UnsetSession::Viewer(viewer) => &viewer.stun_actor_handle,
        }
    }

    pub fn get_username(&self) -> &SessionUsername {
        match self {
            UnsetSession::Streamer(streamer) => &streamer._ice_username,
            UnsetSession::Viewer(viewer) => &viewer._ice_username,
        }
    }

    pub fn get_keepalive_handle(&self) -> &KeepaliveActorHandle {
        match self {
            UnsetSession::Streamer(streamer) => &streamer.keepalive_handle,
            UnsetSession::Viewer(viewer) => &viewer.keepalive_handle,
        }
    }
}

#[derive(Debug)]
pub struct UnsetStreamerSession {
    pub keepalive_handle: KeepaliveActorHandle,
    pub negotiated_session: NegotiatedSession,
    pub stun_actor_handle: UnsetSTUNActorHandle,
    _ice_username: SessionUsername,
}

#[derive(Debug)]
pub struct UnsetViewerSession {
    pub keepalive_handle: KeepaliveActorHandle,
    pub negotiated_session: NegotiatedSession,
    pub stun_actor_handle: UnsetSTUNActorHandle,
    _target_room_id: Uuid,
    _ice_username: SessionUsername,
}

#[derive(Debug)]
pub struct StreamerSessionData {
    pub keepalive_handle: KeepaliveActorHandle,
    pub stun_actor_handle: NominatedSTUNActorHandle,
    pub thumbnail_generator_handle: ThumbnailGeneratorActorHandle,
    pub media_digest_actor_handle: MediaIngestActorHandle,
    pub dtls_actor: DTLSActorHandle,
    _socket_address: SocketAddr,
}

#[derive(Debug)]
pub struct ViewerSessionData {
    pub keepalive_handle: KeepaliveActorHandle,
    pub stun_actor_handle: NominatedSTUNActorHandle,
    pub dtls_actor: DTLSActorHandle,
    pub media_control_actor: ViewerMediaControlActorHandle,
    pub media_digest_actor_handle: MediaDigestActorHandle,
    _socket_address: SocketAddr,
    _target_room_id: Uuid,
}
