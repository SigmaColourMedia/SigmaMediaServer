use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::mpsc::Sender;

use crate::http::{Request, Response, SessionCommand};
use crate::http_server::HttpServer;

type CallbackFuture<O> = Pin<Box<dyn Future<Output = O> + Send>>;

type Callback = dyn (Fn(Request, Arc<ServerContext>) -> CallbackFuture<Response>) + Send + Sync;

pub type Context = Arc<ServerContext>;
pub struct ServerContext {
    pub fingerprint: String,
    pub sender: Sender<SessionCommand>,
}
pub struct ServerBuilder {
    fingerprint: Option<String>,
    sender: Option<Sender<SessionCommand>>,
    route_handlers: RouteHandlers,
}

pub type RouteHandlers = HashMap<String, Box<Callback>>;

impl ServerBuilder {
    pub fn new() -> Self {
        ServerBuilder {
            sender: None,
            fingerprint: None,
            route_handlers: HashMap::new(),
        }
    }

    pub fn add_handler<F>(&mut self, route: &str, handler: F)
    where
        F: Fn(Request, Context) -> CallbackFuture<Response>,
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

    pub async fn build(self) -> HttpServer {
        let ctx = ServerContext {
            sender: self.sender.expect("No sender provided to builder"),
            fingerprint: self
                .fingerprint
                .expect("No fingerprint provided to builder"),
        };
        HttpServer::new(Arc::new(ctx), self.route_handlers).await
    }
}
