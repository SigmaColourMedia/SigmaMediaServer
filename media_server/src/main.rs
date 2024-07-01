use std::future::Future;
use std::net::UdpSocket;
use std::sync::{Arc, OnceLock};
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::time::Duration;

use openssl::stack::Stackable;
use threadpool::ThreadPool;

use crate::config::{Config, get_global_config};
use crate::http::routes::rooms::rooms_route;
use crate::http::routes::whep::whep_route;
use crate::http::routes::whip::whip_route;
use crate::http::server_builder::ServerBuilder;
use crate::http::SessionCommand;
use crate::http_server::HttpServer;
use crate::ice_registry::ConnectionType;
use crate::server::UDPServer;

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

fn main() {
    let (tx, mut rx) = std::sync::mpsc::channel::<SessionCommand>();

    let config = Config::initialize(tx.clone());
    GLOBAL_CONFIG.set(config);

    thread::spawn(start_http_server);
    let socket = build_udp_socket();
    let mut server = UDPServer::new(socket.try_clone().unwrap());

    thread::spawn(move || {
        let socket = socket.try_clone().unwrap();
        let sender = &get_global_config().session_command_sender;
        loop {
            let mut buffer = [0; 3600];

            if let Ok((bytes_read, remote)) = socket.recv_from(&mut buffer) {
                sender
                    .send(SessionCommand::HandlePacket(
                        Vec::from(&buffer[..bytes_read]),
                        remote,
                    ))
                    .expect("Command channel should be open")
            }
        }
    });

    loop {
        match rx.try_recv() {
            Ok(command) => match command {
                SessionCommand::HandlePacket(packet, remote) => {
                    server.process_packet(&packet, remote)
                }
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
                    let stream_sdp =
                        server
                            .session_registry
                            .get_session(&stream_id)
                            .and_then(|session| match &session.connection_type {
                                ConnectionType::Viewer(_) => None,
                                ConnectionType::Streamer(streamer) => Some(streamer.sdp.clone()),
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
        }
    }
}

fn start_http_server() {
    let pool = ThreadPool::new(4);

    let server = Arc::new(build_http_server());

    loop {
        let server = server.clone();
        if let Ok(stream) = server.read_stream() {
            pool.execute(move || {
                server.handle_stream(stream);
            });
        }
    }
}

fn build_http_server() -> HttpServer {
    let mut server_builder = ServerBuilder::new();
    server_builder.add_handler("/whip", |req| whip_route(req));
    server_builder.add_handler("/rooms", |req| rooms_route(req));
    server_builder.add_handler("/whep", |req| whep_route(req));

    server_builder.build()
}

fn build_udp_socket() -> UdpSocket {
    let global_config = get_global_config();
    let socket = UdpSocket::bind(global_config.udp_server_config.address).unwrap();
    println!(
        "Running UDP server at {}",
        global_config.udp_server_config.address
    );
    socket
}

pub const CERT_PATH: &'static str = "../certs/cert.pem";
pub const CERT_KEY_PATH: &'static str = "../certs/key.pem";
