use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use openssl::ssl::SslAcceptor;
use tokio::sync::mpsc::Receiver;

use crate::client::Client;
use crate::http::SessionCommand;
use crate::ice_registry::SessionRegistry;

pub struct Server {
    clients: HashMap<SocketAddr, Client>,
    session_registry: SessionRegistry,
    session_commands_receiver: Receiver<SessionCommand>,
    socket: Arc<UdpSocket>,
    acceptor: Arc<SslAcceptor>,
}

impl Server {
    pub fn new(acceptor: Arc<SslAcceptor>, socket: Arc<UdpSocket>, receiver: Receiver<SessionCommand>) -> Self {
        Server {
            clients: HashMap::new(),
            socket,
            acceptor,
            session_commands_receiver: receiver,
            session_registry: SessionRegistry::new(),
        }
    }

    pub fn listen(&mut self, data: &[u8], remote: SocketAddr) {
        println!("received packets from {} ", remote);
        if let Some(client) = self.clients.get_mut(&remote) {
            println!("already connected");
            client.read_packet(data).unwrap();
            return;
        }

        self.clients.insert(remote.clone(), Client::new(remote, self.acceptor.clone(), self.socket.clone()).unwrap());


        // let stun_message = parse_stun_packet(data);
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

