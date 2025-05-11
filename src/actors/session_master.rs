use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::Duration;

use log::trace;
use rand::random;
use tokio::time::Instant;

use sdp::NegotiatedSession;

use crate::actors::dtls_actor::DTLSActorHandle;
use crate::actors::media_ingest_actor::MediaIngestActorHandle;
use crate::actors::nominated_stun_actor::NominatedSTUNActorHandle;
use crate::actors::receiver_report_actor::ReceiverReportActorHandle;
use crate::actors::session_socket_actor::SessionSocketActorHandle;
use crate::actors::SessionPointer;
use crate::actors::udp_io_actor::UDPIOActorHandle;
use crate::actors::unset_stun_actor::UnsetSTUNActorHandle;
use crate::ice_registry::SessionUsername;

static MAX_TTL: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct SessionMaster {
    nominated_map: NominatedSessionMap,
    room_map: HashMap<usize, HashSet<usize>>,
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

    pub fn prune_stale_sessions(&mut self) {
        // Retain only sessions with TTL < MAX_TTL
        self.nominated_map.session_map.retain(|_, session| {
            let ttl = match session {
                NominatedSession::Streamer(streamer) => &streamer.ttl,
            };
            ttl.elapsed() < MAX_TTL
        });
        self.nominated_map
            .address_map
            .retain(|_, id| self.nominated_map.session_map.get(id).is_some());

        self.unset_map.session_map.retain(|_, session| {
            let ttl = match session {
                UnsetSession::Streamer(streamer) => &streamer.ttl,
                UnsetSession::Viewer(viewer) => &viewer.ttl,
            };
            ttl.elapsed() < MAX_TTL
        });
        self.unset_map
            .ice_username_map
            .retain(|_, id| self.nominated_map.session_map.get(id).is_some());

        self.room_map
            .retain(|id, _| self.nominated_map.session_map.get(id).is_some());
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
            ttl: Instant::now(),
            negotiated_session: negotiated_session.clone(),
            stun_actor_handle: UnsetSTUNActorHandle::new(
                negotiated_session,
                self.socket_io_actor_handle.clone(),
            ),
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

                let nominated_session = NominatedSession::Streamer(StreamerSessionData {
                    ttl: Instant::now(),
                    negotiated_session: session_data.negotiated_session.clone(),
                    media_digest_actor_handle: MediaIngestActorHandle::new(
                        dtls_handle.clone(),
                        rr_handle,
                    ),
                    dtls_actor: dtls_handle,
                    stun_actor_handle: NominatedSTUNActorHandle::new(
                        session_data.negotiated_session,
                        session_socket_handle,
                    ),
                });
                trace!(target: "Session Master", "Created nominated_session {:#?}", nominated_session);

                let id = random::<usize>();
                self.nominated_map.address_map.insert(remote_addr, id);
                self.nominated_map.session_map.insert(id, nominated_session);
                self.room_map.insert(id, HashSet::new());
            }
            UnsetSession::Viewer(_) => {
                // todo support Viewer
            }
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

impl NominatedSession {
    pub fn update_ttl(&mut self) {
        match self {
            NominatedSession::Streamer(streamer) => streamer.ttl = Instant::now(),
        }
    }
}

#[derive(Debug)]
pub enum UnsetSession {
    Streamer(UnsetSessionData),
    Viewer(UnsetSessionData),
}

impl UnsetSession {
    pub fn update_ttl(&mut self) {
        match self {
            UnsetSession::Streamer(streamer) => streamer.ttl = Instant::now(),
            UnsetSession::Viewer(viewer) => viewer.ttl = Instant::now(),
        }
    }
}

#[derive(Debug)]
pub struct UnsetSessionData {
    ttl: Instant,
    pub negotiated_session: NegotiatedSession,
    pub stun_actor_handle: UnsetSTUNActorHandle,
}

#[derive(Debug)]
pub struct StreamerSessionData {
    ttl: Instant,
    pub stun_actor_handle: NominatedSTUNActorHandle,
    pub media_digest_actor_handle: MediaIngestActorHandle,
    pub dtls_actor: DTLSActorHandle,
    pub negotiated_session: NegotiatedSession,
}
