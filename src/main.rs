use std::io::{ErrorKind, Read, Write};
use std::net::UdpSocket;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::acceptor::SSLConfig;
use crate::http::{HTTPServer, SessionCommand};
use crate::ice_registry::ConnectionType;
use crate::server::Server;

mod acceptor;
mod client;
mod http;
mod ice_registry;
mod rnd;
mod sdp;
mod server;
mod stun;

#[tokio::main]
async fn main() {
    let config = SSLConfig::new();
    let (tx, mut rx) = mpsc::channel::<SessionCommand>(1000);

    thread::spawn(move || {
        let socket = UdpSocket::bind(format!("{HOST_ADDRESS}:52000")).unwrap();
        socket.set_nonblocking(true).unwrap();

        let socket = Arc::new(socket);
        let mut server = Server::new(config.acceptor.clone(), socket.clone());
        loop {
            let mut buffer = [0; 3600];
            match socket.recv_from(&mut buffer) {
                Ok((bytes_read, remote_addr)) => {
                    server.listen(&buffer[..bytes_read], remote_addr);
                }
                Err(err) => match err.kind() {
                    ErrorKind::WouldBlock => match rx.try_recv() {
                        Ok(command) => match command {
                            SessionCommand::AddStreamer(session) => {
                                server.session_registry.add_streamer(session);
                            }
                            SessionCommand::AddViewer(session) => {
                                server.session_registry.add_viewer(session).unwrap();
                            }
                            SessionCommand::GetRooms(sender) => {
                                let rooms = server.session_registry.get_rooms();
                                sender.blocking_send(rooms).unwrap()
                            }
                            SessionCommand::GetStreamSDP((sender, stream_id)) => {
                                let stream_sdp = server
                                    .session_registry
                                    .get_session(&stream_id)
                                    .and_then(|session| match &session.connection_type {
                                        ConnectionType::Viewer(_) => None,
                                        ConnectionType::Streamer(streamer) => {
                                            Some(streamer.sdp.clone())
                                        }
                                    });
                                sender.send(stream_sdp).unwrap()
                            }
                        },
                        Err(_) => {
                            let sessions: Vec<_> = server
                                .session_registry
                                .get_all_sessions()
                                .iter()
                                .map(|&session| (session.id.clone(), session.ttl))
                                .collect();

                            println!("sessions count {}", sessions.len());

                            for (id, ttl) in sessions {
                                if ttl.elapsed() > Duration::from_secs(5) {
                                    server.session_registry.remove_session(&id);
                                    println!("your time is up {:?}", ttl.elapsed())
                                }
                            }
                        }
                    },
                    _ => {
                        eprintln!("Encountered socket IO error {}", err)
                    }
                },
            }
        }
    });

    let tcp_server = TcpListener::bind(format!("127.0.0.1:8080")).await.unwrap();
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

pub const HOST_ADDRESS: &'static str = env!("HOST_ADDRESS");
