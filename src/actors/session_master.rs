use std::collections::HashMap;
use std::net::SocketAddr;
use std::ops::Add;

use log::{debug, info, warn};
use rand::random;
use tokio::time::Instant;

use sdp::{ICECredentials, NegotiatedSession};

use crate::actors::MessageEvent;
use crate::actors::stun_actor::STUNActorHandle;
use crate::ice_registry::Streamer;

type SessionID = usize;

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

    pub fn add_streamer(&mut self, negotiated_session: NegotiatedSession) {
        let ice_username = get_ice_username(&negotiated_session.ice_credentials);
        let id = self
            .translator
            .add_session(Session::Streamer(StreamerSession {
                ttl: Instant::now(),
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
    socket_addr: HashMap<SocketAddr, usize>,
    ice_credentials: HashMap<String, usize>,
}

impl SessionAddressTranslator {
    fn new() -> Self {
        Self {
            session_map: HashMap::new(),
            socket_addr: HashMap::new(),
            ice_credentials: HashMap::new(),
        }
    }

    fn add_session(&mut self, session: Session) -> usize {
        let id = random::<usize>();
        self.session_map.insert(id, session);
        id
    }
    fn add_ice_username(&mut self, id: usize, ice_username: String) {
        self.ice_credentials.insert(ice_username, id);
    }

    fn add_socket_address(&mut self, id: usize, socket_addr: SocketAddr) {
        if let Some(_) = self.socket_addr.insert(socket_addr, id) {
            warn!(target: "Session Master", "Attempted to set already existing SocketAddr")
        }
    }
}

enum Session {
    Streamer(StreamerSession),
}

struct StreamerSession {
    ttl: Instant,
    stun_actor_handle: STUNActorHandle,
}

fn get_ice_username(ice_credentials: &ICECredentials) -> String {
    format!(
        "{}-{}",
        ice_credentials.host_username, ice_credentials.remote_username
    )
}
