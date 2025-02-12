use std::cmp::{Ordering};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::net::SocketAddr;
use std::time::Instant;


use rand::{RngCore, thread_rng};
use rtcp::transport_layer_feedback::{GenericNACK, TransportLayerNACK};

use sdp::{NegotiatedSession, VideoSession};
use thumbnail_image_extractor::ThumbnailExtractor;

use crate::client::Client;
use crate::rtcp_reporter::Reporter;

type RoomID = u32;
type ResourceID = u32;

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

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct SessionUsername {
    pub remote: String,
    pub host: String,
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

    pub fn get_room(&mut self, room_id: RoomID) -> Option<&Room> {
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
    pub fn get_all_sessions_mut(&mut self) -> Vec<&mut Session> {
        self.sessions.values_mut().collect()
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
    pub video_reporter: Option<Reporter>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            ttl: Instant::now(),
            id: 1,
            client: None,
            connection_type: ConnectionType::Viewer(Viewer { room_id: 1 }),
            media_session: NegotiatedSession::default(),
            video_reporter: None,
        }
    }
}

impl Session {
    pub fn new_streamer(media_session: NegotiatedSession, room_id: RoomID) -> Self {
        let id = get_random_id();

        Session {
            id,
            ttl: Instant::now(),
            client: None,
            media_session,
            video_reporter: None,
            connection_type: ConnectionType::Streamer(Streamer {
                owned_room_id: room_id,
                thumbnail_extractor: ThumbnailExtractor::new(),
                image_timestamp: None,
            }),
        }
    }

    pub fn new_viewer(target_id: RoomID, media_session: NegotiatedSession) -> Self {
        let id = get_random_id();
        Session {
            id,
            ttl: Instant::now(),
            client: None,
            video_reporter: None,
            media_session,
            connection_type: ConnectionType::Viewer(Viewer { room_id: target_id }),
        }
    }

    pub fn process_packet(&mut self, pid: usize, roc: usize) {
        self.video_reporter.as_mut().expect("Video Reporter should be Some").process_packet(pid, roc);
    }

    pub fn set_reporter(&mut self, pid: usize, roc: usize) {
        self.video_reporter = Some(Reporter::new(pid, roc));
    }


    pub fn check_packet_integrity(&mut self) -> Option<TransportLayerNACK> {
        if self.video_reporter.is_none() {
            return None;
        }
        let reporter = self.video_reporter.as_mut().unwrap();

        let packets_to_report = reporter.lost_packets.iter().filter(|&&pid| {
            pid.abs_diff(reporter.ext_highest_seq) < 528
        }).collect::<Vec<&usize>>();

        if packets_to_report.is_empty() {
            return None;
        }

        let nacks = packets_to_report.into_iter().map(|&pid| GenericNACK { pid: pid as u16, blp: 0 }).collect();

        Some(TransportLayerNACK::new(nacks, self.media_session.video_session.host_ssrc, self.media_session.video_session.remote_ssrc.unwrap_or(0)))
    }
}

#[derive(Debug, Clone)]
pub enum ConnectionType {
    Viewer(Viewer),
    Streamer(Streamer),
}

#[derive(Debug, Clone)]
pub struct Viewer {
    pub room_id: ResourceID,
}

#[derive(Debug, Clone)]
pub struct Streamer {
    pub owned_room_id: u32,
    pub thumbnail_extractor: ThumbnailExtractor,
    pub image_timestamp: Option<Instant>,
}


fn get_random_id() -> u32 {
    thread_rng().next_u32()
}


#[cfg(test)]
mod session_tests {
    use std::time::Instant;
    use rtcp::transport_layer_feedback::GenericNACK;
    use sdp::NegotiatedSession;
    use thumbnail_image_extractor::ThumbnailExtractor;
    use crate::ice_registry::{ConnectionType, Session, Streamer};
    use crate::rtcp_reporter::Reporter;

