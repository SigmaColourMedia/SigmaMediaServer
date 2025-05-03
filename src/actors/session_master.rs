use std::collections::HashMap;
use std::net::SocketAddr;

use log::debug;
use rand::random;
use tokio::time::Instant;

use sdp::NegotiatedSession;

use crate::actors::{EventProducer, SessionPointer};
use crate::actors::dtls_actor::DTLSActorHandle;
use crate::actors::stun_actor::STUNActorHandle;
use crate::ice_registry::SessionUsername;

pub struct SessionMaster {
    event_producer: EventProducer,
    nominated_map: NominatedSessionMap,
    unset_map: UnsetSessionMap,
}

impl SessionMaster {
    pub fn new(event_producer: EventProducer) -> Self {
        Self {
            event_producer,
            nominated_map: NominatedSessionMap::new(),
            unset_map: UnsetSessionMap::new(),
        }
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
            stun_actor_handle: STUNActorHandle::new(
                self.event_producer.clone(),
                negotiated_session,
            ),
        });
        debug!(target: "Session Master", "Adding unset session for username:{}", session_username.host);

        self.unset_map.ice_username_map.insert(session_username, id);
        self.unset_map.session_map.insert(id, unset_session);
    }

    pub fn nominate_session(&mut self, session_pointer: SessionPointer) {
        let remote_addr = session_pointer.socket_address;

        let unset_session = self
            .unset_map
            .ice_username_map
            .remove(&SessionUsername {
                remote: session_pointer.ice_credentials.remote_username,
                host: session_pointer.ice_credentials.host_username,
            })
            .and_then(|id| self.unset_map.session_map.remove(&id))
            .expect("Attempted to nominate a non-existing session");

        match unset_session {
            UnsetSession::Streamer(session_data) => {
                debug!(target: "Session Master", "Nominating session with username:{} and remote address:{}",session_data.negotiated_session.ice_credentials.host_username, remote_addr);

                let nominated_session = NominatedSession::Streamer(StreamerSessionData {
                    ttl: Instant::now(),
                    negotiated_session: session_data.negotiated_session,
                    dtls_actor: DTLSActorHandle::new(self.event_producer.clone(), remote_addr),
                    stun_actor_handle: session_data.stun_actor_handle,
                });

                let id = random::<usize>();
                self.nominated_map.address_map.insert(remote_addr, id);
                self.nominated_map.session_map.insert(id, nominated_session);
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
    pub stun_actor_handle: STUNActorHandle,
}

#[derive(Debug)]
pub struct StreamerSessionData {
    ttl: Instant,
    pub stun_actor_handle: STUNActorHandle,
    pub dtls_actor: DTLSActorHandle,
    pub negotiated_session: NegotiatedSession,
}
