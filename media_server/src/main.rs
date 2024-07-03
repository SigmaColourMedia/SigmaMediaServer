use std::future::Future;
use std::net::UdpSocket;
use std::sync::mpsc::Sender;
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use openssl::stack::Stackable;
use threadpool::ThreadPool;

use crate::config::{get_global_config, Config};
use crate::http::routes::rooms::rooms_route;
use crate::http::routes::whep::whep_route;
use crate::http::routes::whip::whip_route;
use crate::http::server_builder::ServerBuilder;
use crate::http::ServerCommand;
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
    let (tx, mut rx) = std::sync::mpsc::channel::<ServerCommand>();
    let socket = build_udp_socket();
    let mut server = UDPServer::new(socket.try_clone().unwrap());

    thread::spawn({
        let sender = tx.clone();
        move || start_http_server(sender)
    });
    thread::spawn({
        let sender = tx.clone();
        let socket = socket.try_clone().unwrap();
        move || listen_on_udp_socket(socket, sender)
    });
    thread::spawn({
        let sender = tx.clone();
        move || start_session_timeout_counter(sender)
    });

    loop {
        match rx.recv().expect("Server channel should be open") {
            ServerCommand::HandlePacket(packet, remote) => server.process_packet(&packet, remote),
            ServerCommand::AddStreamer(session) => {
                server.session_registry.add_streamer(session);
            }
            ServerCommand::AddViewer(session) => {
                server.session_registry.add_viewer(session).unwrap();
            }
            ServerCommand::GetRooms(sender) => {
                let rooms = server.session_registry.get_rooms();
                sender.send(rooms).unwrap()
            }
            ServerCommand::GetStreamSDP((sender, stream_id)) => {
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
            ServerCommand::CheckForTimeout => {
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
        }
    }
}

fn start_session_timeout_counter(sender: Sender<ServerCommand>) {
    let mut time_reference = Instant::now();
    loop {
        if time_reference.elapsed().gt(&Duration::from_secs(3)) {
            sender
                .send(ServerCommand::CheckForTimeout)
                .expect("Server channel should be open");
            time_reference = Instant::now()
        }
    }
}

fn listen_on_udp_socket(socket: UdpSocket, sender: Sender<ServerCommand>) {
    loop {
        let mut buffer = [0; 3600];

        if let Ok((bytes_read, remote)) = socket.recv_from(&mut buffer) {
            sender
                .send(ServerCommand::HandlePacket(
                    Vec::from(&buffer[..bytes_read]),
                    remote,
                ))
                .expect("Command channel should be open")
        }
    }
}

fn start_http_server(command_sender: Sender<ServerCommand>) {
    let mut server_builder = ServerBuilder::new();
    server_builder.add_handler("/whip", |req, sender| whip_route(req, sender));
    server_builder.add_handler("/rooms", |req, sender| rooms_route(req, sender));
    server_builder.add_handler("/whep", |req, sender| whep_route(req, sender));
    server_builder.add_sender(command_sender);

    let pool = ThreadPool::new(4);

    let server = Arc::new(server_builder.build());

    loop {
        let server = server.clone();
        if let Ok(stream) = server.read_stream() {
            pool.execute(move || {
                server.handle_stream(stream);
            });
        }
    }
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
