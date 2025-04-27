use std::cell::RefCell;
use std::collections::HashSet;
use std::io::Write;
use std::mem;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Instant};

use bytes::{Bytes};
use rtcp::rtcp::RtcpPacket;
use rtcp::{Marshall, Unmarshall, unmarshall_compound_rtcp};

use sdp::SDPResolver;

use crate::client::{Client, ClientSslState};
use crate::config::get_global_config;
use crate::ice_registry::{ConnectionType, Session, SessionRegistry};
use crate::media_header::{MediaHeader, RTPHeader};
use crate::rtp_reporter::{nack_to_lost_pids, RTPReporter};
use crate::stun::{create_stun_success, get_stun_packet, ICEStunMessageType};
#[derive(Debug, Clone)]
pub enum UDPServerError {
    RTCPProtectError,
    RTPProtectError,
    SocketWriteError,
    RTCPUnprotectError,
    RTCPDecodeError,
}

pub struct UDPServer {
    pub session_registry: SessionRegistry,
    pub sdp_resolver: SDPResolver,
    inbound_buffer: Vec<u8>,
    outbound_buffer: Vec<u8>,
    pub socket: UdpSocket,
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
        self.inbound_buffer.resize(data.len(), 0);
        self.inbound_buffer
            .copy_from_slice(data);

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
                        eprintln!("Error writing to remote")
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
        let dummy_session = Session::default();
        let mut sender_session = mem::replace(sender_session.unwrap(), dummy_session);

        // Update session TTL
        sender_session.ttl = Instant::now();


