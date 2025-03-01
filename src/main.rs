use std::net::UdpSocket;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::sleep;
use std::time::{Duration, Instant};
use rtcp::Marshall;
use crate::client::{Client, ClientSslState};

use crate::config::get_global_config;
use crate::http::server::{Notification, Room, start_http_server};
use crate::http::ServerCommand;
use crate::ice_registry::{ConnectionType, Session, Streamer};
use crate::server::UDPServer;
use crate::thumbnail::save_thumbnail_to_storage;

mod acceptor;
mod client;
mod config;
mod http;
mod ice_registry;
mod server;
mod stun;
mod thumbnail;
mod rtp_replay_buffer;
mod rtcp_reporter;
mod rtp_reporter;
mod media_header;

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
        move || run_periodic_checks(sender)
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

                            if !should_update_thumbnail {
                                return None;
                            }

                            streamer.thumbnail_extractor.last_picture.take().map(|last_picture| {
                                streamer.image_timestamp.replace(Instant::now());
                                (streamer.owned_room_id, last_picture)
                            })
                        }
                    })
                    .collect::<Vec<_>>();

                for (thumbnail_id, thumbnail_data) in thumbnails_to_update {
                    thread::spawn(move || save_thumbnail_to_storage(thumbnail_id, thumbnail_data));
                }

                // *** Remove stale sessions ***
                let stale_session_ids: Vec<_> = udp_server
                    .session_registry
                    .get_all_sessions()
                    .iter()
                    .filter(|&session| session.ttl.elapsed() > Duration::from_secs(5)).map(|&session| session.id)
                    .collect();

                for id in stale_session_ids {
                    udp_server.session_registry.remove_session(id);
                }

                // *** Schedule Receiver Reports ***
                let sessions_scheduled_for_receiver_report = udp_server
                    .session_registry
                    .get_all_sessions_mut().into_iter()
                    .filter_map(|session| {
                        let reporter = session.video_reporter.as_mut()?;

                        match &session.connection_type {
                            ConnectionType::Streamer(_) => {
                                let client = session.client.as_mut()?;
                                match &mut client.ssl_state {
                                    ClientSslState::Established(ssl_stream) => {
                                        let last_timestamp = reporter.last_report_timestamp.get_or_insert(Instant::now());
                                        let is_early_feedback = !reporter.missing_packets.is_empty() && last_timestamp.elapsed() >= Duration::from_millis(1);
                                        let is_regular_feedback = last_timestamp.elapsed() >= Duration::from_secs(1);

                                        if is_early_feedback || is_regular_feedback { Some((reporter, client.remote_address, ssl_stream)) } else { None }
                                    }
                                    _ => None
                                }
                            }
                            _ => None,
                        }
                    }).collect::<Vec<_>>();

                for (reporter, remote_address, ssl_stream) in sessions_scheduled_for_receiver_report {
                    let mut receiver_report = reporter.generate_receiver_report().to_vec();
                    reporter.last_report_timestamp.replace(Instant::now());
                    if let Err(err) = ssl_stream.srtp_outbound.protect_rtcp(&mut receiver_report).or(Err(UDPServerError::RTCPProtectError)).and_then(|_| udp_server.socket.send_to(&receiver_report, remote_address).or(Err(UDPServerError::SocketWriteError))) {
                        eprintln!("Error sending RTCP report {:?}", err)
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum UDPServerError {
    RTCPProtectError,
    RTPProtectError,
    SocketWriteError,
    RTCPUnprotectError,
    RTCPDecodeError,
}

fn run_periodic_checks(sender: Sender<ServerCommand>) {
    loop {
        sleep(Duration::from_micros(150));
        sender
            .send(ServerCommand::RunPeriodicChecks)
            .expect("Server channel should be open");
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

fn build_udp_socket() -> UdpSocket {
    let global_config = get_global_config();
    let socket = UdpSocket::bind(global_config.udp_server_config.address).unwrap();
    println!(
        "Running UDP server at {}",
        global_config.udp_server_config.address
    );
    socket
}
