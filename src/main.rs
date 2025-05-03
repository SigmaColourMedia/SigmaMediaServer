use std::sync::OnceLock;

use bytes::Bytes;
use log::{debug, trace, warn};
use tokio::net::UdpSocket;

use crate::actors::get_packet_type::{get_packet_type, PacketType};
use crate::actors::MessageEvent;
use crate::actors::rust_hyper::start_http_server;
use crate::actors::session_master::{NominatedSession, SessionMaster, UnsetSession};
use crate::config::get_global_config;
use crate::stun::ICEStunMessageType;

mod acceptor;
mod actors;
mod client;
mod config;
mod http;
mod ice_registry;
mod media_header;
mod rtp_replay_buffer;
mod rtp_reporter;
mod server;
mod stun;
mod thumbnail;

static EVENT_BUS: OnceLock<tokio::sync::mpsc::Sender<MessageEvent>> = OnceLock::new();

#[tokio::main]
async fn main() {
    env_logger::init();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<MessageEvent>(10000);
    EVENT_BUS.set(tx.clone()).unwrap();

    let mut master = SessionMaster::new();

    tokio::task::spawn(async move {
        start_http_server().await;
    });
    let udp_socket = UdpSocket::bind(get_global_config().udp_server_config.address)
        .await
        .unwrap();

    loop {
        let mut buffer = [0u8; 2500];

        tokio::select! {
            Some(message) = rx.recv() => {
                match message {
                    MessageEvent::NominateSession(session_pointer) => {
                        debug!(target: "Main", "Nominating session with ICE-host username:{}",session_pointer.session_username.host);
                        master.nominate_session(session_pointer);
                    }
                    MessageEvent::Test => {}
                    MessageEvent::InitStreamer(negotiated_session) => {
                        debug!(target: "Main","Assigning new streamer session with ICE-host username:{}", &negotiated_session.ice_credentials.host_username);
                        master.add_streamer(negotiated_session);
                    }
                    MessageEvent::ForwardPacket((packet, remote)) => {
                        if let Err(_err) = udp_socket.send_to(&packet, remote).await {
                            warn!(target: "Main", "Error sending packet to remote socket {}", remote);
                        }
                    }
                }
            },
            Ok((bytes_read, remote_addr)) = udp_socket.recv_from(&mut buffer) => {
                let packet = Vec::from(&buffer[..bytes_read]);

                let packet_type = get_packet_type(Bytes::from(packet.clone()));
                match packet_type{
                    PacketType::RTP(_) => {
                        if let Some(session) = master.get_session(&remote_addr){
                            if let NominatedSession::Streamer(streamer) = session{
                                streamer.media_digest_actor_handle.sender.send(actors::media_digest_actor::Message::ReadPacket(packet)).await.unwrap()
                            }
                        }
                    }
                    PacketType::RTCP(_) => {}
                    PacketType::STUN(stun_type) => {
                        let session_username = match &stun_type
                        {
                            ICEStunMessageType::LiveCheck(packet) => {&packet.username_attribute}
                            ICEStunMessageType::Nomination(packet) => {&packet.username_attribute}
                        };

                        // Check for unset-session traffic
                        if let Some(session) = master.get_unset_session(session_username){
                            let stun_handle = match session{UnsetSession::Streamer(streamer) => {&streamer.stun_actor_handle}UnsetSession::Viewer(viewer) => {&viewer.stun_actor_handle}};
                            stun_handle.sender.send(actors::unset_stun_actor::Message::ReadPacket(stun_type, remote_addr)).await.unwrap();
                        }
                        // Check for nominated-session live checks
                        else if let Some(session) = master.get_session(&remote_addr){
                            let stun_handle = match session{NominatedSession::Streamer(streamer) => {&streamer.stun_actor_handle}};
                            stun_handle.sender.send(actors::nominated_stun_actor::Message::ReadPacket(stun_type, remote_addr)).await.unwrap();
                        }
                    }
                    // Forward packets for DTLS Establishment
                    PacketType::Unknown => {
                        if let Some(session) = master.get_session(&remote_addr){
                            let dtls_actor_handle = match session{NominatedSession::Streamer(streamer) => {&streamer.dtls_actor}};
                            dtls_actor_handle.sender.send(actors::dtls_actor::Message::ReadPacket(packet)).await.unwrap()
                        }
                    }
                }
            }
        }
    }
}
