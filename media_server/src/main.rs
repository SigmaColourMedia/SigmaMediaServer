use std::future::Future;
use std::io::ErrorKind;
use std::net::UdpSocket;
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

use openssl::stack::Stackable;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc::error::TryRecvError;

use crate::config::Config;
use crate::http::routes::rooms::rooms_route;
use crate::http::routes::whep::whep_route;
use crate::http::routes::whip::whip_route;
use crate::http::server_builder::ServerBuilder;
use crate::http::SessionCommand;
use crate::ice_registry::ConnectionType;
use crate::server::Server;

mod acceptor;
mod client;
mod config;
mod http;
mod http_legacy;
mod http_server;
mod ice_registry;
mod rnd;
mod sdp;
mod server;
mod stun;

pub static GLOBAL_CONFIG: OnceLock<Config> = OnceLock::new();

#[tokio::main]
async fn main() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<SessionCommand>(1000);

    let my_config = Config::initialize(tx.clone());
    GLOBAL_CONFIG.set(my_config);

    let global_config = GLOBAL_CONFIG.get().unwrap();

    thread::spawn(move || {
        let socket = UdpSocket::bind(global_config.udp_server_config.address).unwrap();
        println!(
            "Running UDP server at {}",
            global_config.udp_server_config.address
        );
        socket.set_nonblocking(true).unwrap();

        let socket = Arc::new(socket);
        let mut server = Server::new(socket.clone());
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
                                sender.send(rooms).unwrap()
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
    let mut server_builder = ServerBuilder::new();
    server_builder.add_handler("/whip", |req| Box::pin(whip_route(req)));
    server_builder.add_handler("/rooms", |req| Box::pin(rooms_route(req)));
    server_builder.add_handler("/whep", |req| Box::pin(whep_route(req)));

    let server = Arc::new(server_builder.build().await);

    loop {
        let server = server.clone();
        if let Ok(stream) = server.read_stream().await {
            tokio::spawn(async move {
                server.handle_stream(stream).await;
            });
        }
    }
}

pub const CERT_PATH: &'static str = "../certs/cert.pem";
pub const CERT_KEY_PATH: &'static str = "../certs/key.pem";
