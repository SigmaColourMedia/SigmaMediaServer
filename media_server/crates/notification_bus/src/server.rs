use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::thread;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Notification {
    pub rooms: Vec<Room>,
}

#[derive(Serialize, Deserialize)]
pub struct Room {
    pub viewer_count: usize,
    pub id: u32,
}

pub struct NotificationBusBuilder {
    tcp_listener: Option<TcpListener>,
    cors_origin: Option<String>,
}

impl NotificationBusBuilder {
    pub fn new() -> Self {
        Self {
            cors_origin: None,
            tcp_listener: None,
        }
    }
    pub fn add_address<A: ToSocketAddrs>(mut self, address: A) -> Self {
        let tcp_listener = TcpListener::bind(address).expect("Socket address should be available");
        self.tcp_listener = Some(tcp_listener);
        self
    }

    pub fn add_cors_origin(mut self, origin: String) -> Self {
        self.cors_origin = Some(origin);
        self
    }

    pub fn build(self) -> NotificationBus {
        NotificationBus {
            notification_channel: crossbeam_channel::unbounded(),
            cors_origin: self.cors_origin.expect("CORS origin should be defined"),
            tcp_listener: self.tcp_listener.expect("TCP listener should be defined"),
        }
    }
}

pub struct NotificationBus {
    tcp_listener: TcpListener,
    cors_origin: String,
    notification_channel: (
        crossbeam_channel::Sender<Notification>,
        crossbeam_channel::Receiver<Notification>,
    ),
}

impl NotificationBus {
    pub fn get_sender(&self) -> crossbeam_channel::Sender<Notification> {
        self.notification_channel.0.clone()
    }
    pub fn startup(&self) {
        let origin = Arc::new(self.cors_origin.clone());

        for incoming in self.tcp_listener.incoming() {
            let receiver = self.notification_channel.1.clone();
            let origin = origin.clone();
            thread::spawn(move || {
                if let Ok(mut tcp_stream) = incoming {
                    if let Some(request) = read_request(&mut tcp_stream) {
                        match request.pathname.as_str() {
                            "/" => {
                                let response = format!(
                                    "HTTP/1.1 200 OK\r\n\
                                Connection: keep-alive\r\n\
                                Cache-Control: no-cache\r\n\
                                Access-Control-Allow-Origin: {origin}\r\n\
                                Access-Control-Allow-Method: GET\r\n\
                                Content-Type: text/event-stream\r\n\r\n"
                                );

                                if let Err(_) = tcp_stream
                                    .write_all(response.as_bytes())
                                    .and_then(|_| tcp_stream.flush())
                                {
                                    return;
                                }

                                loop {
                                    if let Ok(notification) = receiver.recv() {
                                        let message = format!(
                                            "data: {}\r\n\r\n",
                                            serde_json::to_string(&notification).unwrap()
                                        );
                                        if let Err(_) = tcp_stream
                                            .write_all(message.as_bytes())
                                            .and_then(|_| tcp_stream.flush())
                                        {
                                            break;
                                        }
                                    }
                                }
                            }
                            _ => {
                                let response = format!(
                                    "HTTP/1.1 404 NOT FOUND\r\n\
                                Connection: keep-alive\r\n\
                                Cache-Control: no-cache\r\n\
                                Access-Control-Allow-Origin: {origin}\r\n\
                                Access-Control-Allow-Method: GET\r\n\r\n"
                                );

                                if let Err(_) = tcp_stream
                                    .write_all(response.as_bytes())
                                    .and_then(|_| tcp_stream.flush())
                                {
                                    return;
                                }
                            }
                        }
                    }
                }
            });
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut heading = String::new();
    reader.read_line(&mut heading).ok()?;
    let mut heading_split = heading.split(" ");
    let method = heading_split.next()?;

    if !method.eq_ignore_ascii_case("GET") {
        return None;
    }

    let pathname = heading_split.next()?.to_string();
    let mut headers = HashMap::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;

        if header.trim().is_empty() {
            break;
        }

        let (key, value) = header.split_once(":")?;
        headers.insert(key.to_string(), value.to_string());
    }

    Some(Request { headers, pathname })
}

struct Request {
    pathname: String,
    headers: HashMap<String, String>,
}

mod tests {
    #[test]
    fn it_should_pass() {}
}
