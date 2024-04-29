use std::collections::{HashMap, HashSet};

use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;

type ResourceID = String;

struct IceRegistry {
    sessions: HashMap<ResourceID, Session>,
    rooms: HashSet<ResourceID>,
}

impl IceRegistry {
    pub fn new() -> Self {
        IceRegistry {
            sessions: HashMap::new(),
            rooms: HashSet::new(),
        }
    }

    fn add_streamer(&mut self, credentials: SessionCredentials) -> Option<ResourceID> {
        let streamer = Session::new_streamer(credentials);
        let id = streamer.id.clone();
        self.sessions.insert(streamer.id.clone(), streamer);
        self.rooms.insert(id.clone());
        Some(id)
    }

    fn add_viewer(&mut self, target_id: &str, credentials: SessionCredentials) -> Option<ResourceID> {
        let viewer = Session::new_viewer(target_id, credentials);

        self.sessions.get_mut(target_id).map(|session| {
            match &mut session.connection_type {
                ConnectionType::Viewer(_) => None,
                ConnectionType::Streamer(streamer) => {
                    streamer.viewers_ids.push(viewer.id.clone());
                    Some(viewer.id.clone())
                }
            }
        }).flatten()
    }
}

struct Session {
    pub id: ResourceID,
    credentials: SessionCredentials,
    connection_type: ConnectionType,
}

impl Session {
    pub fn new_streamer(credentials: SessionCredentials) -> Self {
        let mut rng = thread_rng();

        let id: String = rng.sample_iter(Alphanumeric).take(12).map(char::from).collect();

        Session {
            id,
            credentials,
            connection_type: ConnectionType::Streamer(Streamer {
                viewers_ids: vec![],
            }),
        }
    }

    pub fn new_viewer(target_id: &str, credentials: SessionCredentials) -> Self {
        let mut rng = thread_rng();
        let id: String = rng.sample_iter(Alphanumeric).take(12).map(char::from).collect();

        Session {
            id,
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