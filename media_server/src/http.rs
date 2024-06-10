use openssl::ssl::{SslConnector, SslMethod};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr};
use std::ops::Add;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{channel, Sender};

use crate::ice_registry::{Session, SessionCredentials};
use crate::rnd::get_random_string;
use crate::sdp::{create_sdp_receive_answer, create_streaming_sdp_answer, parse_sdp, SDP};
use crate::{DISCORD_API_URL, WHIP_TOKEN};

pub struct HTTPServer {
    fingerprint: String,
    session_commands_sender: Sender<SessionCommand>,
}

impl HTTPServer {
    pub fn new(fingerprint: String, sender: Sender<SessionCommand>) -> Self {
        HTTPServer {
            fingerprint,
            session_commands_sender: sender,
        }
    }
    pub async fn handle_http_request(&self, mut stream: TcpStream) {
        match parse_http_request(&mut stream).await {
            Some(req) => {
                match &req.path[..] {
                    "/whip" => match &req.method {
                        HTTPMethod::POST => {
                            if let Err(err) = self.register_streamer(&mut stream, req).await {
                                HTTPServer::handle_http_error(err, &mut stream).await;
                            }
                        }
                        _ => {
                            HTTPServer::handle_http_error(HttpError::MethodNotAllowed, &mut stream)
                                .await;
                        }
                    },
                    "/whep" => match &req.method {
                        HTTPMethod::GET => {
                            if let Err(err) = self.register_viewer(&mut stream, &req).await {
                                HTTPServer::handle_http_error(err, &mut stream).await;
                            }
                        }
                        _ => {
                            HTTPServer::handle_http_error(HttpError::MethodNotAllowed, &mut stream)
                                .await;
                        }
                    },
                    "/rooms" => match &req.method {
                        HTTPMethod::GET => {
                            if let Err(err) = self.get_rooms(&mut stream, &req).await {
                                HTTPServer::handle_http_error(err, &mut stream).await;
                            }
                        }
                        _ => {
                            HTTPServer::handle_http_error(HttpError::MethodNotAllowed, &mut stream)
                                .await;
                        }
                    },
                    _ => {
                        HTTPServer::handle_http_error(HttpError::NotFound, &mut stream).await;
                    }
                };
            }
            None => {
                HTTPServer::handle_http_error(HttpError::BadRequest, &mut stream).await;
            }
        }
    }

    async fn handle_http_error(http_error: HttpError, stream: &mut TcpStream) {
        match http_error {
            HttpError::NotFound => {
                if let Err(err) = write_404_response(stream).await {
                    eprint!("Error writing a HTTP response {}", err)
                }
            }
            HttpError::InternalServerError => {
                if let Err(err) = write_500_response(stream).await {
                    eprint!("Error writing a HTTP response {}", err)
                }
            }
            HttpError::BadRequest => {
                if let Err(err) = write_400_response(stream).await {
                    eprint!("Error writing a HTTP response {}", err)
                }
            }
            HttpError::MethodNotAllowed => {
                if let Err(err) = write_405_response(stream).await {
                    eprint!("Error writing a HTTP response {}", err)
                }
            }
            HttpError::Unauthorized => {
                if let Err(err) = write_401_response(stream).await {
                    eprint!("Error writing a HTTP response {}", err)
                }
            }
        }
    }

    async fn register_streamer(
        &self,
        stream: &mut TcpStream,
        request: Request,
    ) -> Result<(), HttpError> {
        println!("here");

        let bearer_token = request
            .headers
            .get("authorization")
            .ok_or(HttpError::Unauthorized)?;

        if !bearer_token.eq(&format!("Bearer {}", WHIP_TOKEN)) {
            return Err(HttpError::Unauthorized);
        }

        let sdp = request
            .body
            .and_then(parse_sdp)
            .ok_or(HttpError::BadRequest)?;
        let host_username = get_random_string(4);
        let host_password = get_random_string(24);
        let session_credentials = SessionCredentials {
            host_username,
            host_password,
        };

        let answer = create_sdp_receive_answer(&sdp, &session_credentials, &self.fingerprint);
        let session = Session::new_streamer(session_credentials, sdp);
        let session_id = session.id.to_string();

        let response = format!(
            "HTTP/1.1 201 CREATED\r\n\
        content-length:{content_length}\r\n\
        content-type:application/sdp\r\n\
        location: http://localhost:8080/whip?id={resource_id}\r\n\r\n\
        {answer}",
            content_length = answer.len(),
            resource_id = session_id
        );

        stream
            .write_all(response.as_bytes())
            .await
            .or(Err(HttpError::InternalServerError))?;

        self.session_commands_sender
            .send(SessionCommand::AddStreamer(session))
            .await
            .or(Err(HttpError::InternalServerError))?;

        notify_discord(session_id).await;

        Ok(())
    }
    async fn register_viewer(
        &self,
        stream: &mut TcpStream,
        request: &Request,
    ) -> Result<(), HttpError> {
        let target_id = request
            .search
            .get("target_id")
            .ok_or(HttpError::BadRequest)?;

        let (tx, rx) = tokio::sync::oneshot::channel::<Option<SDP>>();

        self.session_commands_sender
            .send(SessionCommand::GetStreamSDP((tx, target_id.clone())))
            .await
            .unwrap();

        let stream_sdp = rx.await.unwrap().ok_or(HttpError::NotFound)?;
        let (sdp_answer, credentials) = create_streaming_sdp_answer(&stream_sdp, &self.fingerprint)
            .ok_or(HttpError::BadRequest)?;

        let viewer_session = Session::new_viewer(target_id.to_owned(), credentials);

        let request_origin = request.headers.get("origin").ok_or(HttpError::BadRequest)?;

        let cors_allowed_origin = match request_origin.as_str() {
            "http://localhost:9000" => "http://localhost:9000",
            _ => "https://nynon.work",
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\n\
        content-type: application/sdp\r\n\
        content-length:{content_length}\r\n\
        Access-Control-Allow-Methods: GET\r\n\
        Access-Control-Allow-Origin: {CORS_ALLOWED_ORIGIN}\r\n\
        location:http://localhost:8080/whep?id={viewer_id}\r\n\r\n\
        {payload}",
            content_length = sdp_answer.len(),
            viewer_id = &viewer_session.id,
            payload = sdp_answer,
            CORS_ALLOWED_ORIGIN = cors_allowed_origin
        );

        self.session_commands_sender
            .send(SessionCommand::AddViewer(viewer_session))
            .await
            .unwrap();

        stream
            .write_all(response.as_bytes())
            .await
            .map_err(|_| HttpError::InternalServerError)?;

        Ok(())
    }

