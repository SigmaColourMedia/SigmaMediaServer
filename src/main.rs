use std::{io, thread};
use std::io::{Read, Write};
use std::net::UdpSocket;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
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
    let (tx, mut rx) = mpsc::channel::<SessionCommand>(1000);


    thread::spawn(move || {
        let socket = UdpSocket::bind("127.0.0.1:52000").unwrap();
        socket.set_nonblocking(true).unwrap();
        let socket = Arc::new(socket);
        let mut server = Server::new(config.acceptor.clone(), socket.clone(), rx);
        loop {
            let mut buffer = [0; 3600];
            match socket.recv_from(&mut buffer) {
                Ok((bytes_read, remote_addr)) => {
                    server.listen(&buffer[..bytes_read], remote_addr);
                }
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                    // println!("would block")
                }
                Err(err) => {
                    eprintln!("Encountered socket IO error {}", err)
                }
            }
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


