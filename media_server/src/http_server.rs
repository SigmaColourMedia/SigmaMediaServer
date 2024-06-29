use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use crate::config::get_global_config;
use crate::http::parsers::parse_http;
use crate::http::Request;
use crate::http::response_builder::ResponseBuilder;
use crate::http::server_builder::RouteHandlers;

pub struct HttpServer {
    route_handlers: RouteHandlers,
    tcp_listener: TcpListener,
}

impl HttpServer {
    pub fn new(route_handlers: RouteHandlers) -> Self {
        let address = get_global_config().tcp_server_config.address;
        let listener = TcpListener::bind(address).unwrap();
        println!("Running TCP server at {}", address);

        HttpServer {
            route_handlers,
            tcp_listener: listener,
        }
    }

    pub fn read_stream(&self) -> std::io::Result<TcpStream> {
        self.tcp_listener.accept().map(|incoming| incoming.0)
    }

    fn handle_request(&self, request: Request, mut stream: TcpStream) {
        if let Some(handler) = self.route_handlers.get(&request.path) {
            let response = handler(request);
            if let Err(err) = stream.write_all(response.as_bytes()) {
                println!("Error writing to stream {}", err)
            }
        } else {
            let response = ResponseBuilder::new().set_status(404).build();
            if let Err(err) = stream.write_all(response.as_bytes()) {
                println!("Error writing to stream {}", err)
            }
        }
    }

    pub fn handle_stream(&self, mut stream: TcpStream) {
        let mut buffer = [0u8; 3000];
        stream
            .read(&mut buffer)
            .expect("Failed reading from buffer");
        if let Some(request) = parse_http(&buffer) {
            self.handle_request(request, stream);
        }
    }
}