    async fn get_rooms(&self, stream: &mut TcpStream, request: &Request) -> Result<(), HttpError> {
        let (tx, mut rx) = channel::<Vec<String>>(1000);
        self.session_commands_sender
            .send(SessionCommand::GetRooms(tx))
            .await
            .unwrap();

        let rooms = rx.recv().await.unwrap();

        let rooms_string = rooms
            .into_iter()
            .map(|room_id| format!("\"{}\"", room_id))
            .collect::<Vec<String>>()
            .join(",");
        let body = format!("{{\"rooms\":[{}]}}", rooms_string);

        let request_origin = request.headers.get("origin").ok_or(HttpError::BadRequest)?;

        let cors_allowed_origin = match request_origin.as_str() {
            "http://localhost:9000" => "http://localhost:9000",
            _ => "https://nynon.work",
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\n\
            content-type: application/json\r\n\
            Access-Control-Allow-Methods: GET\r\n\
            Access-Control-Allow-Origin: {CORS_ALLOWED_ORIGIN}\r\n\
            content-length: {content_length}\r\n\r\n\
            {body}",
            content_length = body.len(),
            body = body,
            CORS_ALLOWED_ORIGIN = cors_allowed_origin
        );

        match stream.write_all(response.as_bytes()).await {
            Ok(_) => Ok(()),
            Err(_) => Err(HttpError::InternalServerError),
        }
    }
}

// todo please clean this up
async fn notify_discord(target_id: String) {
    let payload = format!(
        "{{\"content\": \"Nowy strumyczek pod https://nynon.work?watch={}\"}}",
        target_id
    );

    let connector = SslConnector::builder(SslMethod::tls()).unwrap().build();
    let stream = std::net::TcpStream::connect("discord.com:443").unwrap();
    let mut stream = connector.connect("discord.com", stream).unwrap();
    let request = format!(
        "POST {api_url} HTTP/1.1\r\n\
        content-type: application/json\r\n\
        Host: discord.com\r\n\
        content-length: {payload_len}\r\n\r\n\
        {payload}",
        payload_len = payload.len(),
        api_url = DISCORD_API_URL
    );

    stream.write_all(request.as_bytes()).unwrap();

    let mut buffer = [0u8; 2000];
    let bytes_read = stream.read(&mut buffer).unwrap();
    let res = String::from_utf8_lossy(&buffer[..bytes_read]);

    if !res.starts_with("HTTP/1.1 204") {
        println!("{res}");
        eprint!("Error sending discord webhook")
    }
}

async fn write_cors(stream: &mut TcpStream, remote: SocketAddr) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 205 NO CONTENT";
    println!("{}", remote.ip());

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

async fn write_404_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 404 NOT FOUND";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

async fn write_400_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 400 BAD REQUEST";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

async fn write_500_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 500 INTERNAL SERVER ERROR";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

async fn write_401_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 401 UNAUTHORIZED";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

async fn write_405_response(stream: &mut TcpStream) -> std::io::Result<()> {
    let status_line = "HTTP/1.1 405 METHOD NOT ALLOWED";

    let response = format! {"{status_line}\r\n\r\n"};
    stream.write_all(response.as_bytes()).await
}

async fn parse_http_request(stream: &mut TcpStream) -> Option<Request> {
    let mut buffer = [0u8; 2000];
    stream.read(&mut buffer).await.unwrap();

    let req = parse_http(&buffer).await;

    req
}

