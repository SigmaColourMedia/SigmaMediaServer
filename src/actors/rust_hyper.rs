use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{body::Incoming as IncomingBody, Request, Response};
use hyper::server::conn::http1;
use hyper::service::{Service, service_fn};
use hyper_util::rt::TokioIo;
use log::{debug, error, info};
use tokio::net::TcpListener;

use sdp::SDPResolver;

use crate::actors::{get_event_bus, MessageEvent};
use crate::config::get_global_config;

#[derive(Clone)]
struct WHIPService {
    sdp_resolver: SDPResolver,
}

pub async fn start_http_server() {
    let listener = TcpListener::bind(get_global_config().tcp_server_config.address)
        .await
        .unwrap();
    info!(target: "HTTP", "Listening on {}", get_global_config().tcp_server_config.address);

    let sdp_resolver = Arc::new(SDPResolver::new(
        format!("sha-256 {}", get_global_config().ssl_config.fingerprint).as_str(),
        get_global_config().udp_server_config.address,
    ));

    loop {
        let sdp_resolver = sdp_resolver.clone();

        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);

        let service = service_fn(move |req| {
            let sdp_resolver = sdp_resolver.clone();

            async move {
                match req.uri().path() {
                    "/whip" => whip_route(req, sdp_resolver).await,
                    "/debug/session" => session_debug_route(req).await,
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

type RouteResult = Result<Response<Full<Bytes>>, hyper::Error>;

type HTTPResponse = Response<Full<Bytes>>;

async fn error_route(http_error: HTTPError) -> RouteResult {
    match http_error {
        HTTPError::NotFound => Ok(Response::builder()
            .status(404)
            .body(Full::new(Bytes::from("404 Not Found")))
            .unwrap()),
        HTTPError::BadRequest => Ok(Response::builder()
            .status(400)
            .body(Full::new(Bytes::from("400 Bad Request")))
            .unwrap()),
        HTTPError::NotAuthorized => Ok(Response::builder()
            .status(403)
            .body(Full::new(Bytes::from("403 Not Authorized")))
            .unwrap()),
        HTTPError::ServerError => Ok(Response::builder()
            .status(500)
            .body(Full::new(Bytes::from("500 Server Error")))
            .unwrap()),
    }
}

async fn whip_route(req: Request<IncomingBody>, sdp_resolver: Arc<SDPResolver>) -> RouteResult {
    let res = whip_resolver(req, sdp_resolver).await;
    match res {
        Ok(res) => Ok(res),
        Err(err) => error_route(err).await,
    }
}

async fn session_debug_route(req: Request<IncomingBody>) -> RouteResult{
    let res = session_debug_resolver(req).await;
    match res {
        Ok(res) => Ok(res),
        Err(err) => error_route(err).await,
    }
}

async fn session_debug_resolver(req: Request<IncomingBody>) -> Result<HTTPResponse, HTTPError>{
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    
    get_event_bus().send(MessageEvent::DebugSession(tx)).unwrap();
    
    let res = rx.await.unwrap();
    
    Ok(Response::builder().body(Full::new(Bytes::from(res))).unwrap())
}

async fn whip_resolver(
    req: Request<IncomingBody>,
    sdp_resolver: Arc<SDPResolver>,
) -> Result<HTTPResponse, HTTPError> {
    let bearer_token = req.headers().get("Authorization");
    let is_authorized = bearer_token
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.split_once("Bearer "))
        .is_some_and(|(_, key)| key == get_global_config().tcp_server_config.whip_token);

    if !is_authorized {
        return Err(HTTPError::NotAuthorized);
    }

    let negotiated_session = Limited::new(req.into_body(), 15000)
        .collect()
        .await
        .or(Err(HTTPError::BadRequest))
        .map(|body| body.to_bytes())
        .and_then(|data| String::from_utf8(data.to_vec()).or(Err(HTTPError::BadRequest)))
        .and_then(|data| {
            sdp_resolver
                .accept_stream_offer(&data)
                .or(Err(HTTPError::BadRequest))
        })?;

    let sdp = negotiated_session.sdp_answer.clone();

    get_event_bus()
        .send(MessageEvent::InitStreamer(negotiated_session))
        .unwrap();

    Ok(Response::builder()
        .status(201)
        .header("content-type", "application/sdp")
        .header(
            "location",
            format!("{}/whip", get_global_config().tcp_server_config.address),
        )
        .body(Full::new(Bytes::from(String::from(sdp))))
        .unwrap())
}

enum HTTPError {
    NotFound,
    BadRequest,
    NotAuthorized,
    ServerError,
}
