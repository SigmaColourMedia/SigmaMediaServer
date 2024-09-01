use std::future::Future;
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use openssl::stack::Stackable;
use threadpool::ThreadPool;

use notification_bus::{Notification, NotificationBusBuilder, Room};

use crate::config::get_global_config;
use crate::http::routes::rooms::rooms_route;
use crate::http::routes::whep::whep_route;
use crate::http::routes::whip::whip_route;
use crate::http::server_builder::ServerBuilder;
use crate::http::ServerCommand;
use crate::server::UDPServer;

mod acceptor;
mod client;
mod config;
mod http;
mod http_server;
mod ice_registry;
mod rnd;
mod rtp;
mod server;
mod stun;
mod thumbnail_image;

fn main() {
    let (server_command_sender, server_command_receiver) =
        std::sync::mpsc::channel::<ServerCommand>();
    let socket = build_udp_socket();
    let mut server = UDPServer::new(socket.try_clone().unwrap());
    let notification_bus = NotificationBusBuilder::new()
        .add_address(get_global_config().notification_bus_config.address)
        .add_cors_origin("http://localhost:9000".to_string())
        .build();
    let notification_sender = notification_bus.get_sender();

    thread::spawn({
        let sender = server_command_sender.clone();
        move || start_http_server(sender)
    });
    thread::spawn({
        let sender = server_command_sender.clone();
        let socket = socket.try_clone().unwrap();
        move || start_udp_server(socket, sender)
    });
    thread::spawn({
        let sender = server_command_sender.clone();
        move || start_session_timeout_counter(sender)
    });
    thread::spawn(move || notification_bus.startup());
    thread::spawn(move || {
        let sender = server_command_sender.clone();
        start_notification_poll(sender)
    });

    loop {
        match server_command_receiver
            .recv()
            .expect("Server channel should be open")
        {
            ServerCommand::HandlePacket(packet, remote) => server.process_packet(&packet, remote),
            ServerCommand::AddStreamer(sdp_offer, response_tx) => {
                let negotiated_session = server.sdp_resolver.accept_stream_offer(&sdp_offer).ok();

                let response = negotiated_session.map(|session| {
                    let sdp_answer = String::from(session.sdp_answer.clone());
                    server.session_registry.add_streamer(session);
                    sdp_answer
                });

                response_tx
                    .send(response)
                    .expect("Response channel should remain open")
            }
            ServerCommand::AddViewer(sdp_offer, target_id, response_tx) => {
                let streamer_session = server
                    .session_registry
                    .get_room(target_id)
                    .map(|room| room.owner_id)
                    .map(|owner_id| {
                        server
                            .session_registry
                            .get_session(owner_id)
                            .map(|session| &session.media_session)
                    })
                    .flatten();

                let viewer_media_session = streamer_session.and_then(|media_session| {
                    server
                        .sdp_resolver
                        .accept_viewer_offer(&sdp_offer, media_session)
                        .ok()
                });
                let response = viewer_media_session.and_then(|media_session| {
                    let sdp_answer = String::from(media_session.sdp_answer.clone());
                    server.session_registry.add_viewer(media_session, target_id);
                    Some(sdp_answer)
                });

                response_tx
                    .send(response)
                    .expect("Response channel should remain open")
            }
            ServerCommand::GetRooms(sender) => {
                let rooms = server.session_registry.get_room_ids();
                sender.send(rooms).unwrap()
            }
            ServerCommand::SendRoomsStatus => {
                let rooms = server.session_registry.get_rooms();
                let notification = Notification {
                    rooms: rooms
                        .into_iter()
                        .map(|room| Room {
                            viewer_count: room.viewer_ids.len(),
                            id: room.id,
                        })
                        .collect::<Vec<_>>(),
                };
                notification_sender.send(notification).unwrap();
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
                        server.session_registry.remove_session(id);
                    }
                }
            }
        }
    }
}

fn start_notification_poll(sender: Sender<ServerCommand>) {
    let mut time_reference = Instant::now();
    loop {
        if time_reference.elapsed().gt(&Duration::from_secs(1)) {
            sender
                .send(ServerCommand::SendRoomsStatus)
                .expect("Server channel should be open");
            time_reference = Instant::now()
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

fn start_udp_server(socket: UdpSocket, sender: Sender<ServerCommand>) {
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
