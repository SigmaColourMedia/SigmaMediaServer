use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use openssl::ssl::SslAcceptor;

use crate::client::{Client, ClientSslState};
use crate::ice_registry::{ConnectionType, SessionRegistry};
use crate::stun::{
    create_stun_success, ICEStunMessageType, parse_binding_request, parse_stun_packet,
};

pub struct Server {
    clients: HashMap<SocketAddr, Client>,
    pub session_registry: SessionRegistry,
    socket: Arc<UdpSocket>,
    acceptor: Arc<SslAcceptor>,
}

impl Server {
    pub fn new(acceptor: Arc<SslAcceptor>, socket: Arc<UdpSocket>) -> Self {
        Server {
            clients: HashMap::new(),
            socket,
            acceptor,
            session_registry: SessionRegistry::new(),
        }
    }

    pub fn listen(&mut self, data: &[u8], remote: SocketAddr) {
        match parse_stun_packet(data) {
            Some(binding_request) => {
                match parse_binding_request(binding_request) {
                    Some(message_type) => {
                        match message_type {
                            ICEStunMessageType::LiveCheck(msg) => {
                                // println!("received live check {:?}", msg.transaction_id);
                                if let Some(session) = self
                                    .session_registry
                                    .get_session_by_username(&msg.username_attribute)
                                {
                                    let mut buffer: [u8; 200] = [0; 200];
                                    if let Ok(bytes_written) = create_stun_success(
                                        &session.credentials,
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
                                    .get_session_by_username(&msg.username_attribute)
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

                            let result = inbound
                                .unprotect(&mut rtp_buffer)
                                .map_err(|_| inbound.unprotect_rtcp(&mut rtp_buffer))
                                .ok()
                                .map(|_| rtp_buffer);

                            result
                        }
                    });
                if let Some(packet) = has_rtp_packet {
                    let viewer_ids = self
                        .session_registry
                        .get_session_by_address(&remote)
                        .and_then(|session| match &session.connection_type {
                            ConnectionType::Viewer(_) => None,
                            ConnectionType::Streamer(streamer) => {
                                Some(streamer.viewers_ids.to_vec())
                            }
                        });

                    if viewer_ids.is_none(){
                        return
                    }
                    
                    let viewer_ids = viewer_ids.unwrap();

                    for id in &viewer_ids {
                        println!("the id {}", id);
                        let client = self
                            .session_registry
                            .get_session(id)
                            .and_then(|session| session.client.as_ref())
                            .and_then(|client| match &client.ssl_state {
                                ClientSslState::Established(ssl_stream) => {
                                    println!("found him");
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
                            let mut outbound_packet = packet.clone();
                            outbound.protect(&mut outbound_packet).unwrap();
                            println!("sending packet to {}", remote);
                            self.socket.send_to(&outbound_packet, address).unwrap();
                        }
                    }
                }

                // if let Some(client) = self
                //     .session_registry
                //     .get_session_by_address(&remote)
                //     .and_then(|session| session.client.as_mut())
                // {
                //     match &client.ssl_state {
                //         ClientSslState::Handshake(_) => {
                //             if let Err(e) = client.read_packet(data) {
                //                 eprintln!("Error reading packet mid handshake {}", e)
                //             }
                //         }
                //         ClientSslState::Established(ssl_stream) => {
                //             let (mut inbound, mut outbound) =
                //                 srtp::openssl::session_pair(ssl_stream.ssl(), Default::default())
                //                     .unwrap();
                //
                //             let mut srtp_buffer = &mut data.to_vec();
                //             let mut copy = srtp_buffer.clone();
                //             match inbound.unprotect(srtp_buffer) {
                //                 Ok(_) => {
                //                     // println!("got RTP packet {:?}", srtp_buffer.len())
                //                 }
                //                 Err(_) => match inbound.unprotect_rtcp(&mut copy) {
                //                     Ok(_) => {
                //                         // println!("got RTCP packet {}", copy.len())
                //                     }
                //                     Err(_) => {
                //                         eprintln!("Did not get RTCP packet {}", copy.len())
                //                     }
                //                 },
                //             }
                //             // println!("some other packet {:?}", data.len())
                //         }
                //         ClientSslState::Shutdown => {
                //             // do nothing
                //         }
                //     }
                // }
            }
        }
    }
}
