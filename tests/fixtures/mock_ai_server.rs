//! Mock AI HTTP server for testing.

use std::sync::Arc;
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct MockAiServer {
    pub port: u16,
    pub requests: Arc<Mutex<Vec<MockRequest>>>,
    shutdown: Arc<tokio::sync::Notify>,
}

#[derive(Clone, Debug)]
pub struct MockRequest {
    pub path: String,
    pub body: String,
}

impl MockAiServer {
    pub async fn start(response_body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(tokio::sync::Notify::new());
        let requests_clone = Arc::clone(&requests);
        let shutdown_clone = Arc::clone(&shutdown);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept = listener.accept() => {
                        let (mut sock, _) = match accept {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let requests_clone = Arc::clone(&requests_clone);
                        let response_body = response_body.clone();
                        tokio::spawn(async move {
                            let mut buf = vec![0u8; 8192];
                            let n = sock.read(&mut buf).await.unwrap_or(0);
                            let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                            let (path, body) = parse_http_request(&raw);
                            requests_clone.lock().unwrap().push(MockRequest { path, body });
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                                response_body.len(),
                                response_body
                            );
                            let _ = sock.write_all(resp.as_bytes()).await;
                        });
                    }
                    _ = shutdown_clone.notified() => break,
                }
            }
        });
        Self { port, requests, shutdown }
    }

    pub fn stop(&self) {
        self.shutdown.notify_waiters();
    }

    pub fn captured_requests(&self) -> Vec<MockRequest> {
        self.requests.lock().unwrap().clone()
    }
}

fn parse_http_request(raw: &str) -> (String, String) {
    let mut lines = raw.split("\r\n");
    let first_line = lines.next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("").to_string();
    let mut body = String::new();
    let mut in_body = false;
    for line in lines {
        if in_body {
            body.push_str(line);
        } else if line.is_empty() {
            in_body = true;
        }
    }
    (path, body)
}