        match &mut sender_session.connection_type {
            ConnectionType::Viewer(_) => {
                self.handle_viewer_session(&mut sender_session);
            }
            ConnectionType::Streamer(_) => {
                self.handle_streamer_session(&mut sender_session)
            }
        };
        mem::replace(self.session_registry.get_session_by_address_mut(remote).unwrap(), sender_session);
    }

    fn handle_viewer_session(&mut self, viewer: &mut Session) {
        let client = viewer.client.as_mut().expect("Viewer Client must be first established");
        match &mut client.ssl_state {
            ClientSslState::Handshake(_) => {
                if let Err(e) = client.read_packet(&self.inbound_buffer) {
                    eprintln!("Error reading packet mid handshake {}", e)
                };
            }
            ClientSslState::Established(viewer_ssl) => {
                let mut inbound_buffer_copy = self.inbound_buffer.clone();
                let rtcp_decode_result = viewer_ssl.srtp_inbound.unprotect_rtcp(&mut inbound_buffer_copy).or(Err(UDPServerError::RTCPUnprotectError)).and_then(|_| {
                    let bytes = Bytes::from(inbound_buffer_copy);
                    unmarshall_compound_rtcp(bytes).or(Err(UDPServerError::RTCPDecodeError))
                });

                if let Ok(rtcp_packets) = rtcp_decode_result {
                    for packet in rtcp_packets {
                        match packet {
                            RtcpPacket::TransportLayerFeedbackMessage(nack) => {
                                let lost_packets = nack.nacks.iter().flat_map(nack_to_lost_pids
                                ).collect::<HashSet<u16>>().into_iter().filter_map(|pid| client.rtp_replay_buffer.get(pid)).collect::<Vec<_>>();


                                for packet in lost_packets {
                                    if let Err(e) = self.socket.send_to(packet, client.remote_address) {
                                        eprintln!("Error resending RTP packet {}", e)
                                    }
                                }
                            }
                            // todo handle other RTCP packets
                            _ => {}
                        }
                    }
                }
            }

            ClientSslState::Shutdown => {
                todo!("Handle shutdown case?")
            }
        }
    }

    fn handle_streamer_session(&mut self, sender_session: &mut Session) {
        let mut sender_client = sender_session.client.as_mut().unwrap();
        if let ConnectionType::Streamer(_) = &sender_session.connection_type {
            match &mut sender_client.ssl_state {
                ClientSslState::Handshake(_) => {
                    self.process_dtls_handshake(sender_client)
                }

                ClientSslState::Established(_) => {
                    // if thread_rng().gen_bool(0.25) {
                    //     mem::replace(&mut sender_session.client, Some(sender_client));
                    //     mem::replace(&mut sender_session.connection_type, sender_connection_type);
                    //     let _ = mem::replace(self.session_registry.get_session_by_address_mut(remote).unwrap(), sender_session);
                    //     return;
                    // }

                    let buffer = Bytes::copy_from_slice(&self.inbound_buffer);

                    if let Ok(header) = MediaHeader::unmarshall(buffer) {
                        match header {
                            MediaHeader::RTP(header) => {
                                self.process_streamer_rtp(sender_session, header)
                            }
                            // todo Do something with the Sender Reports
                            MediaHeader::RTCP(_) => {}
                        }
                    }
                }
                ClientSslState::Shutdown => {
                    todo!("Handle shutdown case?")
                }
            }
        } else {
            unreachable!("ConnectionType must be Streamer type")
        }
    }

    fn process_streamer_rtp(&mut self, sender_session: &mut Session, header: RTPHeader) {
        let mut sender_client = sender_session.client.as_mut().unwrap();

        if let ConnectionType::Streamer(streamer) = &mut sender_session.connection_type {
            if let ClientSslState::Established(ssl_stream) = &mut sender_client.ssl_state {
                if let Ok(_) = ssl_stream.srtp_inbound.unprotect(&mut self.inbound_buffer) {
                    let is_video_packet = header.payload_type == sender_session.media_session.video_session.payload_number as u8;

                    if is_video_packet {
                        // Feed thumbnail image extractor
                        streamer
                            .thumbnail_extractor
                            .try_extract_thumbnail(&self.inbound_buffer);

                        // Update video_reporter
                        match sender_session.video_reporter.as_mut() {
                            Some(mut reporter) =>
                                {
                                    reporter.feed_rtp(header.clone());
                                }
                            None => {
                                let reporter = RTPReporter::new(header.seq, sender_session.media_session.video_session.host_ssrc, sender_session.media_session.video_session.remote_ssrc.unwrap());
                                sender_session.video_reporter.insert(reporter);
                            }
                        };
                    }


                    let viewer_ids = self
                        .session_registry
                        .get_room(streamer.owned_room_id)
                        .expect("Streamer room should exist")
                        .viewer_ids
                        .clone()
                        .into_iter();

                    for id in viewer_ids {
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
                            self.outbound_buffer.resize(self.inbound_buffer.len(), 0);
                            self.outbound_buffer.copy_from_slice(&self.inbound_buffer);


                            // Remap RTP header
                            let (host_pt, host_ssrc) = if is_video_packet {
                                (viewer_session.media_session.video_session.payload_number, viewer_session.media_session.video_session.host_ssrc)
                            } else {
                                (viewer_session.media_session.audio_session.payload_number, viewer_session.media_session.audio_session.host_ssrc)
                            };
                            let mut header_copy = header.clone();
                            header_copy.payload_type = host_pt as u8;
                            header_copy.ssrc = host_ssrc;
                            let header_buffer = header_copy.marshall().unwrap().to_vec();
                            self.outbound_buffer[..header_buffer.len()].copy_from_slice(&header_buffer);


                            // Turn packet into SRTP
                            if let Ok(_) = ssl_stream.srtp_outbound.protect(&mut self.outbound_buffer) {

                                // Update RTP replay Buffer
                                if is_video_packet {
                                    let roc = ssl_stream.srtp_outbound.session().get_stream_roc(viewer_session.media_session.video_session.host_ssrc).unwrap_or(0);
                                    viewer_client.rtp_replay_buffer.insert(Bytes::copy_from_slice(&self.outbound_buffer), roc);
                                }

                                // if thread_rng().gen_bool(0.25) {
                                //     mem::replace(&mut sender_session.client, Some(sender_client));
                                //     mem::replace(&mut sender_session.connection_type, sender_connection_type);
                                //     let _ = mem::replace(self.session_registry.get_session_by_address_mut(remote).unwrap(), sender_session);
                                //     return;
                                // }


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
            } else {
                unreachable!("ClientSSLState must be established")
            }
        } else {
            unreachable!("ClientType must be Streamer type")
        }
    }


    fn process_dtls_handshake(&self, client: &mut Client) {
        if let Err(e) = client.read_packet(&self.inbound_buffer) {
            eprintln!("Error reading packet mid handshake {}", e)
        }
    }
}



