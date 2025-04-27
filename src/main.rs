use bytes::Bytes;
use log::{debug, trace, warn};
use tokio::net::UdpSocket;

use crate::actors::get_packet_type::{get_packet_type, PacketType};
use crate::actors::MessageEvent;
use crate::actors::rust_hyper::start_http_server;
use crate::actors::session_master::{Session, SessionMaster};
use crate::actors::stun_actor::STUNMessage;
use crate::config::get_global_config;
use crate::stun::{get_stun_packet, ICEStunMessageType};

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

#[tokio::main]
async fn main() {
    env_logger::init();

    let mut master = SessionMaster::new();

    let event_producer_copy = master.master_channel_tx.clone();
    tokio::task::spawn(async move {
        start_http_server(event_producer_copy).await;
    });
    let udp_socket = UdpSocket::bind(get_global_config().udp_server_config.address)
        .await
        .unwrap();

    loop {
        let mut buffer = [0u8; 2500];

        tokio::select! {
            Some(message) = master.master_channel_rx.recv() => {
                match message {
                    MessageEvent::NominateSession(session_pointer) => {
                        trace!(target: "Main", "Nominating session {:?}",session_pointer);
                    }
                    MessageEvent::Test => {}
                    MessageEvent::InitStreamer(negotiated_session) => {
                        trace!(target: "Main","Assigning new streamer session {:?}", &negotiated_session);
                        master.add_streamer(negotiated_session);
                    }
                    MessageEvent::ForwardPacket((packet, remote)) => {
                        trace!(target: "Main", "Forwarding packet to remote socket {}", remote);
                        if let Err(err) = udp_socket.send_to(&packet, remote).await{
                            warn!(target: "Main", "Error sending packet to remote socket {}", remote);
                        }
                    }
                }
            },
            Ok((bytes_read, remote_addr)) = udp_socket.recv_from(&mut buffer) => {
                let packet = Vec::from(&buffer[..bytes_read]);

                let packet_type = get_packet_type(Bytes::from(packet));
                match packet_type{
                    PacketType::RTP(_) => {
                    }
                    PacketType::RTCP(_) => {}
                    PacketType::STUN(stun_type) => {
                        let stun_packet = match &stun_type
                        {
                            ICEStunMessageType::LiveCheck(packet) => {packet}
                            ICEStunMessageType::Nomination(packet) => {packet}
                        };
                        if let Some(session) = master.get_session_by_ice_username(&stun_packet.username_attribute){
                            let stun_actor_handle = match session{Session::Streamer(streamer_session) => {&streamer_session.stun_actor_handle}};
                            let ice_credentials = match session{Session::Streamer(streamer_session) => {streamer_session.negotiated_session.ice_credentials.clone()}};

                            let actor_message = STUNMessage{
                                ice_credentials, packet: stun_packet.clone(),socket_addr: remote_addr
                            };
                            match stun_type{
                                ICEStunMessageType::LiveCheck(_) => {stun_actor_handle.sender.send(actors::stun_actor::Message::LiveCheck(actor_message)).await.unwrap()}
                                ICEStunMessageType::Nomination(_) => {stun_actor_handle.sender.send(actors::stun_actor::Message::Nominate(actor_message)).await.unwrap()}
                            };
                        }
                    }
                    PacketType::Unknown => {
                         trace!(target: "Main", "Incoming unknown packet");
                    }
                }
            }
        }
    }
}
