use std::io::Read;

use bytes::Bytes;
use log::{debug, trace, warn};
use uuid::Uuid;

use rtcp::Unmarshall;
use sdp::NegotiatedSession;

use crate::actors;
use crate::actors::{get_main_bus, MessageEvent};
use crate::actors::dtls_actor::{CryptoResult, DTLSActorHandle};
use crate::actors::receiver_report_actor::ReceiverReportActorHandle;
use crate::actors::thumbnail_generator_actor::ThumbnailGeneratorActorHandle;
use crate::media_header::RTPHeader;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ReadPacket(Vec<u8>),
}

struct MediaIngestActor {
    receiver: Receiver,
    _room_id: Uuid,
    negotiated_session: NegotiatedSession,
    dtls_actor_handle: DTLSActorHandle,
    rr_actor_handle: ReceiverReportActorHandle,
    thumbnail_handle: ThumbnailGeneratorActorHandle,
}

impl MediaIngestActor {
    pub async fn handle_message(&self, message: Message) {
        match message {
            Message::ReadPacket(packet) => {
                let (tx, rx) = tokio::sync::oneshot::channel::<CryptoResult>();

                self.dtls_actor_handle
                    .sender
                    .send(actors::dtls_actor::Message::DecodeSRTP(packet, tx))
                    .unwrap();
                let decode_result = rx.await.unwrap();

                match decode_result {
                    Ok(packet) => {
                        let header =
                            RTPHeader::unmarshall(Bytes::copy_from_slice(&packet)).unwrap();
                        let media_ssrc_type =
                            get_media_ssrc_type(&self.negotiated_session, &header);
                        match media_ssrc_type {
                            MediaSSRCType::Video => {
                                self.thumbnail_handle
                                    .sender
                                    .send(actors::thumbnail_generator_actor::Message::ReadPacket(
                                        packet.clone(),
                                    ))
                                    .unwrap();
                                self.rr_actor_handle
                                    .sender
                                    .send(actors::receiver_report_actor::Message::FeedVideoRTP(
                                        header,
                                    ))
                                    .unwrap();
                                get_main_bus()
                                    .send(MessageEvent::ForwardToViewers(packet, self._room_id))
                                    .unwrap();
                            }
                            MediaSSRCType::Audio => {
                                get_main_bus()
                                    .send(MessageEvent::ForwardToViewers(packet, self._room_id))
                                    .unwrap();
                            }
                            MediaSSRCType::Unknown => {
                                warn!(target: "Media Ingest Actor", "Received unrecognized RTP payload type")
                            }
                        };
                    }
                    Err(err) => {
                        warn!(target: "Media Ingest Actor", "Error decoding SRTP packet {:?}", err);
                    }
                }
            }
        }
    }
}

enum MediaSSRCType {
    Video,
    Audio,
    Unknown,
}

fn get_media_ssrc_type(
    negotiated_session: &NegotiatedSession,
    rtp_header: &RTPHeader,
) -> MediaSSRCType {
    match rtp_header.payload_type {
        n if n == negotiated_session.video_session.payload_number as u8 => MediaSSRCType::Video,
        n if n == negotiated_session.audio_session.payload_number as u8 => MediaSSRCType::Audio,
        _ => MediaSSRCType::Unknown,
    }
}
#[derive(Debug)]
pub struct MediaIngestActorHandle {
    pub sender: Sender,
}

impl MediaIngestActorHandle {
    pub fn new(
        dtls_actor_handle: DTLSActorHandle,
        rr_actor_handle: ReceiverReportActorHandle,
        thumbnail_handle: ThumbnailGeneratorActorHandle,
        negotiated_session: NegotiatedSession,
        room_id: Uuid,
    ) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = MediaIngestActor {
            negotiated_session,
            receiver,
            dtls_actor_handle,
            rr_actor_handle,
            thumbnail_handle,
            _room_id: room_id,
        };
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: MediaIngestActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }
    trace!(target: "Media Ingest Actor", "Dropping Actor")
}
