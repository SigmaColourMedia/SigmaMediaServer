use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::Instant;

use sdp2::NegotiatedSession;

use crate::client::Client;
use crate::rnd::get_random_id;

type RoomID = u32;
type ResourceID = u32;
type HostUsername = String;

pub struct SessionRegistry {
    sessions: HashMap<ResourceID, Session>,
    username_map: HashMap<SessionUsername, ResourceID>,
    address_map: HashMap<SocketAddr, ResourceID>,
    rooms: HashMap<RoomID, Room>,
}
#[derive(Clone)]
pub struct Room {
    pub id: u32,
    pub owner_id: u32,
    pub viewer_ids: HashSet<u32>,
}

impl Room {
    pub fn new(id: u32, owner_id: u32) -> Self {
        Self {
            id,
            owner_id,
            viewer_ids: HashSet::new(),
        }
    }
}

impl SessionRegistry {
    pub fn new() -> Self {
        SessionRegistry {
            sessions: HashMap::new(),
            username_map: HashMap::new(),
            address_map: HashMap::new(),
            rooms: HashMap::new(),
        }
    }

    pub fn get_room_ids(&self) -> Vec<RoomID> {
        self.rooms
            .keys()
            .map(|val| val.to_owned())
            .collect::<Vec<_>>()
    }

    pub fn get_rooms(&self) -> Vec<Room> {
        self.rooms.values().map(Clone::clone).collect()
    }

    pub fn get_room(&self, room_id: RoomID) -> Option<&Room> {
        self.rooms.get(&room_id)
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
    pub fn get_all_sessions(&self) -> Vec<&Session> {
        self.sessions.values().collect()
    }

    pub fn remove_session(&mut self, id: ResourceID) {
        let session = self
            .sessions
            .get(&id)
            .expect("Session should be established in order to remove it");

        // Clear username map
        let host_username = session.media_session.ice_credentials.host_username.clone();
        let remote_username = session
            .media_session
            .ice_credentials
            .remote_username
            .clone();
        let session_username = SessionUsername {
            host: host_username,
            remote: remote_username,
        };
        self.username_map.remove(&session_username);

        // Clear address map if applicable
        if let Some(remote) = session.client.as_ref().map(|client| client.remote_address) {
            self.address_map.remove(&remote);
        }

        // Handle Room cleaning
        match &session.connection_type {
            // If viewer and room is not orphaned remove viewer from room viewers
            // Perhaps this should also remove the viewer session? But I don't exactly want this function to modify sessions other than the one pointed by the resource_id
            ConnectionType::Viewer(viewer) => {
                if let Some(target_room) = self.rooms.get_mut(&viewer.room_id) {
                    target_room.viewer_ids.remove(&id);
                }
            }
            // If streamer, remove the room
            ConnectionType::Streamer(streamer) => {
                self.rooms.remove(&streamer.owned_room_id);
            }
        }

        self.sessions.remove(&id);
    }

    pub fn get_session_mut(&mut self, id: ResourceID) -> Option<&mut Session> {
        self.sessions.get_mut(&id)
    }

    pub fn get_session(&self, id: ResourceID) -> Option<&Session> {
        self.sessions.get(&id)
    }
    pub fn get_session_by_username_mut(
        &mut self,
        session_username: &SessionUsername,
    ) -> Option<&mut Session> {
        self.username_map
            .get(session_username)
            .map(|id| self.sessions.get_mut(id))
            .flatten()
    }

    pub fn get_session_by_address_mut(
        &mut self,
        remote_address: &SocketAddr,
    ) -> Option<&mut Session> {
        self.address_map
            .get(remote_address)
            .and_then(|id| self.sessions.get_mut(id))
    }

    pub fn get_session_by_address(&self, remote_address: &SocketAddr) -> Option<&Session> {
        self.address_map
            .get(remote_address)
            .and_then(|id| self.sessions.get(id))
    }

    pub fn add_streamer(&mut self, negotiated_session: NegotiatedSession) -> ResourceID {
        let room_id = get_random_id();

        let streamer_session = Session::new_streamer(negotiated_session, room_id);
        let resource_id = streamer_session.id;
        let host_username = streamer_session
            .media_session
            .ice_credentials
            .host_username
            .clone();
        let remote_username = streamer_session
            .media_session
            .ice_credentials
            .remote_username
            .clone();

        let room = Room::new(room_id, resource_id);

        let session_username = SessionUsername {
            host: host_username,
            remote: remote_username,
        };
        // Update username map
        self.username_map.insert(session_username, resource_id);
        self.rooms.insert(room_id, room); // Update rooms map
        self.sessions.insert(resource_id, streamer_session); // Update sessions map

        resource_id
    }

    pub fn add_viewer(
        &mut self,
        negotiated_session: NegotiatedSession,
        target_room: RoomID,
    ) -> ResourceID {
        let viewer = Session::new_viewer(target_room, negotiated_session);
        let resource_id = viewer.id;

        let host_username = viewer.media_session.ice_credentials.host_username.clone();
        let remote_username = viewer.media_session.ice_credentials.remote_username.clone();
        let session_username = SessionUsername {
            host: host_username,
            remote: remote_username,
        };

        self.username_map.insert(session_username, resource_id);
        self.sessions.insert(resource_id, viewer);
        self.rooms
            .get_mut(&target_room)
            .expect("Target room should be present")
            .viewer_ids
            .insert(resource_id);

        resource_id
    }
}

#[derive(Debug)]
pub struct Session {
    pub id: ResourceID,
    pub ttl: Instant,
    pub client: Option<Client>,
    pub media_session: NegotiatedSession,
    pub connection_type: ConnectionType,
}

impl Session {
    pub fn new_streamer(media_session: NegotiatedSession, room_id: RoomID) -> Self {
        let id = get_random_id();

        Session {
            id,
            ttl: Instant::now(),
            client: None,
            media_session,
            connection_type: ConnectionType::Streamer(Streamer {
                owned_room_id: room_id,
            }),
        }
    }

    pub fn new_viewer(target_id: RoomID, media_session: NegotiatedSession) -> Self {
        let id = get_random_id();
        Session {
            id,
            ttl: Instant::now(),
            client: None,
            media_session,
            connection_type: ConnectionType::Viewer(Viewer { room_id: target_id }),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConnectionType {
    Viewer(Viewer),
    Streamer(Streamer),
}

#[derive(Debug, Clone)]
pub struct Viewer {
    room_id: ResourceID,
}

#[derive(Debug, Clone)]
pub struct Streamer {
    pub owned_room_id: u32,
}

#[derive(Debug)]
pub struct SessionCredentials {
    pub host_username: String,
    pub host_password: String,
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct SessionUsername {
    pub remote: String,
    pub host: String,
}
