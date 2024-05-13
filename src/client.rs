use std::{fmt, io, mem};
use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use openssl::error::ErrorStack;
use openssl::ssl::{HandshakeError, MidHandshakeSslStream, SslAcceptor, SslStream};

use crate::client::ClientError::{IncompletePacketRead, OpenSslError};

#[derive(Debug)]
pub enum ClientSslState {
    Handshake(MidHandshakeSslStream<UDPPeerStream>),
    Established(SslStream<UDPPeerStream>),
    Shutdown,
}

#[derive(Debug)]
pub struct Client {
    pub ssl_state: ClientSslState,
    pub remote_address: SocketAddr,
}

impl Client {
    pub fn new(
        remote: SocketAddr,
        acceptor: Arc<SslAcceptor>,
        socket: Arc<UdpSocket>,
    ) -> Result<Self, ErrorStack> {
        let udp_stream = UDPPeerStream::new(socket, remote.clone());
        match acceptor.accept(udp_stream) {
            Ok(_) => unreachable!("handshake cannot finish with no incoming packets"),
            Err(HandshakeError::SetupFailure(err)) => return Err(err),
            Err(HandshakeError::Failure(_)) => {
                unreachable!("handshake cannot fail before starting")
            }
            Err(HandshakeError::WouldBlock(mid_handshake)) => Ok(Client {
                ssl_state: ClientSslState::Handshake(mid_handshake),
                remote_address: remote,
            }),
        }
    }

    pub fn read_packet(&mut self, packet: &[u8]) -> Result<(), ClientError> {
        self.ssl_state = match mem::replace(&mut self.ssl_state, ClientSslState::Shutdown) {
            ClientSslState::Handshake(mut mid_handshake) => {
                mid_handshake
                    .get_mut()
                    .incoming_packets
                    .push_back(Vec::from(packet));
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
                ssl_stream
                    .get_mut()
                    .incoming_packets
                    .push_back(Vec::from(packet));
                ClientSslState::Established(ssl_stream)
            }
            ClientSslState::Shutdown => ClientSslState::Shutdown,
        };

        while let ClientSslState::Established(ssl_stream) = &mut self.ssl_state {
            let mut my_buff = [0; 300];
            match ssl_stream.read(&mut my_buff) {
                Ok(size) => {
                    println!(
                        "read {}",
                        String::from_utf8(Vec::from(&my_buff[..size])).unwrap()
                    );
                    break;
                }
                Err(e) => {
                    println!("error reading packet {}", e);
                    break;
                }
            }
        }
        Ok(())
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
pub struct UDPPeerStream {
    socket: Arc<UdpSocket>,
    remote: SocketAddr,
    incoming_packets: VecDeque<Vec<u8>>,
}

impl UDPPeerStream {
    pub fn new(socket: Arc<UdpSocket>, remote: SocketAddr) -> Self {
        UDPPeerStream {
            incoming_packets: VecDeque::new(),
            socket,
            remote,
        }
    }
}

impl Read for UDPPeerStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(packet) = self.incoming_packets.pop_front() {
            if packet.len() > buf.len() {
                return Err(Error::new(ErrorKind::Other, IncompletePacketRead));
            }
            buf[0..packet.len()].copy_from_slice(&packet);
            Ok(packet.len())
        } else {
            Err(ErrorKind::WouldBlock.into())
        }
    }
}

impl Write for UDPPeerStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.socket
            .send_to(buf, self.remote)
            .and_then(|_| Ok(buf.len()))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