async fn parse_http(data: &[u8]) -> Option<Request> {
    let string_data = std::str::from_utf8(data).ok()?;
    let mut lines = string_data.lines();

    let mut request_line = lines.next()?.split(" ");

    let method = request_line.next()?;
    let pathname = request_line.next()?;
    let method = match method {
        "GET" => HTTPMethod::GET,
        "POST" => HTTPMethod::POST,
        "OPTIONS" => HTTPMethod::OPTIONS,
        "DELETE" => HTTPMethod::DELETE,
        _ => {
            return None;
        }
    };

    let pathname_split = pathname.split_once("?");
    let (path, search) = match &pathname_split {
        Some((path, search)) => (path.to_string(), parse_search(search)?),
        None => (pathname.to_string(), HashMap::new()),
    };

    let mut headers: HashMap<String, String> = HashMap::new();
    while let Some(line) = lines.next() {
        if line.is_empty() {
            // END OF HEADERS
            break;
        }
        let (key, value) = line.split_once(":")?;
        let key = key.trim().to_lowercase();
        let value = value.trim().to_string();
        headers.insert(key, value);
    }

    let content_length = headers.get("content-length");

    let body = content_length.and_then(|length| {
        let length = length.parse::<usize>().ok()?;
        let payload = lines.collect::<Vec<&str>>().join("\r\n").into_bytes();
        let truncated_payload = std::str::from_utf8(&payload[..length]).ok()?.to_string();
        Some(truncated_payload)
    });

    Some(Request {
        method,
        headers,
        search,
        body,
        path,
    })
}

fn parse_search(search: &str) -> Option<HashMap<String, String>> {
    let mut search_map = HashMap::new();
    let split_iter = search.split("&");
    for split in split_iter {
        let (key, value) = split.split_once("=")?;
        search_map.insert(key.to_string(), value.to_string());
    }

    Some(search_map)
}

#[derive(Debug)]
struct Request {
    path: String,
    method: HTTPMethod,
    search: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Option<String>,
}

impl Display for Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = format!("{} {}", self.method, self.path);
        write!(f, "{}", &msg)
    }
}

struct ResponseBuilder {
    status: Option<usize>,
    headers: HashMap<String, String>,
    body: Option<String>,
}

impl ResponseBuilder {
    fn new() -> Self {
        ResponseBuilder {
            body: None,
            status: None,
            headers: HashMap::new(),
        }
    }

    fn set_status(&mut self, status: usize) {
        self.status = Some(status)
    }

    fn set_header(&mut self, key: String, value: String) {
        self.headers.insert(key, value);
    }

    fn set_body(&mut self, body: String) {
        self.body = Some(body)
    }

    fn build(mut self) -> String {
        let status = self.status.expect("No status provided for response");

        let status_text = match status {
            200 => "OK",
            400 => "BAD REQUEST",
            401 => "UNAUTHORIZED",
            404 => "NOT FOUND",
            405 => "METHOD NOT ALLOWED",
            _ => "",
        };

        let mut response = format!("HTTP/1.1 {status} {status_text}\r\n");

        let concat_headers = |headers: HashMap<String, String>| {
            headers
                .into_iter()
                .map(|(key, value)| format!("{}:{}\r\n", key, value))
                .collect::<String>()
        };

        match self.body {
            None => {
                let headers = concat_headers(self.headers);
                response.push_str(&headers);
                response.push_str("\r\n");
            }
            Some(body) => {
                self.headers
                    .insert("content-length".to_string(), body.len().to_string());
                let headers = concat_headers(self.headers);
                response.push_str(&headers);
                response.push_str("\r\n");
                response.push_str(&body)
            }
        };

        response
    }
}

#[derive(Debug)]
enum HTTPMethod {
    GET,
    POST,
    OPTIONS,
    DELETE,
}

impl Display for HTTPMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HTTPMethod::GET => write!(f, "GET"),
            HTTPMethod::POST => write!(f, "POST"),
            HTTPMethod::OPTIONS => write!(f, "OPTIONS"),
            HTTPMethod::DELETE => write!(f, "DELETE"),
        }
    }
}

#[derive(Debug)]
enum HttpError {
    NotFound,
    Unauthorized,
    InternalServerError,
    BadRequest,
    MethodNotAllowed,
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::NotFound => write!(f, "404 Not Found"),
            HttpError::InternalServerError => write!(f, "500 Internal Server Error"),
            HttpError::BadRequest => write!(f, "400 Bad Request"),
            HttpError::MethodNotAllowed => write!(f, "405 Method Not Allowed"),
            HttpError::Unauthorized => write!(f, "401 Unauthorized"),
        }
    }
}

#[derive(Debug)]
pub enum SessionCommand {
    AddStreamer(Session),
    AddViewer(Session),
    GetStreamSDP((tokio::sync::oneshot::Sender<Option<SDP>>, String)),
    GetRooms(Sender<Vec<String>>),
}
