use crate::acceptor::SSLConfig;
use crate::http::parsers::parse_http;
use crate::http::router::RouterBuilder;
use crate::http::routes::rooms::rooms;
use crate::http::routes::whip::whip;
use crate::http::SessionCommand;
use crate::ice_registry::ConnectionType;
use crate::server::Server;
use openssl::stack::Stackable;
use std::future::Future;
use std::io::ErrorKind;
use std::net::UdpSocket;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc::error::TryRecvError;

mod acceptor;
mod client;
mod http;
mod http_legacy;
mod ice_registry;
mod rnd;
mod sdp;
mod server;
mod stun;

#[tokio::main]
async fn main() {
    let config = SSLConfig::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<SessionCommand>(1000);

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

    let tcp_server = TcpListener::bind(format!("{HOST_ADDRESS}:8080"))
        .await
        .unwrap();
    println!("Running TCP server at {}:8080", HOST_ADDRESS);

    let mut router_builder = RouterBuilder::new();

    router_builder.add_handler("/whip", |req, fingerprint, sender| {
        Box::pin(whip(req, fingerprint, sender))
    });
    router_builder.add_handler("/rooms", |req, fingerprint, sender| {
        Box::pin(rooms(req, fingerprint, sender))
    });

    router_builder.add_fingerprint(config.fingerprint.clone());
    router_builder.add_sender(tx.clone());

    let router = Arc::new(router_builder.build());

    while let Ok((mut stream, _)) = tcp_server.accept().await {
        let router = router.clone();

        tokio::spawn(async move {
            let mut buffer = [0u8; 3000];
            stream
                .read(&mut buffer)
                .await
                .expect("Failed reading from buffer");
            if let Some(request) = parse_http(&buffer).await {
                router.handle_request(request, &mut stream).await;
            }
        });
    }
}

async fn handle_usize(a: usize, aa: &str) -> String {
    String::new()
}

pub const HOST_ADDRESS: &'static str = env!("HOST_ADDRESS");
pub const WHIP_TOKEN: &'static str = env!("WHIP_TOKEN");
pub const CERT_PATH: &'static str = "../certs/cert.pem";
pub const CERT_KEY_PATH: &'static str = "../certs/key.pem";
pub const HTML_PATH: &'static str = "../public/index.html";
pub const BUNDLE_PATH: &'static str = "../public/index.js";
pub const DISCORD_API_URL: &'static str = env!("DISCORD_API_URL");
