use std::collections::HashMap;
use std::net::SocketAddr;

use log::{debug, warn};
use rand::random;
use tokio::time::Instant;

use sdp::NegotiatedSession;

use crate::actors::MessageEvent;
use crate::actors::stun_actor::STUNActorHandle;
use crate::ice_registry::SessionUsername;

pub struct SessionMaster {
    pub master_channel_tx: tokio::sync::mpsc::Sender<MessageEvent>,
    pub master_channel_rx: tokio::sync::mpsc::Receiver<MessageEvent>,
    translator: SessionAddressTranslator,
}

impl SessionMaster {
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<MessageEvent>(1000);
        Self {
            translator: SessionAddressTranslator::new(),
            master_channel_rx: rx,
            master_channel_tx: tx,
        }
    }

    fn nominate_session(&mut self, id: usize, socket_addr: SocketAddr) {
        self.translator.add_socket_address(id, socket_addr)
    }
    pub fn get_session_by_ice_username(&self, username: &SessionUsername) -> Option<&Session> {
        self.translator.get_by_ice_username(username)
    }
    pub fn get_session_by_socket_addr(&self, address: &SocketAddr) -> Option<&Session> {
        self.translator.get_by_socket_addr(address)
    }

    pub fn add_streamer(&mut self, negotiated_session: NegotiatedSession) {
        let ice_username = SessionUsername {
            host: negotiated_session.ice_credentials.host_username.clone(),
            remote: negotiated_session.ice_credentials.remote_username.clone(),
        };
        let id = self
            .translator
            .add_session(Session::Streamer(StreamerSession {
                ttl: Instant::now(),
                negotiated_session: negotiated_session.clone(),
                stun_actor_handle: STUNActorHandle::new(
                    self.master_channel_tx.clone(),
                    negotiated_session,
                ),
            }));
        self.translator.add_ice_username(id, ice_username);
        debug!(target: "Session Master", "Assigning session: {id}")
    }
}

struct SessionAddressTranslator {
    session_map: HashMap<usize, Session>,
    address_map: HashMap<SocketAddr, usize>,
    ice_username_map: HashMap<SessionUsername, usize>,
}

impl SessionAddressTranslator {
    fn new() -> Self {
        Self {
            session_map: HashMap::new(),
            address_map: HashMap::new(),
            ice_username_map: HashMap::new(),
        }
    }

    fn get_by_ice_username(&self, username: &SessionUsername) -> Option<&Session> {
        self.ice_username_map
            .get(username)
            .and_then(|id| self.session_map.get(id))
    }

    fn get_by_socket_addr(&self, address: &SocketAddr) -> Option<&Session> {
        self.address_map
            .get(&address)
            .and_then(|id| self.session_map.get(id))
    }

    fn add_session(&mut self, session: Session) -> usize {
        let id = random::<usize>();
        self.session_map.insert(id, session);
        id
    }
    fn add_ice_username(&mut self, id: usize, ice_username: SessionUsername) {
        self.ice_username_map.insert(ice_username, id);
    }

    fn add_socket_address(&mut self, id: usize, socket_addr: SocketAddr) {
        if let Some(_) = self.address_map.insert(socket_addr, id) {
            warn!(target: "Session Master", "Attempted to set already existing SocketAddr")
        }
    }
}

pub enum Session {
    Streamer(StreamerSession),
}

pub struct StreamerSession {
    ttl: Instant,
    pub stun_actor_handle: STUNActorHandle,
    pub negotiated_session: NegotiatedSession,
}
