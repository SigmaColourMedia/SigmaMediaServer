use std::fmt::{Display, Formatter};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use openssl::ssl::SslAcceptor;

use crate::client::{Client, ClientSslState};
use crate::ice_registry::{ConnectionType, SessionRegistry};
use crate::stun::{create_stun_success, get_stun_packet, ICEStunMessageType};

pub struct Server {
    pub session_registry: SessionRegistry,
    socket: Arc<UdpSocket>,
    acceptor: Arc<SslAcceptor>,
}

impl Server {
    pub fn new(acceptor: Arc<SslAcceptor>, socket: Arc<UdpSocket>) -> Self {
        Server {
            socket,
            acceptor,
            session_registry: SessionRegistry::new(),
        }
    }

    pub fn listen(&mut self, data: &[u8], remote: SocketAddr) {
        match get_stun_packet(data) {
            Some(message_type) => {
                match message_type {
                    ICEStunMessageType::LiveCheck(msg) => {
                        if let Some(session) = self
                            .session_registry
                            .get_session_by_username(&msg.username_attribute.host)
                        {
                            let mut buffer: [u8; 200] = [0; 200];
                            let bytes_written = create_stun_success(
                                &session.credentials,
                                &msg.username_attribute,
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
                            .get_session_by_username(&msg.username_attribute.host)
                            .map(|session| session.id.clone())
                        {
                            let is_new_client = self
                                .session_registry
                                .get_session(&resource_id)
                                .map(|session| session.client.is_none())
                                .unwrap();

                            if is_new_client {
                                let client = Client::new(
                                    remote.clone(),
                                    self.acceptor.clone(),
                                    self.socket.clone(),
                                )
                                .expect("Failed to create Client");

                                self.session_registry.nominate_client(client, &resource_id);
                            }

                            let credentials = &self
                                .session_registry
                                .get_session(&resource_id)
                                .unwrap()
                                .credentials;

                            // Send OK response
                            let mut buffer: [u8; 200] = [0; 200];
                            let bytes_written = create_stun_success(
                                credentials,
                                &msg.username_attribute,
                                msg.transaction_id,
                                &remote,
                                &mut buffer,
                            )
                            .expect("Failed to create STUN success message");

                            let output_buffer = &buffer[0..bytes_written];
                            if let Err(error) = self.socket.send_to(output_buffer, remote) {
                                eprintln!("Error writing to remote {}", error)
                            }
                        };
                    }
                }
            }
            None => {
                let has_rtp_packet = self
                    .session_registry
                    .get_session_by_address(&remote)
                    .and_then(|session| session.client.as_mut())
                    .and_then(|client| match &mut client.ssl_state {
                        ClientSslState::Handshake(_) => {
                            if let Err(e) = client.read_packet(data) {
                                eprintln!("Error reading packet mid handshake {}", e)
                            }
                            None
                        }
                        ClientSslState::Shutdown => None,
                        ClientSslState::Established(ssl_stream) => {
                            let mut rtp_buffer = data.to_vec();
                            let mut rtcp_buffer = data.to_vec();
                            println!("packet len {}", data.len());

                            ssl_stream
                                .srtp_inbound
                                .unprotect(&mut rtp_buffer)
                                .map(|_| VideoPacket::RTP(rtp_buffer))
                                .or_else(|err| {
                                    ssl_stream
                                        .srtp_inbound
                                        .unprotect_rtcp(&mut rtcp_buffer)
                                        .map(|_| VideoPacket::RTCP(rtcp_buffer))
                                })
                                .ok()
                        }
                    });
                if has_rtp_packet.is_none() {
                    println!("packet {} is not RTP packet", data.len())
                }

                if let Some(packet) = has_rtp_packet {
                    println!("{}", packet);
                    let viewer_ids = self
                        .session_registry
                        .get_session_by_address(&remote)
                        .and_then(|session| match &session.connection_type {
                            ConnectionType::Viewer(_) => None,
                            ConnectionType::Streamer(streamer) => {
                                Some(streamer.viewers_ids.to_vec())
                            }
                        });

                    if viewer_ids.is_none() {
                        return;
                    }

                    let viewer_ids = viewer_ids.unwrap();

                    for id in &viewer_ids {
                        let client = self
                            .session_registry
                            .get_session(id)
                            .and_then(|session| session.client.as_mut())
                            .and_then(|client| match &mut client.ssl_state {
                                ClientSslState::Established(ssl_stream) => {
                                    Some((ssl_stream, client.remote_address))
                                }
                                _ => {
                                    println!("some other state");
                                    None
                                }
                            });
                        if let Some((stream, address)) = client {
                            match &packet {
                                VideoPacket::RTP(rtp_packet) => {
                                    let mut outbound_packet = rtp_packet.clone();
                                    stream.srtp_outbound.protect(&mut outbound_packet).unwrap();
                                    self.socket.send_to(&outbound_packet, address).unwrap();
                                }
                                VideoPacket::RTCP(rtcp_packet) => {
                                    let mut outbound_packet = rtcp_packet.clone();
                                    // outbound.protect_rtcp(&mut outbound_packet).unwrap();
                                    // self.socket.send_to(&outbound_packet, address).unwrap();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
enum VideoPacket {
    RTP(Vec<u8>),
    RTCP(Vec<u8>),
}

impl Display for VideoPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoPacket::RTP(packet) => {
                write!(f, "Video packet RTP {}", packet.len())
            }
            VideoPacket::RTCP(packet) => {
                write!(f, "Video packet RTCP {}", packet.len())
            }
        }
    }
}
