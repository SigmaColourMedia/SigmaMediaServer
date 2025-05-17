use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::Duration;

use log::{debug, trace};
use rand::random;
use tokio::time::Instant;

use sdp::NegotiatedSession;

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
        debug!(target: "Session Master", "Removing session {}", id);
        self.nominated_map
            .session_map
            .remove(&id)
            .and_then(|session| {
                let address = match session {
                    NominatedSession::Streamer(streamer) => streamer._socket_address,
                };
                self.nominated_map.address_map.remove(&address)
            });
        self.unset_map.session_map.remove(&id).and_then(|session| {
            let username = match session {
                UnsetSession::Streamer(streamer) => streamer._ice_username,
                UnsetSession::Viewer(viewer) => viewer._ice_username,
            };
            self.unset_map.ice_username_map.remove(&username)
        });
        self.room_map.remove(&id);
    }

    pub fn get_unset_session(&self, session_username: &SessionUsername) -> Option<&UnsetSession> {
        self.unset_map
            .ice_username_map
            .get(session_username)
            .and_then(|id| self.unset_map.session_map.get(id))
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

    pub fn get_session(&self, remote_addr: &SocketAddr) -> Option<&NominatedSession> {
        self.nominated_map
            .address_map
            .get(remote_addr)
            .and_then(|id| self.nominated_map.session_map.get(id))
    }
    pub fn add_streamer(&mut self, negotiated_session: NegotiatedSession) {
        let id = random::<usize>();
        let session_username = SessionUsername {
            host: negotiated_session.ice_credentials.host_username.clone(),
            remote: negotiated_session.ice_credentials.remote_username.clone(),
        };
        let unset_session = UnsetSession::Streamer(UnsetSessionData {
            keepalive_handle: KeepaliveActorHandle::new(id),
            negotiated_session: negotiated_session.clone(),
            stun_actor_handle: UnsetSTUNActorHandle::new(
                negotiated_session,
                self.socket_io_actor_handle.clone(),
            ),
            _ice_username: session_username.clone(),
        });
        trace!(target: "Session Master", "Created streamer unset_session {:#?}", unset_session);

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
                trace!(target: "Session Master", "Created nominated_session {:#?}", nominated_session);

                self.nominated_map.address_map.insert(remote_addr, id);
                self.nominated_map.session_map.insert(id, nominated_session);
                self.room_map
                    .insert(id, Room::new(session_data.negotiated_session));
            }
            UnsetSession::Viewer(_) => {
                // todo support Viewer
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
}

#[derive(Debug)]
pub enum UnsetSession {
    Streamer(UnsetSessionData),
    Viewer(UnsetSessionData),
}

#[derive(Debug)]
pub struct UnsetSessionData {
    pub keepalive_handle: KeepaliveActorHandle,
    pub negotiated_session: NegotiatedSession,
    pub stun_actor_handle: UnsetSTUNActorHandle,
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
