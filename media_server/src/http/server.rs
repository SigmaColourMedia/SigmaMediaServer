use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};

use serde::{Deserialize, Serialize};
use threadpool::ThreadPool;

use crate::config::get_global_config;
use crate::http::{HttpError, HTTPMethod, Request, Response, ServerCommand};
use crate::http::parsers::{map_http_err_to_response, parse_http};
use crate::http::response_builder::ResponseBuilder;

struct HttpServer {
    tcp_listener: TcpListener,
    server_command_sender: Sender<ServerCommand>,
    notification_bus_receiver: crossbeam_channel::Receiver<Notification>,
    pool: ThreadPool,
}

pub fn start_http_server(
    sender: Sender<ServerCommand>,
    receiver: crossbeam_channel::Receiver<Notification>,
) {
    let pool = ThreadPool::new(60);
    let listener = TcpListener::bind(get_global_config().tcp_server_config.address).unwrap();
    for mut stream in listener.incoming() {
        let sender = sender.clone();
        let receiver = receiver.clone();
        pool.execute(move || {
            let mut stream = stream.unwrap();
            if let Some(request) = parse_http(&mut stream) {
                match request.path.as_str() {
                    "/whip" => {
                        let response = whip_route(request, sender.clone())
                            .unwrap_or_else(map_http_err_to_response);
                        println!(
                            "response is {}",
                            String::from_utf8_lossy(response.as_bytes())
                        );
                        stream.write_all(response.as_bytes()).unwrap()
                    }
                    "/whep" => {
                        let response = match &request.method {
                            HTTPMethod::POST => whep_route(request, sender.clone())
                                .unwrap_or_else(map_http_err_to_response),
                            HTTPMethod::OPTIONS => options_route(),
                            _ => map_http_err_to_response(HttpError::MethodNotAllowed),
                        };
                        stream.write_all(response.as_bytes()).unwrap()
                    }
                    "/images" => {
                        let response =
                            images_route(request).unwrap_or_else(map_http_err_to_response);
                        stream.write_all(response.as_bytes());
                    }
                    "/notifications" => {
                        sender
                            .clone()
                            .send(ServerCommand::SendRoomsStatus)
                            .expect("ServerCommand channel should remain open");
                        let rooms = receiver.recv().unwrap();

                        let message =
                            format!("data: {}\r\n\r\n", serde_json::to_string(&rooms).unwrap());
                        let response = ResponseBuilder::new()
                            .set_status(200)
                            .set_header("Connection", "keep-alive")
                            .set_header("Cache-control", "no-cache")
                            .set_header("content-type", "text/event-stream")
                            .set_body(message.as_bytes())
                            .build();
                        if let Err(_) = stream.write_all(response.as_bytes()) {
                            return; // broken pipe
                        }

                        loop {
                            if let Ok(notification) = receiver.recv() {
                                let message = format!(
                                    "data: {}\r\n\r\n",
                                    serde_json::to_string(&notification).unwrap()
                                );
                                if let Err(_) = stream
                                    .write_all(message.as_bytes())
                                    .and_then(|_| stream.flush())
                                {
                                    return; // broken pipe
                                }
                            }
                        }
                    }
                    _ => {
                        let response = map_http_err_to_response(HttpError::NotFound);
                        stream.write_all(response.as_bytes());
                    }
                }
            }
        })
    }
}

fn whip_route(
    request: Request,
    command_sender: Sender<ServerCommand>,
) -> Result<Response, HttpError> {
    let config = get_global_config();

    let bearer_token = request
        .headers
        .get("authorization")
        .ok_or(HttpError::Unauthorized)?;

    if !bearer_token.eq(&format!("Bearer {}", config.tcp_server_config.whip_token)) {
        return Err(HttpError::Unauthorized);
    }

    let sdp_offer = request
        .body
        .and_then(|body| String::from_utf8(body).ok())
        .ok_or(HttpError::BadRequest)?;

    let (tx, rx) = channel::<Option<String>>();

    command_sender
        .send(ServerCommand::AddStreamer(sdp_offer, tx))
        .expect("SessionCommand channel should remain open");

    let sdp_answer = rx
        .recv()
        .expect("SessionCommand channel should remain open")
        .ok_or(HttpError::NotFound)?;

    Ok(ResponseBuilder::new()
        .set_status(201)
        .set_header("content-type", "application/sdp")
        .set_header("location", "http://localhost:8080/whip")
        .set_body(sdp_answer.as_bytes())
        .build())
}

fn options_route() -> Response {
    ResponseBuilder::new()
        .set_status(204)
        .set_header("Access-Control-Allow-Method", "POST")
        .set_header("Access-Control-Allow-Headers", "content-type")
        .build()
}

fn whep_route(
    request: Request,
    command_sender: Sender<ServerCommand>,
) -> Result<Response, HttpError> {
    let target_id = request
        .search
        .get("target_id")
        .ok_or(HttpError::BadRequest)?
        .to_string()
        .parse::<u32>()
        .map_err(|_| HttpError::BadRequest)?;

    let (tx, rx) = channel::<Option<String>>();

    let body = request
        .body
        .and_then(|body| String::from_utf8(body).ok())
        .ok_or(HttpError::BadRequest)?;

    command_sender
        .send(ServerCommand::AddViewer(body, target_id, tx))
        .expect("Session Command channel should remain open");

    // todo Handle unsupported codecs
    let sdp_answer = rx.recv().unwrap().ok_or(HttpError::BadRequest)?;

    let cors_origin = &get_global_config().frontend_url;

    let response_builder = ResponseBuilder::new();
    let response = response_builder
        .set_status(200)
        .set_header("content-type", "application/sdp")
        .set_header("Access-Control-Allow-Method", "POST")
        .set_header("Access-Control-Allow-Origin", cors_origin)
        .set_header("location", "http://localhost:8080/whep")
        .set_body(sdp_answer.as_bytes())
        .build();

    Ok(response)
}

fn images_route(request: Request) -> Result<Response, HttpError> {
    let file_name = request
        .search
        .get("image")
        .ok_or(HttpError::BadRequest)?
        .as_str();

    let parsed_name = Path::new(file_name)
        .file_name()
        .ok_or(HttpError::BadRequest)?;
    let mut file_pathname = PathBuf::from("../temp");
    file_pathname.push(parsed_name);
    let target_file = fs::read(file_pathname).map_err(|_| HttpError::NotFound)?;

    Ok(ResponseBuilder::new()
        .set_status(200)
        .set_header("Content-Type", "image/webp")
        .add_body(target_file)
        .build())
}

#[derive(Serialize, Deserialize)]
pub struct Notification {
    pub rooms: Vec<Room>,
}

#[derive(Serialize, Deserialize)]
pub struct Room {
    pub viewer_count: usize,
    pub id: u32,
}
