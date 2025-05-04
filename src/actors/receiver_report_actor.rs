use std::net::SocketAddr;
use std::time::Duration;

use log::{debug, warn};
use tokio::select;
use tokio::time::Instant;

use sdp::NegotiatedSession;

use crate::actors::{get_event_bus, MessageEvent};
use crate::actors::dtls_actor::{CryptoResult, DTLSActorHandle};
use crate::media_header::RTPHeader;
use crate::rtp_reporter::RTPReporter;

type Sender = tokio::sync::mpsc::UnboundedSender<Message>;
type Receiver = tokio::sync::mpsc::UnboundedReceiver<Message>;

pub enum Message {
    FeedRTP(RTPHeader),
    SendReport,
}

struct ReceiverReportActor {
    receiver: Receiver,
    rtp_reporter: Option<RTPReporter>,
    dtls_actor_handle: DTLSActorHandle,
    socket_addr: SocketAddr,
    _host_ssrc: u32,
}

impl ReceiverReportActor {
    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::FeedRTP(header) => match self.rtp_reporter.as_mut() {
                None => {
                    let _ = self.rtp_reporter.insert(RTPReporter::new(
                        header.seq,
                        self._host_ssrc,
                        header.ssrc,
                    ));
                }
                Some(rtp_reporter) => {
                    rtp_reporter.feed_rtp(header);
                }
            },
            Message::SendReport => {
                if let Some(rtp_reporter) = self.rtp_reporter.as_mut() {
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
                            debug!(target: "RR Actor","Forwarding RTCP RR to {}", self.socket_addr);

                            get_event_bus()
                                .send(MessageEvent::ForwardPacket((
                                    encoded_packet,
                                    self.socket_addr,
                                )))
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
        negotiated_session: &NegotiatedSession,
        socket_addr: SocketAddr,
        dtls_actor_handle: DTLSActorHandle,
    ) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let actor = ReceiverReportActor {
            receiver,
            dtls_actor_handle,
            rtp_reporter: None,
            socket_addr,
            _host_ssrc: negotiated_session.video_session.host_ssrc,
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
            Some(msg) = actor.receiver.recv() => {
                actor.handle_message(msg).await;
            },
            () = &mut sleep => {
                actor.handle_message(Message::SendReport).await;
                sleep.as_mut().reset(Instant::now() + Duration::from_millis(1));
            },
            else => {
                debug!("exiting from loop");
                break
            }
        }
    }
}
