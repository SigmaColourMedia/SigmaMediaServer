use std::collections::{hash_map, HashMap};
use std::net::SocketAddr;
use std::sync::Arc;

use openssl::ssl::SslAcceptor;
use tokio::net::UdpSocket;

use crate::client::{Client, ClientSslState};
use crate::stun::parse_stun_packet;

pub struct Server {
    clients: HashMap<SocketAddr, Client>,
    socket: Arc<UdpSocket>,
    acceptor: Arc<SslAcceptor>,
}

impl Server {
    pub fn new(acceptor: Arc<SslAcceptor>, socket: Arc<UdpSocket>) -> Self {
        Server {
            socket,
            acceptor,
            clients: HashMap::new(),
        }
    }

    pub async fn listen(&mut self, data: &[u8], remote: SocketAddr) {
        println!("received packets from {} ", remote);

        let stun_message = parse_stun_packet(data).await;
        println!("stun message {:?}", stun_message);
        if let Some(client) = self.clients.get_mut(&remote) {
            if let ClientSslState::Established(ssl_stream) = &client.ssl_state {
                println!("ssl state {:?}", ssl_stream);
                return;
            }
            if let Err(err) = client.read_packet(data) {
                return println!("Error reading client packet at {} : {}", remote, err);
            }


            let outgoing_packets = client.take_outgoing_packets();
            for packet in outgoing_packets {
                println!("sending packet to {}", remote);
                self.socket.send_to(&packet, remote).await.unwrap();
            }
        } else {
            match self.clients.entry(remote) {
                hash_map::Entry::Vacant(vacant) => {
                    println!(
                        "beginning client data channel connection with {}",
                        remote,
                    );

                    vacant.insert(
                        Client::new(remote, self.acceptor.clone())
                            .expect("could not create new client instance"),
                    );
                }
                hash_map::Entry::Occupied(_) => {}
            }
        }
    }
}

