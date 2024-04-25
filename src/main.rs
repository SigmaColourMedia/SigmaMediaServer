use std::{fmt, io, mem};
use std::collections::{hash_map, HashMap, VecDeque};
use std::io::{Error, ErrorKind, Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;

use openssl::error::ErrorStack;
use openssl::ssl::{HandshakeError, MidHandshakeSslStream, SslAcceptor, SslStream};
use tokio::io::AsyncWriteExt;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::acceptor::SSLConfig;
use crate::ClientError::{IncompletePacketRead, OpenSslError};

mod acceptor;
mod server;

#[tokio::main]
async fn main() {
    let config = SSLConfig::new();
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:9090").await.unwrap());
    let mut clients: HashMap<SocketAddr, bool> = HashMap::new();
    let remote_socket = socket.clone();
    let (tx, mut rx) = mpsc::channel::<(Vec<u8>, SocketAddr)>(1000);

    let mut server = Server::new(config.acceptor.clone(), remote_socket);

    tokio::spawn(async move {
        while let Some((bytes, addr)) = rx.recv().await {
            server.receive(&bytes, addr).await;
        }
    });

    loop {
        let mut buffer = [0; 3600];
        let (bytes_read, remote_addr) = socket.recv_from(&mut buffer).await.unwrap();
        tx.send((Vec::from(&buffer[..=bytes_read]), remote_addr)).await.unwrap();
    };
}

enum ClientSslState {
    Handshake(MidHandshakeSslStream<DTLSPacket>),
    Established(SslStream<DTLSPacket>),
    Shutdown,
}

struct Client {
    ssl_state: ClientSslState,
    remote_address: SocketAddr,
}

impl Client {
    pub fn new(remote: SocketAddr, acceptor: Arc<SslAcceptor>) -> Result<Self, ErrorStack> {
        let dtls_packet = DTLSPacket::new();
        match acceptor.accept(dtls_packet) {
            Ok(_) => unreachable!("handshake cannot finish with no incoming packets"),
            Err(HandshakeError::SetupFailure(err)) => return Err(err),
            Err(HandshakeError::Failure(_)) => {
                unreachable!("handshake cannot fail before starting")
            }
            Err(HandshakeError::WouldBlock(mid_handshake)) => Ok(Client {
                ssl_state: ClientSslState::Handshake(mid_handshake),
                remote_address: remote,
            })
        }
    }

    pub fn read_packet(&mut self, packet: &[u8]) -> Result<(), ClientError> {
        self.ssl_state = match mem::replace(&mut self.ssl_state, ClientSslState::Shutdown) {
            ClientSslState::Handshake(mut mid_handshake) => {
                mid_handshake.get_mut().incoming_packets.push_back(Vec::from(packet));
                match mid_handshake.handshake() {
                    Ok(ssl_stream) => {
                        println!("DTLS handshake finished for remote {}", self.remote_address);
                        ClientSslState::Established(ssl_stream)
                    }
                    Err(handshake_error) => match handshake_error {
                        HandshakeError::SetupFailure(err) => {
                            return Err(OpenSslError(err));
                        }
                        HandshakeError::Failure(mid_handshake) => {
                            println!(
                                "SSL handshake failure with remote {}: {}",
                                self.remote_address,
                                mid_handshake.error()
                            );
                            ClientSslState::Handshake(mid_handshake)
                        }
                        HandshakeError::WouldBlock(mid_handshake) => {
                            ClientSslState::Handshake(mid_handshake)
                        }
                    },
                }
            }
            ClientSslState::Established(mut ssl_stream) => {
                ssl_stream.get_mut().incoming_packets.push_back(Vec::from(packet));
                ClientSslState::Established(ssl_stream)
            }
            ClientSslState::Shutdown => ClientSslState::Shutdown,
        };
        Ok(())
    }

    pub fn take_outgoing_packets(&mut self) -> impl Iterator<Item=Vec<u8>> + '_ {
        (match &mut self.ssl_state {
            ClientSslState::Handshake(mid_handshake) => {
                Some(mid_handshake.get_mut().outgoing_packets.drain(..))
            }
            ClientSslState::Established(ssl_stream)
            => {
                Some(ssl_stream.get_mut().outgoing_packets.drain(..))
            }
            ClientSslState::Shutdown => None,
        })
            .into_iter()
            .flatten()
    }
}

struct Server {
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

    pub async fn receive(&mut self, data: &[u8], remote: SocketAddr) {
        if let Some(client) = self.clients.get_mut(&remote) {
            client.read_packet(data).expect("Error reading packet");
            let outgoing_packets = client.take_outgoing_packets().map(|p| (p, remote));
            for (p, r) in outgoing_packets {
                self.socket.send_to(&p, r).await.unwrap();
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


#[derive(Debug)]
pub enum ClientError {
    NotConnected,
    NotEstablished,
    IncompletePacketRead,
    IncompletePacketWrite,
    OpenSslError(ErrorStack),

}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            ClientError::NotConnected => write!(f, "client is not connected"),
            ClientError::NotEstablished => {
                write!(f, "client does not have an established WebRTC data channel")
            }
            ClientError::IncompletePacketRead => {
                write!(f, "WebRTC connection packet not completely read")
            }
            ClientError::IncompletePacketWrite => {
                write!(f, "WebRTC connection packet not completely written")
            }
            ClientError::OpenSslError(stack) => {
                write!(f, "OpenSSL error {}", stack)
            }
        }
    }
}

impl std::error::Error for ClientError {}

#[derive(Debug)]
struct DTLSPacket {
    outgoing_packets: VecDeque<Vec<u8>>,
    incoming_packets: VecDeque<Vec<u8>>,
}

impl DTLSPacket {
    pub fn new() -> Self {
        DTLSPacket {
            incoming_packets: VecDeque::new(),
            outgoing_packets: VecDeque::new(),
        }
    }
}

impl Read for DTLSPacket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(packet) = self.incoming_packets.pop_front() {
            if packet.len() > buf.len() {
                return Err(Error::new(
                    ErrorKind::Other,
                    IncompletePacketRead,
                ));
            }
            buf[0..packet.len()].copy_from_slice(&packet);
            Ok(packet.len())
        } else {
            Err(ErrorKind::WouldBlock.into())
        }
    }
}

impl Write for DTLSPacket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        println!("writing");
        let buffer = Vec::from(buf);
        self.outgoing_packets.push_front(buffer);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

