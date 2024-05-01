use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;
use tokio::time::Instant;

use crate::client::Client;

type ResourceID = String;

pub struct SessionRegistry {
    sessions: HashMap<ResourceID, Session>,
    username_map: HashMap<SessionUsername, ResourceID>,
    address_map: HashMap<SocketAddr, ResourceID>,
    rooms: HashSet<ResourceID>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        SessionRegistry {
            sessions: HashMap::new(),
            username_map: HashMap::new(),
            address_map: HashMap::new(),
            rooms: HashSet::new(),

        }
    }

    pub fn nominate_client(&mut self, client: Client, id: &ResourceID) -> Option<ResourceID> {
        let address = client.remote_address.clone();
        self.sessions.get_mut(id).map(|session| session.client = Some(client)).and_then(|_| {
            self.address_map.insert(address, id.clone());
            Some(id.clone())
        })
    }

    pub fn get_session(&mut self, id: &ResourceID) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }
    pub fn get_session_by_username(&mut self, session_username: &SessionUsername) -> Option<&mut Session> {
        self.username_map.get(session_username).map(|id| self.sessions.get_mut(id)).flatten()
    }

    pub fn get_session_by_address(&mut self, remote_address: &SocketAddr) -> Option<&mut Session> {
        self.address_map.get(remote_address).and_then(|id| self.sessions.get_mut(id))
    }

    pub fn add_streamer(&mut self, streamer: Session) -> Option<ResourceID> {
        let id = streamer.id.clone();
        self.username_map.insert(SessionUsername {
            host: streamer.credentials.host_username.clone(),
            remote: streamer.credentials.remote_username.clone(),
        }, id.clone());
        self.sessions.insert(streamer.id.clone(), streamer);
        self.rooms.insert(id.clone());
        Some(id)
    }

    pub fn add_viewer(&mut self, viewer: Session) -> Option<ResourceID> {
        let id = viewer.id.clone();
        match viewer.connection_type {
            ConnectionType::Viewer(viewer_session) => {
                let target_id = &viewer_session.target_resource;
                self.sessions.get_mut(target_id).and_then(|session| {
                    match &mut session.connection_type {
                        ConnectionType::Viewer(_) => None,
                        ConnectionType::Streamer(streamer) => {
                            self.username_map.insert(SessionUsername {
                                host: viewer.credentials.host_username.clone(),
                                remote: viewer.credentials.remote_username.clone(),
                            }, id.clone());

                            streamer.viewers_ids.push(viewer.id.clone());
                            Some(id)
                        }
                    }
                })
            }
            ConnectionType::Streamer(_) => None,
        }
    }
}

pub struct Session {
    pub id: ResourceID,
    pub ttl: Instant,
    client: Option<Client>,
    credentials: SessionCredentials,
    connection_type: ConnectionType,
}

impl Session {
    pub fn new_streamer(credentials: SessionCredentials) -> Self {
        let mut rng = thread_rng();

        let id: String = rng.sample_iter(Alphanumeric).take(12).map(char::from).collect();

        Session {
            id,
            ttl: Instant::now(),
            client: None,
            credentials,
            connection_type: ConnectionType::Streamer(Streamer {
                viewers_ids: vec![],
            }),
        }
    }

    pub fn new_viewer(target_id: String, credentials: SessionCredentials) -> Self {
        let mut rng = thread_rng();

        let id: String = rng.sample_iter(Alphanumeric).take(12).map(char::from).collect();
        Session {
            id,
            ttl: Instant::now(),
            client: None,
            credentials,
            connection_type: ConnectionType::Viewer(Viewer {
                target_resource: target_id.to_owned()
            }),
        }
    }
}

enum ConnectionType {
    Viewer(Viewer),
    Streamer(Streamer),
}

struct Viewer {
    target_resource: ResourceID,
}

struct Streamer {
    viewers_ids: Vec<ResourceID>,
}

pub struct SessionCredentials {
    remote_username: String,
    host_username: String,
    host_password: String,
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct SessionUsername {
    remote: String,
    host: String,
}