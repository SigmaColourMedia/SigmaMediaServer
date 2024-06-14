use crate::http::response_builder::ResponseBuilder;
use crate::http::{Request, SessionCommand};
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;

type CallbackFuture<O> = Pin<Box<dyn Future<Output = O> + Send>>;
pub type Callback<T> = dyn (Fn(T) -> CallbackFuture<String>) + Send + Sync;
pub struct RouterBuilder {
    fingerprint: Option<String>,
    sender: Option<Sender<SessionCommand>>,
    route_handlers: HashMap<String, Box<Callback<Request>>>,
}

impl RouterBuilder {
    pub fn new() -> Self {
        RouterBuilder {
            sender: None,
            fingerprint: None,
            route_handlers: HashMap::new(),
        }
    }

    pub fn add_handler<F>(&mut self, route: &str, handler: F)
    where
        F: Fn(Request) -> CallbackFuture<String>,
        F: Send + Sync + 'static,
    {
        self.route_handlers
            .insert(route.to_string(), Box::new(handler));
    }
    pub fn add_sender(&mut self, sender: Sender<SessionCommand>) {
        self.sender = Some(sender)
    }
    pub fn add_fingerprint(&mut self, fingerprint: String) {
        self.fingerprint = Some(fingerprint)
    }

    pub fn build(self) -> Router {
        Router {
            sender: self.sender.expect("Command Sender was not provided"),
            fingerprint: self.fingerprint.expect("Fingerprint was not provided"),
            route_handlers: self.route_handlers,
        }
    }
}

pub struct Router {
    fingerprint: String,
    sender: Sender<SessionCommand>,
    route_handlers: HashMap<String, Box<Callback<Request>>>,
}

impl Router {
    pub async fn handle_request(&self, request: Request, stream: &mut TcpStream) {
        if let Some(handler) = self.route_handlers.get(&request.path) {
            let response = handler(request).await;
            println!("{}", response);
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
}
