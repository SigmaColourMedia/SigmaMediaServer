use std::{fs, thread};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct FileStorageBuilder {
    tcp_listener: Option<TcpListener>,
    cors_origin: Option<String>,
    storage_path: Option<PathBuf>,
}

impl FileStorageBuilder {
    pub fn new() -> Self {
        Self {
            cors_origin: None,
            tcp_listener: None,
            storage_path: None,
        }
    }
    pub fn add_address<A: ToSocketAddrs>(mut self, address: A) -> Self {
        let tcp_listener = TcpListener::bind(address).expect("Socket address should be available");
        self.tcp_listener = Some(tcp_listener);
        self
    }

    pub fn add_cors_origin(mut self, origin: String) -> Self {
        self.cors_origin = Some(origin);
        self
    }

    pub fn add_storage_path(mut self, path: &str) -> Self {
        self.storage_path = Some(PathBuf::from(path));
        self
    }

    pub fn build(self) -> FileStorage {
        FileStorage {
            storage_path: self.storage_path.expect("Storage path should be defined"),
            cors_origin: self.cors_origin.expect("CORS origin should be defined"),
            tcp_listener: self.tcp_listener.expect("TCP listener should be defined"),
        }
    }
}

pub struct FileStorage {
    tcp_listener: TcpListener,
    cors_origin: String,
    storage_path: PathBuf,
}

impl FileStorage {
    pub fn startup(&self) {
        let origin = Arc::new(self.cors_origin.clone());
        let storage_path = Arc::new(self.storage_path.clone());

        // Check for stale files in interval and remove them
        // todo Fix this disaster. Maybe a daemon could handle this?
        let storage_path_clone = storage_path.clone();
        thread::spawn(move || {
            let mut time_reference = Instant::now();
            loop {
                if time_reference.elapsed().gt(&Duration::from_secs(1)) {
                    remove_stale_files(storage_path_clone.as_path());
                    time_reference = Instant::now()
                }
            }
        });

        for stream in self.tcp_listener.incoming() {
            if let Ok(mut stream) = stream {
                let origin = origin.clone();
                let path = storage_path.clone();
                // todo This can spiral out of control for large number of clients. Better use event loop or some thread-pool
                thread::spawn(move || {
                    if let Some(request) = read_request(&mut stream) {
                        let request_path = Path::new(&request.pathname);
                        if request_path.parent().ne(&Some(Path::new("/"))) {
                            let response = format!(
                                "HTTP/1.1 404 NOT FOUND\r\n\
                                Connection: keep-alive\r\n\
                                Cache-Control: no-cache\r\n\
                                Access-Control-Allow-Origin: {origin}\r\n\
                                Access-Control-Allow-Method: GET\r\n\r\n"
                            );
                            if let Err(e) = stream.write_all(response.as_bytes()) {
                                eprintln!("Error writing to socket {}", e)
                            }
                            return;
                        }
                        let target_file = request_path.file_name().and_then(|file_name| {
                            let mut file_pathname = PathBuf::from(path.as_path());
                            file_pathname.push(file_name);
                            std::fs::read(file_pathname.as_path()).ok()
                        });
                        match target_file {
                            None => {
                                let response = format!(
                                    "HTTP/1.1 404 NOT FOUND\r\n\
                                Connection: keep-alive\r\n\
                                Cache-Control: no-cache\r\n\
                                Access-Control-Allow-Origin: {origin}\r\n\
                                Access-Control-Allow-Method: GET\r\n\r\n"
                                );
                                if let Err(e) = stream.write_all(response.as_bytes()) {
                                    eprintln!("Error writing to socket {}", e)
                                }
                            }
                            Some(mut image_data) => {
                                let mut response = format!(
                                    "HTTP/1.1 200 OK\r\n\
                                Connection: close\r\n\
                                Access-Control-Allow-Origin: {origin}\r\n\
                                Access-Control-Allow-Method: GET\r\n\
                                Content-Type: image/webp\r\n\
                                Content-Length: {}\r\n\r\n",
                                    image_data.len()
                                )
                                .into_bytes();
                                response.append(&mut image_data);

                                // todo handle error
                                if let Err(e) = stream.write_all(&response) {
                                    eprintln!("Error writing to socket {}", e)
                                }
                            }
                        }
                    }
                });
            }
        }
    }
}

fn remove_stale_files(path: &Path) {
    let files = fs::read_dir(path).expect("Should read files directory");
    for entry in files {
        let entry = entry.expect("Should read entry");
        let is_stale_file = entry
            .metadata()
            .expect("Should read entry's metadata")
            .accessed()
            .expect("Should read access time")
            .elapsed()
            .expect("File should have correct last access time")
            .gt(&Duration::from_secs(300));

        if is_stale_file {
            fs::remove_file(entry.path()).expect("Should remove file")
        }
    }
}

// todo This copies/uses very similar code to notification_bus and http_server. Unify this at some point please.
struct Request {
    pathname: String,
    headers: HashMap<String, String>,
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut heading = String::new();
    reader.read_line(&mut heading).ok()?;
    let mut heading_split = heading.split(" ");
    let method = heading_split.next()?;

    if !method.eq_ignore_ascii_case("GET") {
        return None;
    }

    let pathname = heading_split.next()?.to_string();
    let mut headers = HashMap::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;

        if header.trim().is_empty() {
            break;
        }

        let (key, value) = header.split_once(":")?;
        headers.insert(key.to_string(), value.to_string());
    }

    Some(Request { headers, pathname })
}

mod tests {
    use std::ffi::OsStr;
    use std::path::Path;

    #[test]
    fn parses_pathname() {
        let pathname = "/test";
        let path = Path::new(pathname);
        assert_eq!(path.has_root(), true);
        assert_eq!(path.parent(), Some(Path::new("/images")));
        assert_eq!(path.file_name(), Some(OsStr::new("imagename")));
    }
}
