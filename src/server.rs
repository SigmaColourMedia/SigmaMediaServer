use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use openssl::ssl::SslAcceptor;

use crate::acceptor::SSLConfig;

enum DTLSState {
    Pending,
    Connected,
}

struct Client {
    acceptor: Arc<SslAcceptor>,
    address: SocketAddr,
    dtls_state: DTLSState,
}

impl Client {
    pub fn new(address: SocketAddr, acceptor: Arc<SslAcceptor>) -> Self {
        Client { address, dtls_state: DTLSState::Pending, acceptor }
    }
}

struct Server {
    config: SSLConfig,
    clients: HashMap<SocketAddr, Client>,
}

impl Server {
    pub fn new(config: SSLConfig) -> Self {
        Server {
            config,
            clients: HashMap::new(),
        }
    }
    pub fn create_client(&mut self, addr: SocketAddr) {
        self.clients.insert(addr, Client::new(addr, self.config.acceptor.clone()));
    }

    pub fn get_client(&mut self, addr: &SocketAddr) -> Option<&Client> {
        self.clients.get(addr)
    }
}