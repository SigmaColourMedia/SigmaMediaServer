use crate::http::parsers::parse_http;
use crate::http::response_builder::ResponseBuilder;
use crate::http::server_builder::{RouteHandlers, ServerContext};
use crate::http::Request;
use crate::HOST_ADDRESS;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub struct HttpServer {
    route_handlers: RouteHandlers,
    context: Arc<ServerContext>,
    tcp_listener: TcpListener,
}

impl HttpServer {
    pub async fn new(context: Arc<ServerContext>, route_handlers: RouteHandlers) -> Self {
        let listener = TcpListener::bind(format!("{HOST_ADDRESS}:8080"))
            .await
            .unwrap();
        println!("Running TCP server at {}:8080", HOST_ADDRESS);

        HttpServer {
            context,
            route_handlers,
            tcp_listener: listener,
        }
    }

    pub async fn read_stream(&self) -> std::io::Result<TcpStream> {
        self.tcp_listener.accept().await.map(|incoming| incoming.0)
    }

    async fn handle_request(&self, request: Request, mut stream: TcpStream) {
        if let Some(handler) = self.route_handlers.get(&request.path) {
            let response = handler(request, self.context.clone()).await;
            if let Err(err) = stream.write_all(response.as_bytes()).await {
                println!("Error writing to stream {}", err)
            }
        } else {
            let response = ResponseBuilder::new().set_status(404).build();
            if let Err(err) = stream.write_all(response.as_bytes()).await {
                println!("Error writing to stream {}", err)
            }
        }
    }

    pub async fn handle_stream(&self, mut stream: TcpStream) {
        let mut buffer = [0u8; 3000];
        stream
            .read(&mut buffer)
            .await
            .expect("Failed reading from buffer");
        if let Some(request) = parse_http(&buffer).await {
            self.handle_request(request, stream).await;
        }
    }
}
