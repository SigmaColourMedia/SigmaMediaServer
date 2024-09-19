use std::{fmt::Write, sync::Arc};
use std::fs::read;
use std::path::PathBuf;

use openssl::hash::MessageDigest;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslVerifyMode};
use openssl::x509::X509;

pub struct SSLConfig {
    pub acceptor: Arc<SslAcceptor>,
    pub fingerprint: String,
}

impl SSLConfig {
    pub fn new(cert_dir: PathBuf) -> SSLConfig {
        let cert_path = cert_dir.join("cert.pem");
        let cert_key_path = cert_dir.join("key.pem");
        let mut acceptor_builder = SslAcceptor::mozilla_intermediate(SslMethod::dtls()).unwrap();
        acceptor_builder
            .set_private_key_file(cert_key_path, SslFiletype::PEM)
            .expect("Missing private key file");
        acceptor_builder
            .set_certificate_chain_file(cert_path.as_path())
            .expect("Missing cert file");
        acceptor_builder.set_verify(SslVerifyMode::NONE);
        acceptor_builder
            .set_tlsext_use_srtp("SRTP_AES128_CM_SHA1_80")
            .expect("Failed enabling DTLS extension");

        let acceptor = Arc::new(acceptor_builder.build());

        let cert_file = read(cert_path).expect("Failed to read cert file");

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
