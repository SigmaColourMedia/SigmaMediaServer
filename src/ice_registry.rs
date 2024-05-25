use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use tokio::time::Instant;

use crate::client::Client;
use crate::rnd::get_random_string;
use crate::sdp::SDP;

type ResourceID = String;

pub struct SessionRegistry {
    sessions: HashMap<ResourceID, Session>,
    username_map: HashMap<UsernameKey, ResourceID>,
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

    pub fn get_rooms(&self) -> Vec<String> {
        self.rooms.clone().into_iter().collect::<Vec<String>>()
    }

    pub fn nominate_client(&mut self, client: Client, id: &ResourceID) -> Option<ResourceID> {
        let address = client.remote_address.clone();
        self.sessions
            .get_mut(id)
            .map(|session| session.client = Some(client))
            .and_then(|_| {
                self.address_map.insert(address, id.clone());
                Some(id.clone())
            })
    }

    pub fn get_session(&self, id: &ResourceID) -> Option<&Session> {
        self.sessions.get(id)
    }
    pub fn get_session_by_username(&self, session_username: &UsernameKey) -> Option<&Session> {
        self.username_map
            .get(session_username)
            .map(|id| self.sessions.get(id))
            .flatten()
    }

    pub fn get_session_by_address(&mut self, remote_address: &SocketAddr) -> Option<&mut Session> {
        self.address_map
            .get(remote_address)
            .and_then(|id| self.sessions.get_mut(id))
    }

    pub fn add_streamer(&mut self, streamer: Session) -> Option<ResourceID> {
        let id = streamer.id.clone();

        // Update username map
        self.username_map.insert(
            UsernameKey {
                host: streamer.credentials.host_username.clone(),
            },
            id.clone(),
        );
        self.sessions.insert(streamer.id.clone(), streamer); // Update sessions map
        self.rooms.insert(id.clone()); // Update rooms map

        Some(id)
    }

    pub fn add_viewer(&mut self, viewer: Session) -> Option<ResourceID> {
        let id = viewer.id.clone();

        match &viewer.connection_type {
            ConnectionType::Viewer(viewer_session) => {
                self.sessions
                    .get_mut(&viewer_session.target_resource)
                    .and_then(|session| {
                        match &mut session.connection_type {
                            ConnectionType::Streamer(streamer) => {
                                // Add viewer to streamer's room
                                streamer.viewers_ids.push(id.to_owned());
                                Some(())
                            }
                            ConnectionType::Viewer(_) => None,
                        }
                    })
            }
            ConnectionType::Streamer(_) => None,
        }
        .map(|_| {
            // Update username map
            self.username_map.insert(
                UsernameKey {
                    host: viewer.credentials.host_username.clone(),
                },
                id.to_owned(),
            );

            // Update sessions Hashmap
            self.sessions.insert(id.to_owned(), viewer);
            id.clone()
        })
    }
}

#[derive(Debug)]
pub struct Session {
    pub id: ResourceID,
    pub ttl: Instant,
    pub client: Option<Client>,
    pub credentials: SessionCredentials,
    pub connection_type: ConnectionType,
}

impl Session {
    pub fn new_streamer(credentials: SessionCredentials, sdp: SDP) -> Self {
        let id = get_random_string(12);

        Session {
            id,
            ttl: Instant::now(),
            client: None,
            credentials,
            connection_type: ConnectionType::Streamer(Streamer {
                viewers_ids: vec![],
                sdp,
            }),
        }
    }

    pub fn new_viewer(target_id: String, credentials: SessionCredentials) -> Self {
        let id = get_random_string(12);
        Session {
            id,
            ttl: Instant::now(),
            client: None,
            credentials,
            connection_type: ConnectionType::Viewer(Viewer {
                target_resource: target_id.to_owned(),
            }),
        }
    }
}

#[derive(Debug)]
pub enum ConnectionType {
    Viewer(Viewer),
    Streamer(Streamer),
}

#[derive(Debug)]
pub struct Viewer {
    target_resource: ResourceID,
}

#[derive(Debug)]
pub struct Streamer {
    pub viewers_ids: Vec<ResourceID>,
    pub sdp: SDP,
}

#[derive(Debug)]
pub struct SessionCredentials {
    pub host_username: String,
    pub host_password: String,
}
#[derive(Hash, Eq, PartialEq, Debug)]
pub struct UsernameKey {
    pub host: String,
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct SessionUsername {
    pub remote: String,
    pub host: String,
}
