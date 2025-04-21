use bytes::Bytes;
use log::{debug, trace};
use tokio::net::UdpSocket;

use crate::actors::MessageEvent;
use crate::actors::rust_hyper::start_http_server;
use crate::actors::session_master::SessionMaster;
use crate::config::get_global_config;
use crate::packet_type::{get_packet_type, PacketType};
use crate::stun::get_stun_packet;

mod acceptor;
mod actors;
mod client;
mod config;
mod http;
mod ice_registry;
mod media_header;
mod packet_type;
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
                    MessageEvent::NominateSession(_) => {}
                    MessageEvent::Test => {}
                    MessageEvent::InitStreamer(negotiated_session) => {
                        trace!(target: "Main","Assigning new streamer session {:?}", &negotiated_session);
                        master.add_streamer(negotiated_session);
                    }
                    MessageEvent::ForwardPacket(_) => {}
                }
            },
            Ok((bytes_read, remote_addr)) = udp_socket.recv_from(&mut buffer) => {
                let packet = Vec::from(&buffer[..bytes_read]);

                let packet_type = get_packet_type(Bytes::from(packet));
                match packet_type{
                    PacketType::RTP => {
                }
                    PacketType::RTCP => {}
                    PacketType::STUN => {

                    }
                    PacketType::Unknown => {}}
            }
        }
    }
}
