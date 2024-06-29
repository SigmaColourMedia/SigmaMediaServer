use std::collections::HashMap;

use crate::http::{Request, Response};
use crate::http_server::HttpServer;

type Callback = dyn (Fn(Request) -> Response) + Send + Sync;

pub struct ServerBuilder {
    route_handlers: RouteHandlers,
}

pub type RouteHandlers = HashMap<String, Box<Callback>>;

impl ServerBuilder {
    pub fn new() -> Self {
        ServerBuilder {
            route_handlers: HashMap::new(),
        }
    }

    pub fn add_handler<F>(&mut self, route: &str, handler: F)
    where
        F: Fn(Request) -> Response,
        F: Send + Sync + 'static,
    {
        self.route_handlers
            .insert(route.to_string(), Box::new(handler));
    }

    pub fn build(self) -> HttpServer {
        HttpServer::new(self.route_handlers)
    }
}
