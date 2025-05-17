use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{body::Incoming as IncomingBody, Method, Request, Response};
use hyper::server::conn::http1;
use hyper::service::{Service, service_fn};
use hyper_util::rt::TokioIo;
use log::{debug, error, info};
use tokio::net::TcpListener;

use sdp::SDPResolver;

use crate::api::HTTPError;
use crate::api::routes::error::error_route;
use crate::api::routes::thumbnail::thumbnail_get;
use crate::api::routes::whep::{whep_options, whep_post};
use crate::api::routes::whip::whip_post;
use crate::config::get_global_config;

pub async fn start_http_server(sdp_resolver: Arc<SDPResolver>) {
    let listener = TcpListener::bind(get_global_config().tcp_server_config.address)
        .await
        .unwrap();
    info!(target: "HTTP", "Listening on {}", get_global_config().tcp_server_config.address);

    loop {
        let sdp_resolver = sdp_resolver.clone();

        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);

        let service = service_fn(move |req| {
            let sdp_resolver = sdp_resolver.clone();

            async move {
                match (req.method(), req.uri().path()) {
                    (&Method::GET, "/thumbnail") => thumbnail_get(req).await,
                    (&Method::POST, "/whip") => whip_post(req, sdp_resolver).await,
                    (&Method::POST, "/whep") => whep_post(req).await,
                    (&Method::OPTIONS, "/whep") => whep_options().await,
                    _ => error_route(HTTPError::NotFound).await,
                }
            }
        });

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                error!(target: "HTTP","Failed to serve connection: {:?}", err);
            }
        });
    }
}
