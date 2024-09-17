use std::net::UdpSocket;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::config::get_global_config;
use crate::http::server::{Notification, Room, start_http_server};
use crate::http::ServerCommand;
use crate::ice_registry::ConnectionType;
use crate::server::UDPServer;
use crate::thumbnail::save_thumbnail_to_storage;

mod acceptor;
mod client;
mod config;
mod http;
mod ice_registry;
mod rnd;
mod rtp;
mod server;
mod stun;
mod thumbnail;

fn main() {
    let (server_command_sender, server_command_receiver) =
        std::sync::mpsc::channel::<ServerCommand>();
    let socket = build_udp_socket();
    let mut udp_server = UDPServer::new(socket.try_clone().unwrap());

    thread::spawn({
        let server_command_sender = server_command_sender.clone();
        move || start_http_server(server_command_sender)
    });
    thread::spawn({
        let sender = server_command_sender.clone();
        let socket = socket.try_clone().unwrap();
        move || start_udp_server(socket, sender)
    });
    thread::spawn({
        let sender = server_command_sender.clone();
        move || start_timeout_interval(sender)
    });

    loop {
        match server_command_receiver
            .recv()
            .expect("Server channel should be open")
        {
            ServerCommand::HandlePacket(packet, remote) => {
                udp_server.process_packet(&packet, remote)
            }
            ServerCommand::AddStreamer(sdp_offer, response_tx) => {
                let negotiated_session =
                    udp_server.sdp_resolver.accept_stream_offer(&sdp_offer).ok();

                let response = negotiated_session.map(|session| {
                    let sdp_answer = String::from(session.sdp_answer.clone());
                    udp_server.session_registry.add_streamer(session);
                    sdp_answer
                });

                response_tx
                    .send(response)
                    .expect("Response channel should remain open")
            }
            ServerCommand::AddViewer(sdp_offer, target_id, response_tx) => {
                let streamer_session = udp_server
                    .session_registry
                    .get_room(target_id)
                    .map(|room| room.owner_id)
                    .map(|owner_id| {
                        udp_server
                            .session_registry
                            .get_session(owner_id)
                            .map(|session| &session.media_session)
                    })
                    .flatten();

                let viewer_media_session = streamer_session.and_then(|media_session| {
                    udp_server
                        .sdp_resolver
                        .accept_viewer_offer(&sdp_offer, media_session)
                        .ok()
                });
                let response = viewer_media_session.and_then(|media_session| {
                    let sdp_answer = String::from(media_session.sdp_answer.clone());
                    udp_server
                        .session_registry
                        .add_viewer(media_session, target_id);
                    Some(sdp_answer)
                });

                response_tx
                    .send(response)
                    .expect("Response channel should remain open")
            }
            ServerCommand::SendRoomsStatus(reply_channel) => {
                let rooms = udp_server.session_registry.get_rooms();
                let notification = Notification {
                    rooms: rooms
                        .into_iter()
                        .map(|room| Room {
                            viewer_count: room.viewer_ids.len(),
                            id: room.id,
                        })
                        .collect::<Vec<_>>(),
                };
                reply_channel.send(notification);
            }
            ServerCommand::RunPeriodicChecks => {
                // todo Move these into separate functions

                // *** Save thumbnails ***

                // Get all ImageData of streamers that:
                // - Have an ImageData ready
                // - Have no thumbnail or enough time has passed for the thumbnail to be updated
                let thumbnails_to_update = udp_server
                    .session_registry
                    .get_all_sessions_mut()
                    .into_iter()
                    .filter_map(|session| match &mut session.connection_type {
                        ConnectionType::Viewer(_) => None,
                        ConnectionType::Streamer(streamer) => {
                            let should_update_thumbnail = streamer.image_timestamp.is_none()
                                || streamer
                                    .image_timestamp
                                    .unwrap()
                                    .elapsed()
                                    .gt(&Duration::from_secs(120));

                            if should_update_thumbnail
                                && streamer.thumbnail_extractor.last_picture.is_some()
                            {
                                // Update new thumbnail timestamp
                                streamer.image_timestamp = Some(Instant::now());
                                let last_picture = streamer
                                    .thumbnail_extractor
                                    .last_picture
                                    .as_ref()
                                    .unwrap()
                                    .clone();
                                return Some((streamer.owned_room_id, last_picture));
                            }
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                for (thumbnail_id, thumbnail_data) in thumbnails_to_update {
                    thread::spawn(move || save_thumbnail_to_storage(thumbnail_id, thumbnail_data));
                }

                // *** Remove stale sessions ***
                let sessions: Vec<_> = udp_server
                    .session_registry
                    .get_all_sessions()
                    .iter()
                    .map(|&session| (session.id.clone(), session.ttl))
                    .collect();

                for (id, ttl) in sessions {
                    if ttl.elapsed() > Duration::from_secs(5) {
                        udp_server.session_registry.remove_session(id);
                    }
                }
            }
        }
    }
}

fn start_timeout_interval(sender: Sender<ServerCommand>) {
    loop {
        sleep(Duration::from_secs(3));
        sender
            .send(ServerCommand::RunPeriodicChecks)
            .expect("Server channel should be open");
    }
}

fn start_udp_server(socket: UdpSocket, sender: Sender<ServerCommand>) {
    loop {
        sleep(Duration::from_millis(1));

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

fn build_udp_socket() -> UdpSocket {
    let global_config = get_global_config();
    let socket = UdpSocket::bind(global_config.udp_server_config.address).unwrap();
    println!(
        "Running UDP server at {}",
        global_config.udp_server_config.address
    );
    socket
}
