use std::{io, mem};
use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::SocketAddr;

use log::{debug, trace, warn};
use openssl::ssl::{HandshakeError, MidHandshakeSslStream};
use srtp::openssl::{Config, InboundSession, OutboundSession};

use crate::actors::{EventProducer, MessageEvent};
use crate::config::get_global_config;
use crate::EVENT_BUS;

type Sender = tokio::sync::mpsc::Sender<Message>;
type Receiver = tokio::sync::mpsc::Receiver<Message>;

pub enum Message {
    ReadPacket(Vec<u8>),
    DecodeSRTP(Vec<u8>, tokio::sync::oneshot::Sender<SRTPDecodeResult>),
}

pub type SRTPDecodeResult = Result<Vec<u8>, DecodeError>;

#[derive(Debug)]
pub enum DecodeError {
    InvalidSSLState,
    SRTPError,
}

/*
- Establish DTLS connection
- Decode SRTP/SRTCP packets
- Encode RTP/RTCP packets
 */
struct DTLSActor {
    receiver: Receiver,
    ssl_stream: SSLStream,
}

impl DTLSActor {
    fn new(receiver: Receiver, socket_addr: SocketAddr) -> Self {
        let dtls_negotiator = DTLSNegotiator::new(socket_addr);
        let mid_handshake_stream = get_global_config()
            .ssl_config
            .acceptor
            .accept(dtls_negotiator);

        if let Err(HandshakeError::WouldBlock(mid_handshake)) = mid_handshake_stream {
            Self {
                ssl_stream: SSLStream::MidHandshake(mid_handshake),
                receiver,
            }
        } else {
            panic!(
                "Should open mid-handshake SSL stream {:?}",
                mid_handshake_stream
            );
        }
    }

    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ReadPacket(packet) => {
                let ssl_stream = mem::replace(&mut self.ssl_stream, SSLStream::Shutdown);
                let ssl_stream = match ssl_stream {
                    SSLStream::MidHandshake(mut mid_handshake) => {
                        mid_handshake.get_mut().buffer.push_back(packet);

                        match mid_handshake.handshake() {
                            Ok(srtp_stream) => {
                                let (inbound, outbound) = srtp::openssl::session_pair(
                                    srtp_stream.ssl(),
                                    Config {
                                        window_size: 512,
                                        encrypt_extension_headers: &vec![],
                                        allow_repeat_tx: true,
                                    },
                                )
                                .expect("SRTP Session Pair setup failure");

                                debug!(target: "DTLS Actor", "DTLS setup complete with remote: {}", srtp_stream.get_ref().socket_addr);

                                SSLStream::Established(SRTPSessionPair {
                                    outbound_session: outbound,
                                    inbound_session: inbound,
                                })
                            }
                            Err(err) => match err {
                                HandshakeError::SetupFailure(err) => {
                                    debug!(target: "DTLS Actor", "DTLS Setup failure: {}", err);
                                    SSLStream::Shutdown
                                }
                                HandshakeError::Failure(err) => {
                                    debug!(target: "DTLS Actor", "Handshake failure: {:?}", err);
                                    SSLStream::Shutdown
                                }
                                HandshakeError::WouldBlock(ssl_stream) => {
                                    SSLStream::MidHandshake(ssl_stream)
                                }
                            },
                        }
                    }
                    SSLStream::Shutdown => {
                        trace!(target: "DTLS Actor", "Message received in SSLStream:Shutdown state");
                        SSLStream::Shutdown
                    }
                    SSLStream::Established(srtp_session) => {
                        trace!(target: "DTLS Actor", "Message received in SSLStream:Established state");
                        SSLStream::Established(srtp_session)
                    }
                };
                self.ssl_stream = ssl_stream
            }
            Message::DecodeSRTP(mut srtp_packet, oneshot) => match &mut self.ssl_stream {
                SSLStream::Established(srtp_session) => {
                    match srtp_session.inbound_session.unprotect(&mut srtp_packet) {
                        Ok(_) => oneshot.send(Ok(srtp_packet)).unwrap(),
                        Err(_) => oneshot.send(Err(DecodeError::SRTPError)).unwrap(),
                    }
                }
                _ => {
                    oneshot.send(Err(DecodeError::InvalidSSLState)).unwrap();
                }
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct DTLSActorHandle {
    pub sender: Sender,
}

impl DTLSActorHandle {
    pub fn new(socket_addr: SocketAddr) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel::<Message>(100);
        let actor = DTLSActor::new(receiver, socket_addr);
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: DTLSActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
}

enum SSLStream {
    MidHandshake(MidHandshakeSslStream<DTLSNegotiator>),
    Established(SRTPSessionPair),
    Shutdown,
}

struct SRTPSessionPair {
    inbound_session: InboundSession,
    outbound_session: OutboundSession,
}
#[derive(Debug)]
struct DTLSNegotiator {
    socket_addr: SocketAddr,
    buffer: VecDeque<Vec<u8>>,
}

impl DTLSNegotiator {
    pub fn new(socket_addr: SocketAddr) -> Self {
        Self {
            socket_addr,
            buffer: VecDeque::new(),
        }
    }
}

impl Write for DTLSNegotiator {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match EVENT_BUS
            .get()
            .unwrap()
            .try_send(MessageEvent::ForwardPacket((
                buf.to_vec(),
                self.socket_addr,
            ))) {
            Ok(_) => Ok(buf.len()),
            Err(err) => {
                warn!(target: "DTLSNegotiator", "Error writing to event_producer channel {}", err);
                Err(Error::from(ErrorKind::ConnectionAborted))
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for DTLSNegotiator {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(packet) = self.buffer.pop_front() {
            if packet.len() > buf.len() {
                return Err(Error::from(ErrorKind::InvalidData));
            }
            buf[0..packet.len()].copy_from_slice(&packet);
            Ok(packet.len())
        } else {
            Err(ErrorKind::WouldBlock.into())
        }
    }
}
