use std::fmt::Display;
use std::io::{ErrorKind, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use openssl::ssl::SslAcceptor;
use tokio::time::Instant;

use crate::client::{Client, ClientSslState};
use crate::ice_registry::{ConnectionType, SessionRegistry};
use crate::stun::{create_stun_success, get_stun_packet, ICEStunMessageType};

pub struct Server {
    pub session_registry: SessionRegistry,
    inbound_buffer: Vec<u8>,
    outbound_buffer: Vec<u8>,
    socket: Arc<UdpSocket>,
    acceptor: Arc<SslAcceptor>,
}

impl Server {
    pub fn new(acceptor: Arc<SslAcceptor>, socket: Arc<UdpSocket>) -> Self {
        Server {
            inbound_buffer: Vec::with_capacity(2000),
            outbound_buffer: Vec::with_capacity(2000),
            socket,
            acceptor,
            session_registry: SessionRegistry::new(),
        }
    }

    pub fn listen(&mut self, data: &[u8], remote: SocketAddr) {
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
                    .get_session_by_username(&msg.username_attribute.host)
                {
                    session.ttl = Instant::now();

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
                    .map(|session| {
                        session.ttl = Instant::now();
                        session.id.clone()
                    })
                {
                    let is_new_client = self
                        .session_registry
                        .get_session(&resource_id)
                        .map(|session| session.client.is_none())
                        .unwrap();

                    if is_new_client {
                        let client =
                            Client::new(remote.clone(), self.acceptor.clone(), self.socket.clone())
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

    fn handle_other_packets(&mut self, remote: &SocketAddr) {
        let mut viewers_to_notify: Option<Vec<String>> = None;

        if let Some(session) = self
            .session_registry
            .get_session_by_address(&remote)
            .and_then(|session| match session.client {
                None => None,
                Some(_) => Some(session),
            })
        {
            session.ttl = Instant::now();

            match &session.connection_type {
                ConnectionType::Viewer(_) => {
                    let client = session.client.as_mut().unwrap();
                    match &mut client.ssl_state {
                        ClientSslState::Handshake(_) => {
                            if let Err(e) = client.read_packet(&self.inbound_buffer) {
                                eprintln!("Error reading packet mid handshake {}", e)
                            }
                        }
                        ClientSslState::Established(_) => {}
                        ClientSslState::Shutdown => {}
                    }
                }
                ConnectionType::Streamer(streamer) => {
                    let client = session.client.as_mut().unwrap();
                    match &mut client.ssl_state {
                        ClientSslState::Handshake(_) => {
                            if let Err(e) = client.read_packet(&self.inbound_buffer) {
                                eprintln!("Error reading packet mid handshake {}", e)
                            }
                        }
                        ClientSslState::Established(ssl_stream) => {
                            if let Ok(_) =
                                ssl_stream.srtp_inbound.unprotect(&mut self.inbound_buffer)
                            {
                                viewers_to_notify =
                                    Some(streamer.viewers_ids.iter().map(Clone::clone).collect());
                            }
                        }
                        ClientSslState::Shutdown => {}
                    }
                }
            }
        }

        if let Some(viewer_ids) = viewers_to_notify {
            for id in viewer_ids {
                let viewer_session = self.session_registry.get_session(&id);
                if let Some(client) = viewer_session.and_then(|session| session.client.as_mut()) {
                    if let ClientSslState::Established(ssl_stream) = &mut client.ssl_state {
                        self.outbound_buffer.clear();
                        self.outbound_buffer
                            .write(&self.inbound_buffer)
                            .expect("Failed writing to outbound buffer");

                        let send_result = ssl_stream
                            .srtp_outbound
                            .protect(&mut self.outbound_buffer)
                            .map_err(|err| {
                                std::io::Error::new(
                                    ErrorKind::Other,
                                    "Error encrypting SRTP packet",
                                )
                            })
                            .and_then(|_| {
                                self.socket
                                    .send_to(&self.outbound_buffer, client.remote_address)
                            });
                        if let Err(err) = send_result {
                            eprintln!("Error forwarding RTP packet {}", err)
                        }
                    }
                }
            }
        }
    }
}
