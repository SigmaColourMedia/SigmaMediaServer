use std::collections::HashMap;
use std::sync::mpsc::Sender;

use crate::http::{Request, Response, ServerCommand};
use crate::http_server::HttpServer;

type Callback = dyn (Fn(Request, Sender<ServerCommand>) -> Response) + Send + Sync;

pub struct ServerBuilder {
    route_handlers: RouteHandlers,
    command_sender: Option<Sender<ServerCommand>>,
}

pub type RouteHandlers = HashMap<String, Box<Callback>>;

impl ServerBuilder {
    pub fn new() -> Self {
        ServerBuilder {
            command_sender: None,
            route_handlers: HashMap::new(),
        }
    }

    pub fn add_handler<F>(&mut self, route: &str, handler: F)
    where
        F: Fn(Request, Sender<ServerCommand>) -> Response,
        F: Send + Sync + 'static,
    {
        self.route_handlers
            .insert(route.to_string(), Box::new(handler));
    }

    pub fn add_sender(&mut self, sender: Sender<ServerCommand>) {
        self.command_sender = Some(sender)
    }
    pub fn build(self) -> HttpServer {
        HttpServer::new(
            self.route_handlers,
            self.command_sender
                .expect("Command sender should be present"),
        )
    }
}
