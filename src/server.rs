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
        println!("received packets from {} ", remote);
        match parse_stun_packet(data) {
            Some(binding_request) => {
                match parse_binding_request(binding_request) {
                    Some(message_type) => {
                        match message_type {
                            ICEStunMessageType::LiveCheck(msg) => {
                                println!("received live check {:?}", msg.transaction_id);
                                if let Some(session) = self.session_registry.get_session_by_username(&msg.username_attribute) {
                                    let mut buffer: [u8; 84] = [0; 84];
                                    if let Ok(bytes_written) = create_stun_success(&session.credentials, msg.transaction_id, &remote, &mut buffer) {
                                        let output_buffer = &buffer[0..bytes_written];
                                        if let Err(error) = self.socket.send_to(output_buffer, remote) {
                                            eprintln!("Error writing to remote {}", error)
                                        }
                                    }
                                }
                            }
                            ICEStunMessageType::Nomination(msg) => {
                                println!("received nominate packet {:?}", msg)
                            }
                        }
                    }
                    None => {
                        // todo Invalid binding request
                    }
                }
            }
            None => {
                // todo Some other packet
            }
        }


        // self.clients.insert(remote.clone(), Client::new(remote, self.acceptor.clone(), self.socket.clone()).unwrap());


        // println!("stun message {:?}", stun_message);
        // if let Some(client) = self.clients.get_mut(&remote) {
        //     if let ClientSslState::Established(ssl_stream) = &client.ssl_state {
        //         println!("ssl state {:?}", ssl_stream);
        //         return;
        //     }
        //     if let Err(err) = client.read_packet(data) {
        //         return println!("Error reading client packet at {} : {}", remote, err);
        //     }
        //
        //
        //     let outgoing_packets = client.take_outgoing_packets();
        //     for packet in outgoing_packets {
        //         println!("sending packet to {}", remote);
        //         self.socket.send_to(&packet, remote).await.unwrap();
        //     }
        // } else {
        //     match self.clients.entry(remote) {
        //         hash_map::Entry::Vacant(vacant) => {
        //             println!(
        //                 "beginning client data channel connection with {}",
        //                 remote,
        //             );
        //
        //             vacant.insert(
        //                 Client::new(remote, self.acceptor.clone())
        //                     .expect("could not create new client instance"),
        //             );
        //         }
        //         hash_map::Entry::Occupied(_) => {}
        //     }
        // }
    }
}

