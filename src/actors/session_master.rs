use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use log::{debug, trace};
use rand::random;

use sdp::NegotiatedSession;
use thumbnail_image_extractor::ImageData;

use crate::actors::dtls_actor::DTLSActorHandle;
use crate::actors::keepalive_actor::KeepaliveActorHandle;
use crate::actors::media_ingest_actor::MediaIngestActorHandle;
use crate::actors::nominated_stun_actor::NominatedSTUNActorHandle;
use crate::actors::receiver_report_actor::ReceiverReportActorHandle;
use crate::actors::session_socket_actor::SessionSocketActorHandle;
use crate::actors::SessionPointer;
use crate::actors::thumbnail_generator_actor::ThumbnailGeneratorActorHandle;
use crate::actors::udp_io_actor::UDPIOActorHandle;
use crate::actors::unset_stun_actor::UnsetSTUNActorHandle;
use crate::ice_registry::SessionUsername;

#[derive(Debug)]
pub struct SessionMaster {
    nominated_map: NominatedSessionMap,
    room_map: HashMap<usize, Room>,
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

    pub fn remove_session(&mut self, id: usize) {
        // Nominated Session removal
        if let Some(session) = self.nominated_map.session_map.remove(&id) {
            self.nominated_map.address_map.remove(session.get_address());

            match session {
                // Remove all Viewers
                NominatedSession::Streamer(_) => {
                    debug!(target: "Session Master", "Removing Nominated Streamer ID {} at address: {}", id, session.get_address());
                    let viewers = self.room_map.get(&id).unwrap().viewers_ids.clone();
                    for viewer_id in viewers {
                        debug!(target: "Session Master", "Removing Viewer of {}", id);
                        self.remove_session(viewer_id)
                    }
                    debug!(target: "Session Master", "Removing Room ID {}", id);
                    self.room_map.remove(&id);
                }
                // Remove viewer from Room
                NominatedSession::Viewer(viewer) => {
                    debug!(target: "Session Master", "Removing Nominated Viewer ID {} at address: {}", id, viewer._socket_address);
                    debug!(target: "Session Master", "Removing Viewer ID {} from Room ID {}", id, viewer._target_room_id);
                    self.room_map
                        .get_mut(&viewer._target_room_id)
                        .expect("Streamer Room must exist before Viewer is deleted")
                        .viewers_ids
                        .remove(&id);
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
    pub async fn get_room_thumbnail(&self, id: usize) -> Option<ImageData> {
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

    pub fn get_room_negotiated_session(&self, id: usize) -> Option<&NegotiatedSession> {
        self.room_map.get(&id).map(|room| &room.host_session)
    }

    pub fn get_session(&self, remote_addr: &SocketAddr) -> Option<&NominatedSession> {
        self.nominated_map
            .address_map
            .get(remote_addr)
            .and_then(|id| self.nominated_map.session_map.get(id))
    }
    pub fn add_viewer(&mut self, room_id: usize, negotiated_session: NegotiatedSession) {
        let id = random::<usize>();
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
        let id = random::<usize>();

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
                let thumbnail_handle = ThumbnailGeneratorActorHandle::new();

                let id = random::<usize>();

                let nominated_session = NominatedSession::Streamer(StreamerSessionData {
                    keepalive_handle: KeepaliveActorHandle::new(id),
                    media_digest_actor_handle: MediaIngestActorHandle::new(
                        dtls_handle.clone(),
                        rr_handle,
                        thumbnail_handle.clone(),
                        session_data.negotiated_session.clone(),
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
            }
            UnsetSession::Viewer(session_data) => {
                // todo support Viewer
                let session_socket_handle = SessionSocketActorHandle::new(
                    self.socket_io_actor_handle.clone(),
                    remote_addr.clone(),
                );
                let dtls_handle = DTLSActorHandle::new(session_socket_handle.clone());

                let id = random::<usize>();

                let nominated_session = NominatedSession::Viewer(ViewerSessionData {
                    keepalive_handle: KeepaliveActorHandle::new(id),
                    dtls_actor: dtls_handle,
                    stun_actor_handle: NominatedSTUNActorHandle::new(
                        session_data.negotiated_session.clone(),
                        session_socket_handle,
                    ),
                    _socket_address: remote_addr.clone(),
                    _target_room_id: session_data._target_room_id,
                });
                trace!(target: "Session Master", "Created NominatedSession {:#?}", nominated_session);

                self.nominated_map.address_map.insert(remote_addr, id);
                self.nominated_map.session_map.insert(id, nominated_session);
                self.room_map
                    .get_mut(&session_data._target_room_id)
                    .expect("Host room must exist for viewer to be nominated")
                    .viewers_ids
                    .insert(id);

                debug!(target: "Session Master","Nominated Viewer Session with ID: {}", id);
            }
        }
    }
}

#[derive(Debug)]
struct Room {
    viewers_ids: HashSet<usize>,
    host_session: NegotiatedSession,
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
    session_map: HashMap<usize, NominatedSession>,
    address_map: HashMap<SocketAddr, usize>,
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
    session_map: HashMap<usize, UnsetSession>,
    ice_username_map: HashMap<SessionUsername, usize>,
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
    _target_room_id: usize,
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
    _socket_address: SocketAddr,
    _target_room_id: usize,
}
