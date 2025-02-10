use std::io::Write;
use std::mem;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};
use byteorder::{BigEndian, ReadBytesExt};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use rand::{Rng, thread_rng};
use rtcp::rtcp::RtcpPacket;
use rtcp::{Marshall, Unmarshall, unmarshall_compound_rtcp};
use rtcp::payload_specific_feedback::PictureLossIndication;
use rtcp::transport_layer_feedback::{GenericNACK, TransportLayerNACK};

use sdp::SDPResolver;

use crate::client::{Client, ClientSslState};
use crate::config::get_global_config;
use crate::ice_registry::{ConnectionType, Session, SessionRegistry, Streamer};
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

        let sender_client = sender_session.client.as_mut().unwrap();
        let sender_remote = sender_client.remote_address;
        // Update session TTL
        sender_session.ttl = Instant::now();


        match &mut sender_session.connection_type {
            ConnectionType::Viewer(viewer) => match &mut sender_client.ssl_state {
                ClientSslState::Handshake(_) => {
                    if let Err(e) = sender_client.read_packet(&self.inbound_buffer) {
                        eprintln!("Error reading packet mid handshake {}", e)
                    }
                }
                ClientSslState::Established(viewer_ssl) => {
                    let mut inbound_buffer_copy = self.inbound_buffer.clone();
                    if let Ok(_) = viewer_ssl.srtp_inbound.unprotect_rtcp(&mut inbound_buffer_copy) {
                        let data = Bytes::from(inbound_buffer_copy);

                        if let Ok(rtcp_packets) = unmarshall_compound_rtcp(data) {
                            if let Some(streamer_session) = self.session_registry.get_room(viewer.room_id).map(|room| room.owner_id).and_then(|owner_id| self.session_registry.get_session_mut(owner_id)) {
                                if let Some(streamer_client) = &mut streamer_session.client {
                                    if let ClientSslState::Established(streamer_ssl) = &mut streamer_client.ssl_state {
                                        let time = Instant::now();

                                        for packet in rtcp_packets {
                                            match packet {
                                                RtcpPacket::TransportLayerFeedbackMessage(nack) => {
                                                    let lost_pids = nack.nacks.iter().map(|item| item.pid).collect::<Vec<u16>>();
                                                    let mut nacks_to_send: Vec<u16> = vec![];
                                                    for pid in lost_pids {
                                                        match sender_client.rtp_replay_buffer.get(pid) {
                                                            Some(data) => {
                                                                if let Err(e) = self.socket.send_to(data, sender_remote) {
                                                                    eprintln!("Error resending RTP packet {}", e)
                                                                }
                                                            }
                                                            None => {
                                                                nacks_to_send.push(pid)
                                                            }
                                                        }
                                                    }
                                                    // let nacks = nacks_to_send.into_iter().map(|pid| GenericNACK { pid, blp: 0 }).collect::<Vec<GenericNACK>>();
                                                    // if !nacks.is_empty() {
                                                    //     let sender_ssrc = streamer_session.media_session.video_session.host_ssrc;
                                                    //     let media_ssrc = streamer_session.media_session.video_session.remote_ssrc.unwrap_or(0); // todo Handle a default for remote ssrc
                                                    //
                                                    //     let mut rtcp_nack = TransportLayerNACK::new(nacks, sender_ssrc, media_ssrc).marshall().expect("Marshall should resolve on trusted source").to_vec();
                                                    //     if let Ok(_) = streamer_ssl.srtp_outbound.protect_rtcp(&mut rtcp_nack) {
                                                    //         if let Err(e) = self.socket.send_to(&rtcp_nack, streamer_client.remote_address) {
                                                    //             eprintln!("Error sending stashed RTCP packet to remote {}", e)
                                                    //         }
                                                    //     }
                                                    // }
                                                }
                                                RtcpPacket::PayloadSpecificFeedbackMessage(_) => {
                                                    let mut pkt = PictureLossIndication::new(streamer_session.media_session.video_session.host_ssrc, streamer_session.media_session.video_session.remote_ssrc.unwrap()).marshall().unwrap().to_vec();
                                                    if let Ok(_) = streamer_ssl.srtp_outbound.protect_rtcp(&mut pkt) {
                                                        if let Err(e) = self.socket.send_to(&pkt, streamer_client.remote_address) {
                                                            eprintln!("Error sending stashed RTCP packet to remote {}", e)
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                ClientSslState::Shutdown => {
                    todo!("Handle shutdown case?")
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
                            // Feed thumbnail image extractor
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

                        let viewers = viewer_ids.len();
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


                                match ssl_stream.srtp_outbound.protect(&mut self.outbound_buffer) {
                                    Ok(_) => {
                                        if is_video_packet {
                                            // Update RTP Replay Buffer
                                            let roc = ssl_stream.srtp_outbound.session().get_stream_roc(viewer_session.media_session.video_session.host_ssrc).unwrap_or(0);
                                            viewer_client.rtp_replay_buffer.insert(Bytes::from(self.outbound_buffer.clone()), roc);
                                        }


                                        if let Err(err) = self.socket.send_to(
                                            &self.outbound_buffer,
                                            viewer_client.remote_address,
                                        ) {
                                            eprintln!("Couldn't send RTP data {}", err)
                                        }
                                    }
                                    Err(err) => { println!("Could not protect due to {}", err) }
                                }
                            }
                        }
                    }
                }
                ClientSslState::Shutdown => {
                    todo!("Handle shutdown case?")
                }
            },
        };
        let _ = mem::replace(self.session_registry.get_session_by_address_mut(remote).unwrap(), sender_session);
    }
}