    #[test]
    fn creates_nack_packet() {
        let mut session = Session {
            video_reporter: Some(Reporter::new(0, 0)),
            media_session: NegotiatedSession::default(),
            ttl: Instant::now(),
            id: 1,
            client: None,
            connection_type: ConnectionType::Streamer(Streamer {
                owned_room_id: 1,
                thumbnail_extractor: ThumbnailExtractor::new(),
                image_timestamp: None,
            }),
        };

        session.video_reporter.process_packet(1, 0);
        session.video_reporter.process_packet(2, 0);
        session.video_reporter.process_packet(5, 0);

        let mut output_nack = session.check_packet_integrity().unwrap();
        let mut expected_nacks = vec![GenericNACK { pid: 3, blp: 0 }, GenericNACK { pid: 4, blp: 0 }];


        // Sort NACKs by PID
        expected_nacks.sort_by(|field, field_2| field.pid.partial_cmp(&field_2.pid).unwrap());
        output_nack.nacks.sort_by(|field, field_2| field.pid.partial_cmp(&field_2.pid).unwrap());

        assert_eq!(output_nack.media_ssrc, session.media_session.video_session.remote_ssrc.unwrap());
        assert_eq!(output_nack.media_ssrc, session.media_session.video_session.host_ssrc);
        assert_eq!(output_nack.nacks, expected_nacks)
    }

    #[test]
    fn does_not_create_nack_packet() {
        let mut session = Session {
            video_reporter: Some(Reporter::new(0, 0)),
            media_session: NegotiatedSession::default(),
            ttl: Instant::now(),
            id: 1,
            client: None,
            connection_type: ConnectionType::Streamer(Streamer {
                owned_room_id: 1,
                thumbnail_extractor: ThumbnailExtractor::new(),
                image_timestamp: None,
            }),
        };

        session.video_reporter.process_packet(1, 0);
        session.video_reporter.process_packet(2, 0);

        let mut output_nack = session.check_packet_integrity();

        assert!(output_nack.is_none())
    }

