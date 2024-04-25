use std::sync::Arc;

use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslVerifyMode};

pub struct SSLConfig {
    pub acceptor: Arc<SslAcceptor>,
}

impl SSLConfig {
    pub fn new() -> SSLConfig {
        let mut acceptor_builder = SslAcceptor::mozilla_intermediate(SslMethod::dtls()).unwrap();
        acceptor_builder.set_private_key_file("key.pem", SslFiletype::PEM).unwrap();
        acceptor_builder.set_certificate_chain_file("cert.pem").unwrap();
        acceptor_builder.set_verify(SslVerifyMode::NONE);
        let acceptor = Arc::new(acceptor_builder.build());
        SSLConfig {
            acceptor
        }
    }
}
