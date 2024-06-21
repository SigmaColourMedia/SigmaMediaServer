use std::{fmt::Write as _, sync::Arc};
use std::fs::read;

use openssl::hash::MessageDigest;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslVerifyMode};
use openssl::x509::X509;

use crate::{CERT_KEY_PATH, CERT_PATH};

pub struct SSLConfig {
    pub acceptor: Arc<SslAcceptor>,
    pub fingerprint: String,
}

impl SSLConfig {
    pub fn new() -> SSLConfig {
        let mut acceptor_builder = SslAcceptor::mozilla_intermediate(SslMethod::dtls()).unwrap();
        acceptor_builder
            .set_private_key_file(CERT_KEY_PATH, SslFiletype::PEM)
            .expect("Missing private key file");
        acceptor_builder
            .set_certificate_chain_file(CERT_PATH)
            .expect("Missing cert file");
        acceptor_builder.set_verify(SslVerifyMode::NONE);
        acceptor_builder
            .set_tlsext_use_srtp("SRTP_AES128_CM_SHA1_80")
            .expect("Failed enabling DTLS extension");

        let acceptor = Arc::new(acceptor_builder.build());

        let cert_file = read(CERT_PATH).expect("Failed to read cert file");

        let x509 = X509::from_pem(&cert_file).unwrap();
        let x509_digest = x509.digest(MessageDigest::sha256()).unwrap();

        let mut fingerprint = String::new();
        for i in 0..x509_digest.len() {
            write!(fingerprint, "{:02X}", x509_digest[i]).unwrap();
            if i != x509_digest.len() - 1 {
                write!(fingerprint, ":").unwrap();
            }
        }

        SSLConfig {
            acceptor,
            fingerprint,
        }
    }
}
