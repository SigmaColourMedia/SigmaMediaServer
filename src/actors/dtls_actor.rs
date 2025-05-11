use std::{io, mem};
use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Read, Write};

use log::{debug, trace, warn};
use openssl::ssl::{HandshakeError, MidHandshakeSslStream};
use srtp::openssl::{Config, InboundSession, OutboundSession};

use crate::actors::session_socket_actor::SessionSocketActorHandle;
use crate::config::get_global_config;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ReadPacket(Vec<u8>),
    DecodeSRTP(Vec<u8>, tokio::sync::oneshot::Sender<CryptoResult>),
    EncodeRTCP(Vec<u8>, tokio::sync::oneshot::Sender<CryptoResult>),
}

pub type CryptoResult = Result<Vec<u8>, CryptoError>;

#[derive(Debug)]
pub enum CryptoError {
    InvalidSSLState,
    EncodingError(srtp::Error),
    DecodingError(srtp::Error),
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
    fn new(receiver: Receiver, session_socket_actor_handle: SessionSocketActorHandle) -> Self {
        let dtls_negotiator = DTLSNegotiator::new(session_socket_actor_handle);
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
                                
                                debug!(target: "DTLS Actor", "DTLS handshake complete");

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
                        Err(err) => oneshot.send(Err(CryptoError::DecodingError(err))).unwrap(),
                    }
                }
                _ => {
                    oneshot.send(Err(CryptoError::InvalidSSLState)).unwrap();
                }
            },
            Message::EncodeRTCP(mut packet, oneshot) => match &mut self.ssl_stream {
                SSLStream::Established(srtp_session) => {
                    match srtp_session.outbound_session.protect_rtcp(&mut packet) {
                        Ok(_) => oneshot.send(Ok(packet)).unwrap(),
                        Err(err) => oneshot.send(Err(CryptoError::EncodingError(err))).unwrap(),
                    }
                }
                _ => {
                    oneshot.send(Err(CryptoError::InvalidSSLState)).unwrap();
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
    pub fn new(session_socket_actor_handle: SessionSocketActorHandle) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = DTLSActor::new(receiver, session_socket_actor_handle);
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: DTLSActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }

    debug!(target: "DTLS Actor", "Dropping Actor");
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
    session_socket_handle: SessionSocketActorHandle,
    buffer: VecDeque<Vec<u8>>,
}

impl DTLSNegotiator {
    pub fn new(session_socket_actor_handle: SessionSocketActorHandle) -> Self {
        Self {
            session_socket_handle: session_socket_actor_handle,
            buffer: VecDeque::new(),
        }
    }
}

impl Write for DTLSNegotiator {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.session_socket_handle
            .sender
            .send(crate::actors::session_socket_actor::Message::ForwardPacket(
                buf.to_vec(),
            ))
            .unwrap();
        Ok(buf.len())
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
