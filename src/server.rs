use std::fmt::{Display, Formatter};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use openssl::ssl::SslAcceptor;

use crate::client::{Client, ClientSslState};
use crate::ice_registry::{ConnectionType, SessionRegistry, UsernameKey};
use crate::stun::{
    create_stun_success, ICEStunMessageType, parse_binding_request, parse_stun_packet,
};

pub struct Server {
    pub session_registry: SessionRegistry,
    packets: usize,
    socket: Arc<UdpSocket>,
    acceptor: Arc<SslAcceptor>,
}

impl Server {
    pub fn new(acceptor: Arc<SslAcceptor>, socket: Arc<UdpSocket>) -> Self {
        Server {
            packets: 0,
            socket,
            acceptor,
            session_registry: SessionRegistry::new(),
        }
    }

    pub fn listen(&mut self, data: &[u8], remote: SocketAddr) {
        match parse_stun_packet(data.clone()) {
            Some(binding_request) => {
                match parse_binding_request(binding_request) {
                    Some(message_type) => {
                        match message_type {
                            ICEStunMessageType::LiveCheck(msg) => {
                                // println!("received live check {:?}", msg.transaction_id);
                                if let Some(session) =
                                    self.session_registry.get_session_by_username(&UsernameKey {
                                        host: msg.username_attribute.host.clone(),
                                    })
                                {
                                    let mut buffer: [u8; 200] = [0; 200];
                                    if let Ok(bytes_written) = create_stun_success(
                                        &session.credentials,
                                        &msg.username_attribute,
                                        msg.transaction_id,
                                        &remote,
                                        &mut buffer,
                                    ) {
                                        let output_buffer = &buffer[0..bytes_written];
                                        if let Err(error) =
                                            self.socket.send_to(output_buffer, remote)
                                        {
                                            eprintln!("Error writing to remote {}", error)
                                        }
                                    }
                                }
                            }
                            ICEStunMessageType::Nomination(msg) => {
                                // println!("received nominate packet {:?}", msg.transaction_id);

                                if let Some(resource_id) = self
                                    .session_registry
                                    .get_session_by_username(&UsernameKey {
                                        host: msg.username_attribute.host.clone(),
                                    })
                                    .map(|session| session.id.clone())
                                {
                                    let is_new_client = self
                                        .session_registry
                                        .get_session(&resource_id)
                                        .map(|session| session.client.is_none())
                                        .unwrap();

                                    if is_new_client {
                                        println!("adding new client");
                                        let client = Client::new(
                                            remote.clone(),
                                            self.acceptor.clone(),
                                            self.socket.clone(),
                                        );

                                        if let Ok(client) = client {
                                            self.session_registry
                                                .nominate_client(client, &resource_id);
                                        }
                                    }

                                    let credentials = &self
                                        .session_registry
                                        .get_session(&resource_id)
                                        .unwrap()
                                        .credentials;

                                    // Send OK response
                                    let mut buffer: [u8; 200] = [0; 200];
                                    if let Ok(bytes_written) = create_stun_success(
                                        credentials,
                                        &msg.username_attribute,
                                        msg.transaction_id,
                                        &remote,
                                        &mut buffer,
                                    ) {
                                        let output_buffer = &buffer[0..bytes_written];
                                        if let Err(error) =
                                            self.socket.send_to(output_buffer, remote)
                                        {
                                            eprintln!("Error writing to remote {}", error)
                                        }
                                    }
                                };
                            }
                        }
                    }
                    None => {
                        // todo Invalid binding request
                    }
                }
            }
            None => {
                let has_rtp_packet = self
                    .session_registry
                    .get_session_by_address(&remote)
                    .and_then(|session| session.client.as_mut())
                    .and_then(|client| match &client.ssl_state {
                        ClientSslState::Handshake(_) => {
                            if let Err(e) = client.read_packet(data) {
                                eprintln!("Error reading packet mid handshake {}", e)
                            }
                            None
                        }
                        ClientSslState::Shutdown => None,
                        ClientSslState::Established(ssl_stream) => {
                            let (mut inbound, _) =
                                srtp::openssl::session_pair(ssl_stream.ssl(), Default::default())
                                    .unwrap();
                            let mut rtp_buffer = data.to_vec();
                            let mut rtcp_buffer = data.to_vec();
                            println!("packet len {}", data.len());

                            inbound
                                .unprotect(&mut rtp_buffer)
                                .map(|_| VideoPacket::RTP(rtp_buffer))
                                .or_else(|err| {
                                    inbound
                                        .unprotect_rtcp(&mut rtcp_buffer)
                                        .map(|_| VideoPacket::RTCP(rtcp_buffer))
                                })
                                .ok()
                        }
                    });

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
                            .and_then(|session| session.client.as_ref())
                            .and_then(|client| match &client.ssl_state {
                                ClientSslState::Established(ssl_stream) => {
                                    Some((ssl_stream, client.remote_address))
                                }
                                _ => {
                                    println!("some other state");
                                    None
                                }
                            });
                        if let Some((stream, address)) = client {
                            let (_, mut outbound) =
                                srtp::openssl::session_pair(stream.ssl(), Default::default())
                                    .unwrap();
                            match &packet {
                                VideoPacket::RTP(rtp_packet) => {
                                    let mut outbound_packet = rtp_packet.clone();
                                    outbound.protect(&mut outbound_packet).unwrap();
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
