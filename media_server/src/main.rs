use std::io::ErrorKind;
use std::net::UdpSocket;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

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
        println!("Running UDP server at {}:52000", HOST_ADDRESS);
        socket.set_nonblocking(true).unwrap();

        let socket = Arc::new(socket);
        let mut server = Server::new(config.acceptor.clone(), socket.clone());
        loop {
            let mut buffer = [0; 3600];
            match socket.recv_from(&mut buffer) {
                // Check for packets
                Ok((bytes_read, remote_addr)) => {
                    server.listen(&buffer[..bytes_read], remote_addr);
                }
                Err(err) => match err.kind() {
                    // Check for commands
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
                        Err(channel_err) => match channel_err {
                            // Check for session timeouts
                            TryRecvError::Empty => {
                                let sessions: Vec<_> = server
                                    .session_registry
                                    .get_all_sessions()
                                    .iter()
                                    .map(|&session| (session.id.clone(), session.ttl))
                                    .collect();

                                // println!("sessions count {}", sessions.len());
                                for (id, ttl) in sessions {
                                    if ttl.elapsed() > Duration::from_secs(5) {
                                        server.session_registry.remove_session(&id);
                                    }
                                }
                            }
                            TryRecvError::Disconnected => {
                                panic!("Command channel unexpectedly closed.")
                            }
                        },
                    },
                    _ => {
                        eprintln!("Encountered socket IO error {}", err)
                    }
                },
            }
        }
    });

    let tcp_server = TcpListener::bind(format!("{HOST_ADDRESS}:8080"))
        .await
        .unwrap();
    println!("Running TCP server at {}:8080", HOST_ADDRESS);

    let http_server = Arc::new(HTTPServer::new(config.fingerprint.clone(), tx.clone()));

    loop {
        while let Ok((stream, remote)) = tcp_server.accept().await {
            let http_server = http_server.clone();
            tokio::spawn(async move {
                http_server.handle_http_request(stream, remote).await;
            });
        }
    }
}

pub const HOST_ADDRESS: &'static str = env!("HOST_ADDRESS");
pub const WHIP_TOKEN: &'static str = env!("WHIP_TOKEN");
pub const CERT_PATH: &'static str = "../certs/cert.pem";
pub const CERT_KEY_PATH: &'static str = "../certs/key.pem";
pub const HTML_PATH: &'static str = "../public/index.html";
pub const BUNDLE_PATH: &'static str = "../public/index.js";