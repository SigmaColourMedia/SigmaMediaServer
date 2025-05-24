use std::net::SocketAddr;
use std::sync::OnceLock;

use log::{info, warn};
use tokio::net::UdpSocket;

use crate::config::get_global_config;

static UDP_SOCKET: OnceLock<UdpSocket> = OnceLock::new();

pub async fn init_socket() {
    info!(target: "UDP", "Listening on {}", get_global_config().udp_server_config.address);

    let udp_socket = UdpSocket::bind(get_global_config().udp_server_config.address)
        .await
        .unwrap();

    UDP_SOCKET.set(udp_socket).unwrap();
}

pub async fn send_packet(packet: &[u8], remote: &SocketAddr) {
    if let Err(err) = get_socket().send_to(packet, remote).await {
        warn!(target: "UDP", "send_to failed to remote {} with err {}", remote, err)
    }
}

pub fn get_socket() -> &'static UdpSocket {
    UDP_SOCKET.get().unwrap()
}
