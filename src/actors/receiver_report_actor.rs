use std::net::SocketAddr;
use std::time::Duration;

use log::{debug, warn};
use tokio::select;
use tokio::time::Instant;

use sdp::NegotiatedSession;

use crate::actors::{get_event_bus, MessageEvent};
use crate::actors::dtls_actor::{CryptoResult, DTLSActorHandle};
use crate::actors::session_socket_actor::SessionSocketActorHandle;
use crate::media_header::RTPHeader;
use crate::rtp_reporter::RTPReporter;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    FeedVideoRTP(RTPHeader),
    SendReport,
}

struct ReceiverReportActor {
    receiver: Receiver,
    negotiated_session: NegotiatedSession,
    video_rtp_reporter: Option<RTPReporter>,
    dtls_actor_handle: DTLSActorHandle,
    session_socket_actor_handle: SessionSocketActorHandle,
}

impl ReceiverReportActor {
    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::FeedVideoRTP(header) => {
                    match self.video_rtp_reporter.as_mut() {
                        None => {
                            let _ = self.video_rtp_reporter.insert(RTPReporter::new(
                                header.seq,
                                self.negotiated_session.video_session.host_ssrc,
                                header.ssrc,
                            ));
                        }
                        Some(rtp_reporter) => {
                            rtp_reporter.feed_rtp(header);
                        }
                    }
                
            }
            Message::SendReport => {
                // Only video RR is supported
                if let Some(rtp_reporter) = self.video_rtp_reporter.as_mut() {
                    // todo See if there's a better way to indicate NACK presence
                    let (report, has_nack) = rtp_reporter.generate_receiver_report();

                    // Don't send report if no Generic NACK is present
                    if !has_nack {
                        return;
                    }

                    let report = report.to_vec();

                    let (tx, rx) = tokio::sync::oneshot::channel::<CryptoResult>();
                    self.dtls_actor_handle
                        .sender
                        .send(crate::actors::dtls_actor::Message::EncodeRTCP(report, tx))
                        .unwrap();

                    let crypto_result = rx.await.unwrap();

                    match crypto_result {
                        Ok(encoded_packet) => {
                            debug!(target: "RR Actor","Forwarding RTCP RR");

                            self.session_socket_actor_handle
                                .sender
                                .send(crate::actors::session_socket_actor::Message::ForwardPacket(
                                    encoded_packet,
                                ))
                                .unwrap();
                        }
                        Err(err) => {
                            warn!(target: "RR Actor", "Error encoding packet {:?}", err)
                        }
                    }
                }
            }
        }
    }
}

pub struct ReceiverReportActorHandle {
    pub sender: Sender,
}

impl ReceiverReportActorHandle {
    pub fn new(
        negotiated_session: NegotiatedSession,
        session_socket_handle: SessionSocketActorHandle,
        dtls_actor_handle: DTLSActorHandle,
    ) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = ReceiverReportActor {
            negotiated_session,
            session_socket_actor_handle: session_socket_handle,
            receiver,
            dtls_actor_handle,
            video_rtp_reporter: None,
        };
        tokio::spawn(run(actor));

        Self { sender }
    }
}
async fn run(mut actor: ReceiverReportActor) {
    let sleep = tokio::time::sleep(Duration::from_millis(1));
    tokio::pin!(sleep);
    loop {
        select! {
            msg_option = actor.receiver.recv() => {
                match msg_option{
                    None => {
                        debug!(target: "RR Actor", "Dropping Actor");
                        break
                    }
                    Some(msg) => {
                        actor.handle_message(msg).await;
                    }
                }
            },
            () = &mut sleep => {
                actor.handle_message(Message::SendReport).await;
                sleep.as_mut().reset(Instant::now() + Duration::from_millis(1));
            },
        }
    }
}
