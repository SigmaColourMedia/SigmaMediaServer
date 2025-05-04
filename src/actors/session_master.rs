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
use crate::actors::SessionPointer;
use crate::actors::unset_stun_actor::UnsetSTUNActorHandle;
use crate::ice_registry::SessionUsername;

static MAX_TTL: Duration = Duration::from_secs(5);

pub struct SessionMaster {
    nominated_map: NominatedSessionMap,
    room_map: HashMap<usize, HashSet<usize>>,
    unset_map: UnsetSessionMap,
}

impl SessionMaster {
    pub fn new() -> Self {
        Self {
            nominated_map: NominatedSessionMap::new(),
            unset_map: UnsetSessionMap::new(),
            room_map: HashMap::new(),
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
    }

    pub fn get_unset_session(&self, session_username: &SessionUsername) -> Option<&UnsetSession> {
        self.unset_map
            .ice_username_map
            .get(session_username)
            .and_then(|id| self.unset_map.session_map.get(id))
    }

    pub fn get_session(&self, remote_addr: &SocketAddr) -> Option<&NominatedSession> {
        self.nominated_map
            .address_map
            .get(remote_addr)
            .and_then(|id| self.nominated_map.session_map.get(id))
    }
    pub fn add_streamer(&mut self, negotiated_session: NegotiatedSession) {
        let session_username = SessionUsername {
            host: negotiated_session.ice_credentials.host_username.clone(),
            remote: negotiated_session.ice_credentials.remote_username.clone(),
        };
        let id = random::<usize>();
        let unset_session = UnsetSession::Streamer(UnsetSessionData {
            ttl: Instant::now(),
            negotiated_session: negotiated_session.clone(),
            stun_actor_handle: UnsetSTUNActorHandle::new(negotiated_session),
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
                let dtls_handle = DTLSActorHandle::new(remote_addr);
                let rr_handle = ReceiverReportActorHandle::new(
                    &session_data.negotiated_session,
                    remote_addr,
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
