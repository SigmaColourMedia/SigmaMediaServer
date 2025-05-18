use std::collections::HashSet;

use bytes::Bytes;
use log::{error, trace, warn};

use rtcp::{Unmarshall, unmarshall_compound_rtcp, UnmarshallError};
use rtcp::rtcp::RtcpPacket;

use crate::actors::dtls_actor::{CryptoResult, DTLSActorHandle};
use crate::actors::nack_responder::NackResponderActorHandle;
use crate::rtp_reporter::nack_to_lost_pids;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    ReadPacket(Vec<u8>),
}
struct ViewerMediaControlActor {
    dtls_handle: DTLSActorHandle,
    nack_handle: NackResponderActorHandle,
    receiver: Receiver,
}

impl ViewerMediaControlActor {
    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ReadPacket(packet) => {
                let (tx, rx) = tokio::sync::oneshot::channel::<CryptoResult>();
                self.dtls_handle
                    .sender
                    .send(crate::actors::dtls_actor::Message::DecodeSRTCP(packet, tx))
                    .unwrap();

                let decode_result = rx.await.unwrap();

                match decode_result {
                    Ok(packet) => {
                        let rtcp_packet = unmarshall_compound_rtcp(Bytes::from(packet));
                        match rtcp_packet {
                            Ok(packets) => {
                                for packet in packets {
                                    match packet {
                                        // Request packet resend for each lost PID
                                        RtcpPacket::TransportLayerFeedbackMessage(tl_nack) => {
                                            let lost_pids =
                                                tl_nack.nacks.iter().flat_map(nack_to_lost_pids);
                                            for pid in lost_pids {
                                                self.nack_handle.sender.send(crate::actors::nack_responder::Message::ResendPacket(pid)).unwrap()
                                            }
                                        }
                                        // Other packet types are unsupported
                                        other => {
                                            trace!(target: "Viewer Media Control Actor", "Received unsupported RTCP packet {:?}", other)
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                error!(target: "Viewer Media Control Actor", "Failed to unmarshall RTCP packet {:?}", err)
                            }
                        }
                    }
                    Err(err) => {
                        warn!(target: "Viewer Media Control Actor", "Failed to decrypt SRTCP packet {:?}", err)
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ViewerMediaControlActorHandle {
    pub sender: Sender,
}

impl ViewerMediaControlActorHandle {
    pub fn new(dtls_handle: DTLSActorHandle, nack_handle: NackResponderActorHandle) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = ViewerMediaControlActor {
            nack_handle,
            dtls_handle,
            receiver,
        };
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: ViewerMediaControlActor) {
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg).await;
    }

    trace!(target: "Viewer Media Control Actor", "Dropping Actor");
}
