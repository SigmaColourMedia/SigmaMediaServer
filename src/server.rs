use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use openssl::ssl::SslAcceptor;

use crate::client::Client;
use crate::ice_registry::SessionRegistry;
use crate::stun::{create_stun_success, ICEStunMessageType, parse_binding_request, parse_stun_packet};

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
                                println!("received live check {:?}", msg.transaction_id);
                                if let Some(session) = self.session_registry.get_session_by_username(&msg.username_attribute) {
                                    let mut buffer: [u8; 200] = [0; 200];
                                    if let Ok(bytes_written) = create_stun_success(&session.credentials, msg.transaction_id, &remote, &mut buffer) {
                                        let output_buffer = &buffer[0..bytes_written];
                                        if let Err(error) = self.socket.send_to(output_buffer, remote) {
                                            eprintln!("Error writing to remote {}", error)
                                        }
                                    }
                                }
                            }
                            ICEStunMessageType::Nomination(msg) => {
                                println!("received nominate packet {:?}", msg.transaction_id);

                                if let Some(resource_id) = self.session_registry.get_session_by_username(&msg.username_attribute).map(|session| session.id.clone()) {
                                    let is_new_client = self.session_registry.get_session(&resource_id).map(|session| session.client.is_none()).unwrap();

                                    if is_new_client {
                                        println!("adding new client");
                                        let client = Client::new(remote.clone(), self.acceptor.clone(), self.socket.clone());

                                        if let Ok(client) = client {
                                            self.session_registry.nominate_client(client, &resource_id);
                                        }
                                    }

                                    let credentials = &self.session_registry.get_session(&resource_id).unwrap().credentials;
                                    // Send OK response
                                    let mut buffer: [u8; 200] = [0; 200];
                                    if let Ok(bytes_written) = create_stun_success(credentials, msg.transaction_id, &remote, &mut buffer) {
                                        let output_buffer = &buffer[0..bytes_written];
                                        if let Err(error) = self.socket.send_to(output_buffer, remote) {
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
                if let Some(client) = self.session_registry.get_session_by_address(&remote).and_then(|session| session.client.as_mut()) {
                    println!("some other packet");
                    let mut buffer = [0u8; 400];
                    if let Ok(bytes_read) = client.read_packet(data) {} else {
                        println!("error reading packet")
                    }
                }
                // if let Some(client) = self.clients.get_mut(&remote) {
                //     client.read_packet(data);
                // } else {
                //     self.clients.insert(remote.clone(), Client::new(remote.clone(), self.acceptor.clone(), self.socket.clone()).unwrap());
                // }
            }
        }
    }
}

