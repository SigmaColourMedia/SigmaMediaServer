use std::{io, mem};
use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Read, Write};

use log::{debug, trace};
use openssl::ssl::{HandshakeError, MidHandshakeSslStream};
use srtp::openssl::{Config, InboundSession, OutboundSession};

use crate::actors::session_socket_actor::SessionSocketActorHandle;
use crate::config::get_global_config;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;
type Oneshot<T> = tokio::sync::oneshot::Sender<T>;

pub type CryptoOneshot = Oneshot<CryptoResult>;
pub type CryptoResult = Result<Vec<u8>, CryptoError>;

pub enum Message {
    ReadPacket(Vec<u8>),
    DecodeSRTP(Vec<u8>, CryptoOneshot),
    DecodeSRTCP(Vec<u8>, CryptoOneshot),
    EncodeRTCP(Vec<u8>, CryptoOneshot),
    EncodeRTP(Vec<u8>, CryptoOneshot),
}

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
    crypto: Crypto,
}

impl DTLSActor {
    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ReadPacket(packet) => self.crypto.read_packet(packet),
            Message::DecodeSRTP(packet, oneshot) => {
                oneshot.send(self.crypto.decode_srtp(packet)).unwrap()
            }
            Message::DecodeSRTCP(packet, oneshot) => {
                oneshot.send(self.crypto.decode_srtcp(packet)).unwrap()
            }
            Message::EncodeRTCP(packet, oneshot) => {
                oneshot.send(self.crypto.encode_rtcp(packet)).unwrap()
            }
            Message::EncodeRTP(packet, oneshot) => {
                oneshot.send(self.crypto.encode_rtp(packet)).unwrap()
            }
        }
    }
}

struct Crypto {
    ssl_stream: SSLStream,
}

impl Crypto {
    fn new(ss_handle: SessionSocketActorHandle) -> Self {
        let dtls_negotiator = DTLSNegotiator::new(ss_handle);
        let mid_handshake_stream = get_global_config()
            .ssl_config
            .acceptor
            .accept(dtls_negotiator)
            .unwrap_err();

        match mid_handshake_stream {
            HandshakeError::SetupFailure(err) => {
                panic!("DTLS setup failure {}", err);
            }
            HandshakeError::Failure(mid_handshake) => Self {
                ssl_stream: SSLStream::MidHandshake(mid_handshake),
            },
            HandshakeError::WouldBlock(mid_handshake) => Self {
                ssl_stream: SSLStream::MidHandshake(mid_handshake),
            },
        }
    }

    // todo Add support for reading DTLS packets
    fn read_packet(&mut self, packet: Vec<u8>) {
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

                        debug!(target: "Crypto", "DTLS handshake complete");

                        SSLStream::Established(SRTPSessionPair {
                            outbound_session: outbound,
                            inbound_session: inbound,
                        })
                    }
                    Err(err) => match err {
                        HandshakeError::SetupFailure(err) => {
                            debug!(target: "Crypto", "DTLS Setup failure: {}", err);
                            SSLStream::Shutdown
                        }
                        HandshakeError::Failure(ssl_stream) => {
                            debug!(target: "Crypto", "Handshake failure: {:?}", ssl_stream);
                            SSLStream::MidHandshake(ssl_stream)
                        }
                        HandshakeError::WouldBlock(ssl_stream) => {
                            SSLStream::MidHandshake(ssl_stream)
                        }
                    },
                }
            }
            SSLStream::Shutdown => {
                trace!(target: "Crypto", "Message received in SSLStream:Shutdown state");
                SSLStream::Shutdown
            }
            SSLStream::Established(srtp_session) => {
                trace!(target: "Crypto", "Message received in SSLStream:Established state");
                SSLStream::Established(srtp_session)
            }
        };
        self.ssl_stream = ssl_stream
    }

    fn decode_srtp(&mut self, mut packet: Vec<u8>) -> CryptoResult {
        match &mut self.ssl_stream {
            SSLStream::Established(ssl_stream) => ssl_stream
                .inbound_session
                .unprotect(&mut packet)
                .map(|_| packet)
                .map_err(|err| CryptoError::DecodingError(err)),
            _ => Err(CryptoError::InvalidSSLState),
        }
    }

    fn decode_srtcp(&mut self, mut packet: Vec<u8>) -> CryptoResult {
        match &mut self.ssl_stream {
            SSLStream::Established(ssl_stream) => ssl_stream
                .inbound_session
                .unprotect_rtcp(&mut packet)
                .map(|_| packet)
                .map_err(|err| CryptoError::DecodingError(err)),
            _ => Err(CryptoError::InvalidSSLState),
        }
    }
    fn encode_rtcp(&mut self, mut packet: Vec<u8>) -> CryptoResult {
        match &mut self.ssl_stream {
            SSLStream::Established(ssl_stream) => ssl_stream
                .outbound_session
                .protect_rtcp(&mut packet)
                .map(|_| packet)
                .map_err(|err| CryptoError::EncodingError(err)),
            _ => Err(CryptoError::InvalidSSLState),
        }
    }

    fn encode_rtp(&mut self, mut packet: Vec<u8>) -> CryptoResult {
        match &mut self.ssl_stream {
            SSLStream::Established(ssl_stream) => ssl_stream
                .outbound_session
                .protect(&mut packet)
                .map(|_| packet)
                .map_err(|err| CryptoError::EncodingError(err)),
            _ => Err(CryptoError::InvalidSSLState),
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
        let crypto = Crypto::new(session_socket_actor_handle);
        let actor = DTLSActor { crypto, receiver };
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: DTLSActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }

    trace!(target: "DTLS Actor", "Dropping Actor");
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
