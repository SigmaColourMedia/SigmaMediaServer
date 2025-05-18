use bytes::Bytes;
use log::{trace, warn};
use rand::random;

use rtcp::{Marshall, Unmarshall};
use sdp::NegotiatedSession;

use crate::actors::dtls_actor::{CryptoResult, DTLSActorHandle};
use crate::actors::nack_responder::NackResponderActorHandle;
use crate::actors::session_socket_actor::SessionSocketActorHandle;
use crate::media_header::RTPHeader;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ReadPacket(Vec<u8>),
}

struct MediaDigestActor {
    dtls_handle: DTLSActorHandle,
    socket_handle: SessionSocketActorHandle,
    nack_handle: NackResponderActorHandle,
    rtp_remap: RTPHeaderDecoder,
    receiver: Receiver,
}

impl MediaDigestActor {
    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ReadPacket(mut packet) => {
                let media_type = self.rtp_remap.remap_header(&mut packet);
                let (tx, rx) = tokio::sync::oneshot::channel::<CryptoResult>();
                self.dtls_handle
                    .sender
                    .send(crate::actors::dtls_actor::Message::EncodeRTP(packet, tx))
                    .unwrap();
                let encrypt_result = rx.await.unwrap();

                match encrypt_result {
                    Ok(packet) => {
                        match media_type {
                            RTPMediaType::Video => {
                                // Forward to viewer
                                if random::<u8>() > 60 {
                                    self.socket_handle
                                        .sender
                                        .send(crate::actors::session_socket_actor::Message::ForwardPacket(
                                            packet.clone(),
                                        ))
                                        .unwrap();
                                }

                                // Save for NACK
                                self.nack_handle
                                    .sender
                                    .send(crate::actors::nack_responder::Message::RegisterPacket(
                                        packet,
                                    ))
                                    .unwrap()
                            }
                            RTPMediaType::Audio => {
                                // Forward to viewer
                                self.socket_handle
                                    .sender
                                    .send(
                                        crate::actors::session_socket_actor::Message::ForwardPacket(
                                            packet.clone(),
                                        ),
                                    )
                                    .unwrap();
                            }
                        }
                    }
                    Err(err) => {
                        warn!(target: "Media Digest Actor", "Error encoding RTP packet {:?}", err);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MediaDigestActorHandle {
    pub sender: Sender,
}

impl MediaDigestActorHandle {
    pub fn new(
        socket_handle: SessionSocketActorHandle,
        dtls_handle: DTLSActorHandle,
        nack_handle: NackResponderActorHandle,
        host_session: NegotiatedSession,
        viewer_session: NegotiatedSession,
    ) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = MediaDigestActor {
            socket_handle,
            rtp_remap: RTPHeaderDecoder {
                viewer_session,
                host_session,
            },
            nack_handle,
            dtls_handle,
            receiver,
        };
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: MediaDigestActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }

    trace!(target: "Media Digest Actor", "Dropping Actor");
}

struct RTPHeaderDecoder {
    host_session: NegotiatedSession,
    viewer_session: NegotiatedSession,
}
impl RTPHeaderDecoder {
    // Remaps RTP Header in place and return packet type
    pub fn remap_header(&self, input: &mut Vec<u8>) -> RTPMediaType {
        let mut rtp_header = RTPHeader::unmarshall(Bytes::copy_from_slice(&input))
            .expect("Should receive valid RTP packet");
        match rtp_header.payload_type {
            pt if pt == self.host_session.video_session.payload_number as u8 => {
                rtp_header.payload_type = self.viewer_session.video_session.payload_number as u8;
                rtp_header.ssrc = self.viewer_session.video_session.host_ssrc;

                let header_bytes = rtp_header.marshall().unwrap();
                input[..header_bytes.len()].copy_from_slice(&header_bytes);
                RTPMediaType::Video
            }
            pt if pt == self.host_session.audio_session.payload_number as u8 => {
                rtp_header.payload_type = self.viewer_session.audio_session.payload_number as u8;
                rtp_header.ssrc = self.viewer_session.audio_session.host_ssrc;

                let header_bytes = rtp_header.marshall().unwrap();
                input[..header_bytes.len()].copy_from_slice(&header_bytes);
                RTPMediaType::Audio
            }
            _ => panic!("Invalid payload type received"),
        }
    }
}

enum RTPMediaType {
    Video,
    Audio,
}
