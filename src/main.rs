use std::sync::{Arc, OnceLock};
use std::time::Duration;

use bytes::Bytes;
use log::{debug, trace, warn};
use rand::random;
use tokio::net::UdpSocket;
use tokio::time::Instant;

use crate::actors::get_packet_type::{get_packet_type, PacketType};
use crate::actors::MessageEvent;
use crate::actors::rust_hyper::start_http_server;
use crate::actors::session_master::{NominatedSession, SessionMaster, UnsetSession};
use crate::actors::udp_io_actor::UDPIOActorHandle;
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

static EVENT_BUS: OnceLock<tokio::sync::mpsc::UnboundedSender<MessageEvent>> = OnceLock::new();

#[tokio::main]
async fn main() {
    env_logger::init();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<MessageEvent>();
    EVENT_BUS.set(tx).unwrap();

    let udp_socket = Arc::new(
        UdpSocket::bind(get_global_config().udp_server_config.address)
            .await
            .unwrap(),
    );
    let socket_io_actor_handle = UDPIOActorHandle::new(udp_socket.clone());

    let mut master = SessionMaster::new(socket_io_actor_handle);

    tokio::task::spawn(async move {
        start_http_server().await;
    });

    loop {
        let mut buffer = [0u8; 2500];

        tokio::select! {
            Some(message) = rx.recv() => {
                match message {
                    MessageEvent::NominateSession(session_pointer) => {
                        debug!(target: "Main", "Nominating session with ICE-host username:{}",session_pointer.session_username.host);
                        master.nominate_session(session_pointer);
                    }
                    MessageEvent::InitStreamer(negotiated_session) => {
                        master.add_streamer(negotiated_session);
                    }
                    MessageEvent::DebugSession(tx) => {
                        tx.send(format!("{:#?}", master)).unwrap();
                    }
                    MessageEvent::TerminateSession(id) => {
                        master.remove_session(id);
                    }
                    MessageEvent::GetRoomThumbnail(id,oneshot) => {
                        oneshot.send(master.get_room_thumbnail(id).await);
                    }}
            },
            Ok((bytes_read, remote_addr)) = udp_socket.recv_from(&mut buffer) => {
                let packet = Vec::from(&buffer[..bytes_read]);

                let packet_type = get_packet_type(Bytes::copy_from_slice(&packet));
                match packet_type{
                    PacketType::RTP(rtp_header) => {
                        if let Some(session) = master.get_session_mut(&remote_addr){
                            match session{
                                NominatedSession::Streamer(streamer) => {
                                    streamer.keepalive_handle.sender.send(actors::keepalive_actor::Message::UpdateTTL).unwrap();
                                    streamer.media_digest_actor_handle.sender.send(actors::media_ingest_actor::Message::ReadPacket(packet)).unwrap();
                                }
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
                        if let Some(session) = master.get_unset_session_mut(session_username){
                            let keepalive_handle = session.get_keepalive_handle();
                            let stun_handle = session.get_stun_handle();

                            keepalive_handle.sender.send(actors::keepalive_actor::Message::UpdateTTL).unwrap();
                            stun_handle.sender.send(actors::unset_stun_actor::Message::ReadPacket(stun_type, remote_addr)).unwrap();
                        }
                        // Check for nominated-session live checks
                        else if let Some(session) = master.get_session_mut(&remote_addr){
                            let stun_handle = session.get_stun_handle();
                            let keepalive_handle = session.get_keepalive_handle();

                            keepalive_handle.sender.send(actors::keepalive_actor::Message::UpdateTTL).unwrap();
                            stun_handle.sender.send(actors::nominated_stun_actor::Message::ReadPacket(stun_type, remote_addr)).unwrap();
                        }
                    }
                    // Forward packets for DTLS Establishment
                    PacketType::Unknown => {
                        if let Some(session) = master.get_session_mut(&remote_addr){
                            let keepalive_handle = session.get_keepalive_handle();
                            let dtls_actor_handle = session.get_dtls_handle();

                            keepalive_handle.sender.send(actors::keepalive_actor::Message::UpdateTTL).unwrap();
                            dtls_actor_handle.sender.send(actors::dtls_actor::Message::ReadPacket(packet)).unwrap()
                        }
                    }
                }
            },
        }
    }
}