    #[test]
    fn discards_old_packets() {
        let mut session = Session {
            video_reporter: Some(Reporter::new(0, 0)),
            media_session: NegotiatedSession::default(),
            ttl: Instant::now(),
            id: 1,
            client: None,
            connection_type: ConnectionType::Streamer(Streamer {
                owned_room_id: 1,
                thumbnail_extractor: ThumbnailExtractor::new(),
                image_timestamp: None,
            }),
        };

        session.video_reporter.process_packet(1, 0);
        session.video_reporter.process_packet(2, 0);
        session.video_reporter.process_packet(u16::MAX - 1, 0);
        session.video_reporter.process_packet(u16::MAX, 0);

        let mut output_nack = session.check_packet_integrity().unwrap();
        let mut expected_nacks = vec![GenericNACK { pid: 65024, blp: 0 }, GenericNACK { pid: 65025, blp: 0 }, GenericNACK { pid: 65026, blp: 0 }, GenericNACK { pid: 65027, blp: 0 }, GenericNACK { pid: 65028, blp: 0 }, GenericNACK { pid: 65029, blp: 0 }, GenericNACK { pid: 65030, blp: 0 }, GenericNACK { pid: 65031, blp: 0 }, GenericNACK { pid: 65032, blp: 0 }, GenericNACK { pid: 65033, blp: 0 }, GenericNACK { pid: 65034, blp: 0 }, GenericNACK { pid: 65035, blp: 0 }, GenericNACK { pid: 65036, blp: 0 }, GenericNACK { pid: 65037, blp: 0 }, GenericNACK { pid: 65038, blp: 0 }, GenericNACK { pid: 65039, blp: 0 }, GenericNACK { pid: 65040, blp: 0 }, GenericNACK { pid: 65041, blp: 0 }, GenericNACK { pid: 65042, blp: 0 }, GenericNACK { pid: 65043, blp: 0 }, GenericNACK { pid: 65044, blp: 0 }, GenericNACK { pid: 65045, blp: 0 }, GenericNACK { pid: 65046, blp: 0 }, GenericNACK { pid: 65047, blp: 0 }, GenericNACK { pid: 65048, blp: 0 }, GenericNACK { pid: 65049, blp: 0 }, GenericNACK { pid: 65050, blp: 0 }, GenericNACK { pid: 65051, blp: 0 }, GenericNACK { pid: 65052, blp: 0 }, GenericNACK { pid: 65053, blp: 0 }, GenericNACK { pid: 65054, blp: 0 }, GenericNACK { pid: 65055, blp: 0 }, GenericNACK { pid: 65056, blp: 0 }, GenericNACK { pid: 65057, blp: 0 }, GenericNACK { pid: 65058, blp: 0 }, GenericNACK { pid: 65059, blp: 0 }, GenericNACK { pid: 65060, blp: 0 }, GenericNACK { pid: 65061, blp: 0 }, GenericNACK { pid: 65062, blp: 0 }, GenericNACK { pid: 65063, blp: 0 }, GenericNACK { pid: 65064, blp: 0 }, GenericNACK { pid: 65065, blp: 0 }, GenericNACK { pid: 65066, blp: 0 }, GenericNACK { pid: 65067, blp: 0 }, GenericNACK { pid: 65068, blp: 0 }, GenericNACK { pid: 65069, blp: 0 }, GenericNACK { pid: 65070, blp: 0 }, GenericNACK { pid: 65071, blp: 0 }, GenericNACK { pid: 65072, blp: 0 }, GenericNACK { pid: 65073, blp: 0 }, GenericNACK { pid: 65074, blp: 0 }, GenericNACK { pid: 65075, blp: 0 }, GenericNACK { pid: 65076, blp: 0 }, GenericNACK { pid: 65077, blp: 0 }, GenericNACK { pid: 65078, blp: 0 }, GenericNACK { pid: 65079, blp: 0 }, GenericNACK { pid: 65080, blp: 0 }, GenericNACK { pid: 65081, blp: 0 }, GenericNACK { pid: 65082, blp: 0 }, GenericNACK { pid: 65083, blp: 0 }, GenericNACK { pid: 65084, blp: 0 }, GenericNACK { pid: 65085, blp: 0 }, GenericNACK { pid: 65086, blp: 0 }, GenericNACK { pid: 65087, blp: 0 }, GenericNACK { pid: 65088, blp: 0 }, GenericNACK { pid: 65089, blp: 0 }, GenericNACK { pid: 65090, blp: 0 }, GenericNACK { pid: 65091, blp: 0 }, GenericNACK { pid: 65092, blp: 0 }, GenericNACK { pid: 65093, blp: 0 }, GenericNACK { pid: 65094, blp: 0 }, GenericNACK { pid: 65095, blp: 0 }, GenericNACK { pid: 65096, blp: 0 }, GenericNACK { pid: 65097, blp: 0 }, GenericNACK { pid: 65098, blp: 0 }, GenericNACK { pid: 65099, blp: 0 }, GenericNACK { pid: 65100, blp: 0 }, GenericNACK { pid: 65101, blp: 0 }, GenericNACK { pid: 65102, blp: 0 }, GenericNACK { pid: 65103, blp: 0 }, GenericNACK { pid: 65104, blp: 0 }, GenericNACK { pid: 65105, blp: 0 }, GenericNACK { pid: 65106, blp: 0 }, GenericNACK { pid: 65107, blp: 0 }, GenericNACK { pid: 65108, blp: 0 }, GenericNACK { pid: 65109, blp: 0 }, GenericNACK { pid: 65110, blp: 0 }, GenericNACK { pid: 65111, blp: 0 }, GenericNACK { pid: 65112, blp: 0 }, GenericNACK { pid: 65113, blp: 0 }, GenericNACK { pid: 65114, blp: 0 }, GenericNACK { pid: 65115, blp: 0 }, GenericNACK { pid: 65116, blp: 0 }, GenericNACK { pid: 65117, blp: 0 }, GenericNACK { pid: 65118, blp: 0 }, GenericNACK { pid: 65119, blp: 0 }, GenericNACK { pid: 65120, blp: 0 }, GenericNACK { pid: 65121, blp: 0 }, GenericNACK { pid: 65122, blp: 0 }, GenericNACK { pid: 65123, blp: 0 }, GenericNACK { pid: 65124, blp: 0 }, GenericNACK { pid: 65125, blp: 0 }, GenericNACK { pid: 65126, blp: 0 }, GenericNACK { pid: 65127, blp: 0 }, GenericNACK { pid: 65128, blp: 0 }, GenericNACK { pid: 65129, blp: 0 }, GenericNACK { pid: 65130, blp: 0 }, GenericNACK { pid: 65131, blp: 0 }, GenericNACK { pid: 65132, blp: 0 }, GenericNACK { pid: 65133, blp: 0 }, GenericNACK { pid: 65134, blp: 0 }, GenericNACK { pid: 65135, blp: 0 }, GenericNACK { pid: 65136, blp: 0 }, GenericNACK { pid: 65137, blp: 0 }, GenericNACK { pid: 65138, blp: 0 }, GenericNACK { pid: 65139, blp: 0 }, GenericNACK { pid: 65140, blp: 0 }, GenericNACK { pid: 65141, blp: 0 }, GenericNACK { pid: 65142, blp: 0 }, GenericNACK { pid: 65143, blp: 0 }, GenericNACK { pid: 65144, blp: 0 }, GenericNACK { pid: 65145, blp: 0 }, GenericNACK { pid: 65146, blp: 0 }, GenericNACK { pid: 65147, blp: 0 }, GenericNACK { pid: 65148, blp: 0 }, GenericNACK { pid: 65149, blp: 0 }, GenericNACK { pid: 65150, blp: 0 }, GenericNACK { pid: 65151, blp: 0 }, GenericNACK { pid: 65152, blp: 0 }, GenericNACK { pid: 65153, blp: 0 }, GenericNACK { pid: 65154, blp: 0 }, GenericNACK { pid: 65155, blp: 0 }, GenericNACK { pid: 65156, blp: 0 }, GenericNACK { pid: 65157, blp: 0 }, GenericNACK { pid: 65158, blp: 0 }, GenericNACK { pid: 65159, blp: 0 }, GenericNACK { pid: 65160, blp: 0 }, GenericNACK { pid: 65161, blp: 0 }, GenericNACK { pid: 65162, blp: 0 }, GenericNACK { pid: 65163, blp: 0 }, GenericNACK { pid: 65164, blp: 0 }, GenericNACK { pid: 65165, blp: 0 }, GenericNACK { pid: 65166, blp: 0 }, GenericNACK { pid: 65167, blp: 0 }, GenericNACK { pid: 65168, blp: 0 }, GenericNACK { pid: 65169, blp: 0 }, GenericNACK { pid: 65170, blp: 0 }, GenericNACK { pid: 65171, blp: 0 }, GenericNACK { pid: 65172, blp: 0 }, GenericNACK { pid: 65173, blp: 0 }, GenericNACK { pid: 65174, blp: 0 }, GenericNACK { pid: 65175, blp: 0 }, GenericNACK { pid: 65176, blp: 0 }, GenericNACK { pid: 65177, blp: 0 }, GenericNACK { pid: 65178, blp: 0 }, GenericNACK { pid: 65179, blp: 0 }, GenericNACK { pid: 65180, blp: 0 }, GenericNACK { pid: 65181, blp: 0 }, GenericNACK { pid: 65182, blp: 0 }, GenericNACK { pid: 65183, blp: 0 }, GenericNACK { pid: 65184, blp: 0 }, GenericNACK { pid: 65185, blp: 0 }, GenericNACK { pid: 65186, blp: 0 }, GenericNACK { pid: 65187, blp: 0 }, GenericNACK { pid: 65188, blp: 0 }, GenericNACK { pid: 65189, blp: 0 }, GenericNACK { pid: 65190, blp: 0 }, GenericNACK { pid: 65191, blp: 0 }, GenericNACK { pid: 65192, blp: 0 }, GenericNACK { pid: 65193, blp: 0 }, GenericNACK { pid: 65194, blp: 0 }, GenericNACK { pid: 65195, blp: 0 }, GenericNACK { pid: 65196, blp: 0 }, GenericNACK { pid: 65197, blp: 0 }, GenericNACK { pid: 65198, blp: 0 }, GenericNACK { pid: 65199, blp: 0 }, GenericNACK { pid: 65200, blp: 0 }, GenericNACK { pid: 65201, blp: 0 }, GenericNACK { pid: 65202, blp: 0 }, GenericNACK { pid: 65203, blp: 0 }, GenericNACK { pid: 65204, blp: 0 }, GenericNACK { pid: 65205, blp: 0 }, GenericNACK { pid: 65206, blp: 0 }, GenericNACK { pid: 65207, blp: 0 }, GenericNACK { pid: 65208, blp: 0 }, GenericNACK { pid: 65209, blp: 0 }, GenericNACK { pid: 65210, blp: 0 }, GenericNACK { pid: 65211, blp: 0 }, GenericNACK { pid: 65212, blp: 0 }, GenericNACK { pid: 65213, blp: 0 }, GenericNACK { pid: 65214, blp: 0 }, GenericNACK { pid: 65215, blp: 0 }, GenericNACK { pid: 65216, blp: 0 }, GenericNACK { pid: 65217, blp: 0 }, GenericNACK { pid: 65218, blp: 0 }, GenericNACK { pid: 65219, blp: 0 }, GenericNACK { pid: 65220, blp: 0 }, GenericNACK { pid: 65221, blp: 0 }, GenericNACK { pid: 65222, blp: 0 }, GenericNACK { pid: 65223, blp: 0 }, GenericNACK { pid: 65224, blp: 0 }, GenericNACK { pid: 65225, blp: 0 }, GenericNACK { pid: 65226, blp: 0 }, GenericNACK { pid: 65227, blp: 0 }, GenericNACK { pid: 65228, blp: 0 }, GenericNACK { pid: 65229, blp: 0 }, GenericNACK { pid: 65230, blp: 0 }, GenericNACK { pid: 65231, blp: 0 }, GenericNACK { pid: 65232, blp: 0 }, GenericNACK { pid: 65233, blp: 0 }, GenericNACK { pid: 65234, blp: 0 }, GenericNACK { pid: 65235, blp: 0 }, GenericNACK { pid: 65236, blp: 0 }, GenericNACK { pid: 65237, blp: 0 }, GenericNACK { pid: 65238, blp: 0 }, GenericNACK { pid: 65239, blp: 0 }, GenericNACK { pid: 65240, blp: 0 }, GenericNACK { pid: 65241, blp: 0 }, GenericNACK { pid: 65242, blp: 0 }, GenericNACK { pid: 65243, blp: 0 }, GenericNACK { pid: 65244, blp: 0 }, GenericNACK { pid: 65245, blp: 0 }, GenericNACK { pid: 65246, blp: 0 }, GenericNACK { pid: 65247, blp: 0 }, GenericNACK { pid: 65248, blp: 0 }, GenericNACK { pid: 65249, blp: 0 }, GenericNACK { pid: 65250, blp: 0 }, GenericNACK { pid: 65251, blp: 0 }, GenericNACK { pid: 65252, blp: 0 }, GenericNACK { pid: 65253, blp: 0 }, GenericNACK { pid: 65254, blp: 0 }, GenericNACK { pid: 65255, blp: 0 }, GenericNACK { pid: 65256, blp: 0 }, GenericNACK { pid: 65257, blp: 0 }, GenericNACK { pid: 65258, blp: 0 }, GenericNACK { pid: 65259, blp: 0 }, GenericNACK { pid: 65260, blp: 0 }, GenericNACK { pid: 65261, blp: 0 }, GenericNACK { pid: 65262, blp: 0 }, GenericNACK { pid: 65263, blp: 0 }, GenericNACK { pid: 65264, blp: 0 }, GenericNACK { pid: 65265, blp: 0 }, GenericNACK { pid: 65266, blp: 0 }, GenericNACK { pid: 65267, blp: 0 }, GenericNACK { pid: 65268, blp: 0 }, GenericNACK { pid: 65269, blp: 0 }, GenericNACK { pid: 65270, blp: 0 }, GenericNACK { pid: 65271, blp: 0 }, GenericNACK { pid: 65272, blp: 0 }, GenericNACK { pid: 65273, blp: 0 }, GenericNACK { pid: 65274, blp: 0 }, GenericNACK { pid: 65275, blp: 0 }, GenericNACK { pid: 65276, blp: 0 }, GenericNACK { pid: 65277, blp: 0 }, GenericNACK { pid: 65278, blp: 0 }, GenericNACK { pid: 65279, blp: 0 }, GenericNACK { pid: 65280, blp: 0 }, GenericNACK { pid: 65281, blp: 0 }, GenericNACK { pid: 65282, blp: 0 }, GenericNACK { pid: 65283, blp: 0 }, GenericNACK { pid: 65284, blp: 0 }, GenericNACK { pid: 65285, blp: 0 }, GenericNACK { pid: 65286, blp: 0 }, GenericNACK { pid: 65287, blp: 0 }, GenericNACK { pid: 65288, blp: 0 }, GenericNACK { pid: 65289, blp: 0 }, GenericNACK { pid: 65290, blp: 0 }, GenericNACK { pid: 65291, blp: 0 }, GenericNACK { pid: 65292, blp: 0 }, GenericNACK { pid: 65293, blp: 0 }, GenericNACK { pid: 65294, blp: 0 }, GenericNACK { pid: 65295, blp: 0 }, GenericNACK { pid: 65296, blp: 0 }, GenericNACK { pid: 65297, blp: 0 }, GenericNACK { pid: 65298, blp: 0 }, GenericNACK { pid: 65299, blp: 0 }, GenericNACK { pid: 65300, blp: 0 }, GenericNACK { pid: 65301, blp: 0 }, GenericNACK { pid: 65302, blp: 0 }, GenericNACK { pid: 65303, blp: 0 }, GenericNACK { pid: 65304, blp: 0 }, GenericNACK { pid: 65305, blp: 0 }, GenericNACK { pid: 65306, blp: 0 }, GenericNACK { pid: 65307, blp: 0 }, GenericNACK { pid: 65308, blp: 0 }, GenericNACK { pid: 65309, blp: 0 }, GenericNACK { pid: 65310, blp: 0 }, GenericNACK { pid: 65311, blp: 0 }, GenericNACK { pid: 65312, blp: 0 }, GenericNACK { pid: 65313, blp: 0 }, GenericNACK { pid: 65314, blp: 0 }, GenericNACK { pid: 65315, blp: 0 }, GenericNACK { pid: 65316, blp: 0 }, GenericNACK { pid: 65317, blp: 0 }, GenericNACK { pid: 65318, blp: 0 }, GenericNACK { pid: 65319, blp: 0 }, GenericNACK { pid: 65320, blp: 0 }, GenericNACK { pid: 65321, blp: 0 }, GenericNACK { pid: 65322, blp: 0 }, GenericNACK { pid: 65323, blp: 0 }, GenericNACK { pid: 65324, blp: 0 }, GenericNACK { pid: 65325, blp: 0 }, GenericNACK { pid: 65326, blp: 0 }, GenericNACK { pid: 65327, blp: 0 }, GenericNACK { pid: 65328, blp: 0 }, GenericNACK { pid: 65329, blp: 0 }, GenericNACK { pid: 65330, blp: 0 }, GenericNACK { pid: 65331, blp: 0 }, GenericNACK { pid: 65332, blp: 0 }, GenericNACK { pid: 65333, blp: 0 }, GenericNACK { pid: 65334, blp: 0 }, GenericNACK { pid: 65335, blp: 0 }, GenericNACK { pid: 65336, blp: 0 }, GenericNACK { pid: 65337, blp: 0 }, GenericNACK { pid: 65338, blp: 0 }, GenericNACK { pid: 65339, blp: 0 }, GenericNACK { pid: 65340, blp: 0 }, GenericNACK { pid: 65341, blp: 0 }, GenericNACK { pid: 65342, blp: 0 }, GenericNACK { pid: 65343, blp: 0 }, GenericNACK { pid: 65344, blp: 0 }, GenericNACK { pid: 65345, blp: 0 }, GenericNACK { pid: 65346, blp: 0 }, GenericNACK { pid: 65347, blp: 0 }, GenericNACK { pid: 65348, blp: 0 }, GenericNACK { pid: 65349, blp: 0 }, GenericNACK { pid: 65350, blp: 0 }, GenericNACK { pid: 65351, blp: 0 }, GenericNACK { pid: 65352, blp: 0 }, GenericNACK { pid: 65353, blp: 0 }, GenericNACK { pid: 65354, blp: 0 }, GenericNACK { pid: 65355, blp: 0 }, GenericNACK { pid: 65356, blp: 0 }, GenericNACK { pid: 65357, blp: 0 }, GenericNACK { pid: 65358, blp: 0 }, GenericNACK { pid: 65359, blp: 0 }, GenericNACK { pid: 65360, blp: 0 }, GenericNACK { pid: 65361, blp: 0 }, GenericNACK { pid: 65362, blp: 0 }, GenericNACK { pid: 65363, blp: 0 }, GenericNACK { pid: 65364, blp: 0 }, GenericNACK { pid: 65365, blp: 0 }, GenericNACK { pid: 65366, blp: 0 }, GenericNACK { pid: 65367, blp: 0 }, GenericNACK { pid: 65368, blp: 0 }, GenericNACK { pid: 65369, blp: 0 }, GenericNACK { pid: 65370, blp: 0 }, GenericNACK { pid: 65371, blp: 0 }, GenericNACK { pid: 65372, blp: 0 }, GenericNACK { pid: 65373, blp: 0 }, GenericNACK { pid: 65374, blp: 0 }, GenericNACK { pid: 65375, blp: 0 }, GenericNACK { pid: 65376, blp: 0 }, GenericNACK { pid: 65377, blp: 0 }, GenericNACK { pid: 65378, blp: 0 }, GenericNACK { pid: 65379, blp: 0 }, GenericNACK { pid: 65380, blp: 0 }, GenericNACK { pid: 65381, blp: 0 }, GenericNACK { pid: 65382, blp: 0 }, GenericNACK { pid: 65383, blp: 0 }, GenericNACK { pid: 65384, blp: 0 }, GenericNACK { pid: 65385, blp: 0 }, GenericNACK { pid: 65386, blp: 0 }, GenericNACK { pid: 65387, blp: 0 }, GenericNACK { pid: 65388, blp: 0 }, GenericNACK { pid: 65389, blp: 0 }, GenericNACK { pid: 65390, blp: 0 }, GenericNACK { pid: 65391, blp: 0 }, GenericNACK { pid: 65392, blp: 0 }, GenericNACK { pid: 65393, blp: 0 }, GenericNACK { pid: 65394, blp: 0 }, GenericNACK { pid: 65395, blp: 0 }, GenericNACK { pid: 65396, blp: 0 }, GenericNACK { pid: 65397, blp: 0 }, GenericNACK { pid: 65398, blp: 0 }, GenericNACK { pid: 65399, blp: 0 }, GenericNACK { pid: 65400, blp: 0 }, GenericNACK { pid: 65401, blp: 0 }, GenericNACK { pid: 65402, blp: 0 }, GenericNACK { pid: 65403, blp: 0 }, GenericNACK { pid: 65404, blp: 0 }, GenericNACK { pid: 65405, blp: 0 }, GenericNACK { pid: 65406, blp: 0 }, GenericNACK { pid: 65407, blp: 0 }, GenericNACK { pid: 65408, blp: 0 }, GenericNACK { pid: 65409, blp: 0 }, GenericNACK { pid: 65410, blp: 0 }, GenericNACK { pid: 65411, blp: 0 }, GenericNACK { pid: 65412, blp: 0 }, GenericNACK { pid: 65413, blp: 0 }, GenericNACK { pid: 65414, blp: 0 }, GenericNACK { pid: 65415, blp: 0 }, GenericNACK { pid: 65416, blp: 0 }, GenericNACK { pid: 65417, blp: 0 }, GenericNACK { pid: 65418, blp: 0 }, GenericNACK { pid: 65419, blp: 0 }, GenericNACK { pid: 65420, blp: 0 }, GenericNACK { pid: 65421, blp: 0 }, GenericNACK { pid: 65422, blp: 0 }, GenericNACK { pid: 65423, blp: 0 }, GenericNACK { pid: 65424, blp: 0 }, GenericNACK { pid: 65425, blp: 0 }, GenericNACK { pid: 65426, blp: 0 }, GenericNACK { pid: 65427, blp: 0 }, GenericNACK { pid: 65428, blp: 0 }, GenericNACK { pid: 65429, blp: 0 }, GenericNACK { pid: 65430, blp: 0 }, GenericNACK { pid: 65431, blp: 0 }, GenericNACK { pid: 65432, blp: 0 }, GenericNACK { pid: 65433, blp: 0 }, GenericNACK { pid: 65434, blp: 0 }, GenericNACK { pid: 65435, blp: 0 }, GenericNACK { pid: 65436, blp: 0 }, GenericNACK { pid: 65437, blp: 0 }, GenericNACK { pid: 65438, blp: 0 }, GenericNACK { pid: 65439, blp: 0 }, GenericNACK { pid: 65440, blp: 0 }, GenericNACK { pid: 65441, blp: 0 }, GenericNACK { pid: 65442, blp: 0 }, GenericNACK { pid: 65443, blp: 0 }, GenericNACK { pid: 65444, blp: 0 }, GenericNACK { pid: 65445, blp: 0 }, GenericNACK { pid: 65446, blp: 0 }, GenericNACK { pid: 65447, blp: 0 }, GenericNACK { pid: 65448, blp: 0 }, GenericNACK { pid: 65449, blp: 0 }, GenericNACK { pid: 65450, blp: 0 }, GenericNACK { pid: 65451, blp: 0 }, GenericNACK { pid: 65452, blp: 0 }, GenericNACK { pid: 65453, blp: 0 }, GenericNACK { pid: 65454, blp: 0 }, GenericNACK { pid: 65455, blp: 0 }, GenericNACK { pid: 65456, blp: 0 }, GenericNACK { pid: 65457, blp: 0 }, GenericNACK { pid: 65458, blp: 0 }, GenericNACK { pid: 65459, blp: 0 }, GenericNACK { pid: 65460, blp: 0 }, GenericNACK { pid: 65461, blp: 0 }, GenericNACK { pid: 65462, blp: 0 }, GenericNACK { pid: 65463, blp: 0 }, GenericNACK { pid: 65464, blp: 0 }, GenericNACK { pid: 65465, blp: 0 }, GenericNACK { pid: 65466, blp: 0 }, GenericNACK { pid: 65467, blp: 0 }, GenericNACK { pid: 65468, blp: 0 }, GenericNACK { pid: 65469, blp: 0 }, GenericNACK { pid: 65470, blp: 0 }, GenericNACK { pid: 65471, blp: 0 }, GenericNACK { pid: 65472, blp: 0 }, GenericNACK { pid: 65473, blp: 0 }, GenericNACK { pid: 65474, blp: 0 }, GenericNACK { pid: 65475, blp: 0 }, GenericNACK { pid: 65476, blp: 0 }, GenericNACK { pid: 65477, blp: 0 }, GenericNACK { pid: 65478, blp: 0 }, GenericNACK { pid: 65479, blp: 0 }, GenericNACK { pid: 65480, blp: 0 }, GenericNACK { pid: 65481, blp: 0 }, GenericNACK { pid: 65482, blp: 0 }, GenericNACK { pid: 65483, blp: 0 }, GenericNACK { pid: 65484, blp: 0 }, GenericNACK { pid: 65485, blp: 0 }, GenericNACK { pid: 65486, blp: 0 }, GenericNACK { pid: 65487, blp: 0 }, GenericNACK { pid: 65488, blp: 0 }, GenericNACK { pid: 65489, blp: 0 }, GenericNACK { pid: 65490, blp: 0 }, GenericNACK { pid: 65491, blp: 0 }, GenericNACK { pid: 65492, blp: 0 }, GenericNACK { pid: 65493, blp: 0 }, GenericNACK { pid: 65494, blp: 0 }, GenericNACK { pid: 65495, blp: 0 }, GenericNACK { pid: 65496, blp: 0 }, GenericNACK { pid: 65497, blp: 0 }, GenericNACK { pid: 65498, blp: 0 }, GenericNACK { pid: 65499, blp: 0 }, GenericNACK { pid: 65500, blp: 0 }, GenericNACK { pid: 65501, blp: 0 }, GenericNACK { pid: 65502, blp: 0 }, GenericNACK { pid: 65503, blp: 0 }, GenericNACK { pid: 65504, blp: 0 }, GenericNACK { pid: 65505, blp: 0 }, GenericNACK { pid: 65506, blp: 0 }, GenericNACK { pid: 65507, blp: 0 }, GenericNACK { pid: 65508, blp: 0 }, GenericNACK { pid: 65509, blp: 0 }, GenericNACK { pid: 65510, blp: 0 }, GenericNACK { pid: 65511, blp: 0 }, GenericNACK { pid: 65512, blp: 0 }, GenericNACK { pid: 65513, blp: 0 }, GenericNACK { pid: 65514, blp: 0 }, GenericNACK { pid: 65515, blp: 0 }, GenericNACK { pid: 65516, blp: 0 }, GenericNACK { pid: 65517, blp: 0 }, GenericNACK { pid: 65518, blp: 0 }, GenericNACK { pid: 65519, blp: 0 }, GenericNACK { pid: 65520, blp: 0 }, GenericNACK { pid: 65521, blp: 0 }, GenericNACK { pid: 65522, blp: 0 }, GenericNACK { pid: 65523, blp: 0 }, GenericNACK { pid: 65524, blp: 0 }, GenericNACK { pid: 65525, blp: 0 }, GenericNACK { pid: 65526, blp: 0 }, GenericNACK { pid: 65527, blp: 0 }, GenericNACK { pid: 65528, blp: 0 }, GenericNACK { pid: 65529, blp: 0 }, GenericNACK { pid: 65530, blp: 0 }, GenericNACK { pid: 65531, blp: 0 }, GenericNACK { pid: 65532, blp: 0 }, GenericNACK { pid: 65533, blp: 0 }];

        // Sort NACKs by PID
        expected_nacks.sort_by(|field, field_2| field.pid.partial_cmp(&field_2.pid).unwrap());
        output_nack.nacks.sort_by(|field, field_2| field.pid.partial_cmp(&field_2.pid).unwrap());

        assert_eq!(output_nack.media_ssrc, session.media_session.video_session.remote_ssrc.unwrap());
        assert_eq!(output_nack.media_ssrc, session.media_session.video_session.host_ssrc);
        assert_eq!(output_nack.nacks, expected_nacks)
    }
}