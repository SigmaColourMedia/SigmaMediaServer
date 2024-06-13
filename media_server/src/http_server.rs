use crate::http::parsers::parse_http;
use crate::http::router::{Router, RouterBuilder};
use crate::http::routes::rooms::{rooms, rooms_factory};
use crate::http::routes::whip::whip_factory;
use crate::http::SessionCommand;
use crate::HOST_ADDRESS;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;

pub struct HttpServer {
    router: Arc<Router>,
    tcp_listener: TcpListener,
}

impl HttpServer {
    pub async fn new(fingerprint: String, sender: Sender<SessionCommand>) -> Self {
        let mut router_builder = RouterBuilder::new();

        let whip_handler = whip_factory(fingerprint.clone(), sender.clone());
        let rooms_handler = rooms_factory(sender.clone());
        router_builder.add_handler("/whip", whip_handler);
        router_builder.add_handler("/rooms", rooms_handler);

        router_builder.add_fingerprint(fingerprint);
        router_builder.add_sender(sender);
        let router = router_builder.build();
        let listener = TcpListener::bind(format!("{HOST_ADDRESS}:8080"))
            .await
            .unwrap();
        println!("Running TCP server at {}:8080", HOST_ADDRESS);

        HttpServer {
            router: Arc::new(router),
            tcp_listener: listener,
        }
    }

    pub async fn read_stream(&self) -> std::io::Result<TcpStream> {
        self.tcp_listener.accept().await.map(|incoming| incoming.0)
    }

    pub async fn handle_stream(&self, mut stream: TcpStream) {
        let mut buffer = [0u8; 3000];
        stream
            .read(&mut buffer)
            .await
            .expect("Failed reading from buffer");
        if let Some(request) = parse_http(&buffer).await {
            self.router.handle_request(request, &mut stream).await;
        }
    }
}
