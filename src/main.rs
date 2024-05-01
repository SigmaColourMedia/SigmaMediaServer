use std::io::{Read, Write};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::mpsc;

use crate::acceptor::SSLConfig;
use crate::http::{HTTPServer, SessionCommand};
use crate::server::Server;

mod acceptor;
mod server;
mod client;
mod stun;
mod http;
mod ice_registry;
mod sdp;
mod rnd;

#[tokio::main]
async fn main() {
    let config = SSLConfig::new();
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:52000").await.unwrap());
    let remote_socket = socket.clone();
    let (tx, mut rx) = mpsc::channel::<SessionCommand>(1000);

    let mut server = Server::new(config.acceptor.clone(), remote_socket, rx);


    tokio::spawn(async move {
        loop {
            let mut buffer = [0; 3600];
            let (bytes_read, remote_addr) = socket.recv_from(&mut buffer).await.unwrap();
            server.listen(&buffer[..bytes_read], remote_addr).await;
        };
    });


    let tcp_server = TcpListener::bind("localhost:8080").await.unwrap();
    let http_server = Arc::new(HTTPServer::new(config.fingerprint.clone(), tx.clone()));

    loop {
        for (mut stream, remote) in tcp_server.accept().await {
            let http_server = http_server.clone();
            tokio::spawn(async move {
                http_server.handle_http_request(stream).await;
            });
        }
    }
}


