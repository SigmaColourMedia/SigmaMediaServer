use std::io::Write;
use std::net::{SocketAddr, UdpSocket};
use std::time::Instant;

use sdp::SDPResolver;

use crate::client::{Client, ClientSslState};
use crate::config::get_global_config;
use crate::ice_registry::{ConnectionType, SessionRegistry};
use crate::rtp::{get_rtp_header_data, remap_rtp_header};
use crate::stun::{create_stun_success, get_stun_packet, ICEStunMessageType};

pub struct UDPServer {
    pub session_registry: SessionRegistry,
    pub sdp_resolver: SDPResolver,
    inbound_buffer: Vec<u8>,
    outbound_buffer: Vec<u8>,
    socket: UdpSocket,
}

impl UDPServer {
    pub fn new(socket: UdpSocket) -> Self {
        let config = get_global_config();
        UDPServer {
            sdp_resolver: SDPResolver::new(
                format!("sha-256 {}", config.ssl_config.fingerprint).as_str(),
                config.udp_server_config.address,
            ),
            inbound_buffer: Vec::with_capacity(2000),
            outbound_buffer: Vec::with_capacity(2000),
            socket,
            session_registry: SessionRegistry::new(),
        }
    }

    pub fn process_packet(&mut self, data: &[u8], remote: SocketAddr) {
        self.inbound_buffer.clear();
        self.inbound_buffer
            .write_all(data)
            .expect("Failed to write to internal buffer");

        match get_stun_packet(&self.inbound_buffer) {
            Some(stun_packet) => self.handle_stun_packet(&remote, stun_packet),
            None => self.handle_other_packets(&remote),
        }
    }

    fn handle_stun_packet(&mut self, remote: &SocketAddr, stun_packet: ICEStunMessageType) {
        match stun_packet {
            ICEStunMessageType::LiveCheck(msg) => {
                if let Some(session) = self
                    .session_registry
                    .get_session_by_username_mut(&msg.username_attribute)
                {
                    session.ttl = Instant::now();

                    let mut buffer: [u8; 200] = [0; 200];
                    let bytes_written = create_stun_success(
                        &session.media_session.ice_credentials,
                        msg.transaction_id,
                        &remote,
                        &mut buffer,
                    )
                    .expect("Failed to create STUN success response");

                    let output_buffer = &buffer[0..bytes_written];
                    if let Err(error) = self.socket.send_to(output_buffer, remote) {
                        eprintln!("Error writing to remote {}", error)
                    }
                }
            }
            ICEStunMessageType::Nomination(msg) => {
                if let Some(resource_id) = self
                    .session_registry
                    .get_session_by_username_mut(&msg.username_attribute)
                    .map(|session| {
                        session.ttl = Instant::now();
                        session.id.clone()
                    })
                {
                    let is_new_client = self
                        .session_registry
                        .get_session_mut(resource_id)
                        .map(|session| session.client.is_none())
                        .unwrap();

                    if is_new_client {
                        let client = Client::new(remote.clone(), self.socket.try_clone().unwrap())
                            .expect("Should create a Client");

                        self.session_registry.nominate_client(client, &resource_id);
                    }

                    let credentials = &self
                        .session_registry
                        .get_session_mut(resource_id)
                        .unwrap()
                        .media_session
                        .ice_credentials;

                    // Send OK response
                    let mut buffer: [u8; 200] = [0; 200];
                    let bytes_written =
                        create_stun_success(credentials, msg.transaction_id, &remote, &mut buffer)
                            .expect("Should create STUN success response");

                    let output_buffer = &buffer[0..bytes_written];
                    if let Err(error) = self.socket.send_to(output_buffer, remote) {
                        eprintln!("Error writing to remote {}", error)
                    }
                };
            }
        }
    }

    fn handle_other_packets(&mut self, remote: &SocketAddr) {
        let sender_session = self.session_registry.get_session_by_address_mut(remote);

        let is_client_established = sender_session
            .as_ref()
            .and_then(|session| session.client.as_ref())
            .is_some();

        // Sender session has not yet established a Client
        if !is_client_established {
            return;
        }

        let sender_session = sender_session.unwrap();
        let sender_client = sender_session.client.as_mut().unwrap();

        // Update session TTL
        sender_session.ttl = Instant::now();

        match &mut sender_session.connection_type {
            ConnectionType::Viewer(_) => {
                if let ClientSslState::Handshake(_) = &mut sender_client.ssl_state {
                    if let Err(err) = sender_client.read_packet(&self.inbound_buffer) {
                        eprintln!("Failed reading packet from {} with error {}", remote, err)
                    }
                }
            }
            ConnectionType::Streamer(streamer) => match &mut sender_client.ssl_state {
                ClientSslState::Handshake(_) => {
                    if let Err(e) = sender_client.read_packet(&self.inbound_buffer) {
                        eprintln!("Error reading packet mid handshake {}", e)
                    }
                }
                ClientSslState::Established(ssl_stream) => {
                    if let Ok(_) = ssl_stream.srtp_inbound.unprotect(&mut self.inbound_buffer) {
                        let room_id = streamer.owned_room_id;

                        let is_video_packet = get_rtp_header_data(&self.inbound_buffer)
                            .payload_type
                            .eq(&(sender_session.media_session.video_session.payload_number as u8));

                        if is_video_packet {
                            streamer
                                .thumbnail_extractor
                                .try_extract_thumbnail(&self.inbound_buffer);
                        }

                        let viewer_ids = self
                            .session_registry
                            .get_room(room_id)
                            .expect("Streamer room should exist")
                            .viewer_ids
                            .clone()
                            .into_iter();

                        for id in viewer_ids {
                            let streamer_media = self
                                .session_registry
                                .get_session_by_address_mut(&remote)
                                .expect("Streamer session should be established")
                                .media_session
                                .clone();
                            let viewer_session = self.session_registry.get_session_mut(id).expect("Viewer session should be established if viewer id belongs to a room");

                            // If viewer has yet elected a Client, skip it
                            if viewer_session.client.is_none() {
                                continue;
                            }

                            let viewer_client = viewer_session.client.as_mut().unwrap();

                            if let ClientSslState::Established(ssl_stream) =
                                &mut viewer_client.ssl_state
                            {
                                // Write to temp buffer
                                self.outbound_buffer.clear();
                                self.outbound_buffer
                                    .write(&self.inbound_buffer)
                                    .expect("Should write to outbound buffer");

                                // Remap Payload Type and SSRC to match negotiated values
                                remap_rtp_header(
                                    &mut self.outbound_buffer,
                                    &streamer_media,
                                    &viewer_session.media_session,
                                );

                                // Convert RTP to SRTP and send to remote
                                if let Ok(_) =
                                    ssl_stream.srtp_outbound.protect(&mut self.outbound_buffer)
                                {
                                    if let Err(err) = self.socket.send_to(
                                        &self.outbound_buffer,
                                        viewer_client.remote_address,
                                    ) {
                                        eprintln!("Couldn't send RTP data {}", err)
                                    }
                                }
                            }
                        }
                    }
                }
                ClientSslState::Shutdown => {
                    todo!("Handle shutdown case?")
                }
            },
        }
    }
}
