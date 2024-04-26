use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::acceptor::SSLConfig;
use crate::server::Server;

mod acceptor;
mod server;
mod client;
mod stun;

#[tokio::main]
async fn main() {
    let config = SSLConfig::new();
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:52000").await.unwrap());
    let remote_socket = socket.clone();
    let (tx, mut rx) = mpsc::channel::<(Vec<u8>, SocketAddr)>(1000);

    let mut server = Server::new(config.acceptor.clone(), remote_socket);

    tokio::spawn(async move {
        while let Some((bytes, addr)) = rx.recv().await {
            server.listen(&bytes, addr).await;
        }
    });

    loop {
        let mut buffer = [0; 3600];
        let (bytes_read, remote_addr) = socket.recv_from(&mut buffer).await.unwrap();
        tx.send((Vec::from(&buffer[..=bytes_read]), remote_addr)).await.unwrap();
    };
}


