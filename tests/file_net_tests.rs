#![allow(
    clippy::all,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::as_underscore,
    clippy::fn_to_numeric_cast_any,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::redundant_pub_crate,
    clippy::missing_const_for_fn
)]
//! Integration tests for file and network commands (Task 10).

use std::net::TcpListener;
use std::sync::Arc;

use dyyl::runtime::configure_agent_for_testing;
use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn eval_values(source: &str) -> Vec<Value> {
    run_script(source, false).values
}

fn eval_one(source: &str) -> Value {
    eval_values(source)
        .into_iter()
        .next()
        .unwrap_or(Value::Empty)
}

struct TestServer {
    addr: std::net::SocketAddr,
    _rt: tokio::runtime::Runtime,
}

fn start_test_server() -> TestServer {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let key_pair = rcgen::KeyPair::generate().expect("key gen");
    let params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).expect("params");
    let cert = params.self_signed(&key_pair).expect("self-signed");

    let cert_der = cert.der().clone();
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(key_pair.serialize_der()),
    );

    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], key_der)
        .expect("server config");

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local addr");
    listener.set_nonblocking(true).expect("nonblocking");

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

    rt.spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => continue,
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let tls = match acceptor.accept(stream).await {
                    Ok(s) => s,
                    Err(_) => return,
                };
                handle_connection(tls).await;
            });
        }
    });

    configure_agent_for_testing(make_trusting_agent(&cert_der));

    TestServer { addr, _rt: rt }
}

fn make_trusting_agent(cert: &rustls::pki_types::CertificateDer<'static>) -> ureq::Agent {
    let mut store = rustls::RootCertStore::empty();
    store.add(cert.clone()).expect("add cert");
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(store)
        .with_no_client_auth();
    ureq::AgentBuilder::new()
        .tls_config(Arc::new(config))
        .build()
}

async fn handle_connection(mut stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).await.unwrap_or(0);
    let request = String::from_utf8_lossy(&buf[..n]);

    let body = if request.contains("/hello") {
        "Hello from test server"
    } else {
        "Not found"
    };

    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes()).await;
}

#[test]
fn file_and_network_commands_local_https_only() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let server = start_test_server();
    let base = format!("https://localhost:{}", server.addr.port());

    let write_path = tmp.path().join("test.txt");
    let wp = write_path.to_str().expect("path str");

    // ── file.write overwrite ──────────────────────────────────────
    let v = eval_one(&format!("file.write \"{wp}\", hello"));
    assert_eq!(
        v,
        Value::Str("hello".to_string()),
        "file.write returns content"
    );

    let v = eval_one(&format!("file.read \"{wp}\""));
    assert_eq!(v, Value::Str("hello".to_string()), "file.read after write");

    // ── file.write overwrites ─────────────────────────────────────
    let v = eval_one(&format!("file.write \"{wp}\", world"));
    assert_eq!(
        v,
        Value::Str("world".to_string()),
        "file.write overwrite returns content"
    );

    let v = eval_one(&format!("file.read \"{wp}\""));
    assert_eq!(
        v,
        Value::Str("world".to_string()),
        "file.read after overwrite"
    );

    // ── file.append ───────────────────────────────────────────────
    let v = eval_one(&format!("file.append \"{wp}\", !"));
    assert_eq!(
        v,
        Value::Str("!".to_string()),
        "file.append returns appended content"
    );

    let v = eval_one(&format!("file.read \"{wp}\""));
    assert_eq!(
        v,
        Value::Str("world!".to_string()),
        "file.read after append"
    );

    // ── relative path rejection ───────────────────────────────────
    let v = eval_one("file.write relative.txt, test");
    assert_eq!(v, Value::Str(String::new()), "relative write → sentinel");

    let v = eval_one("file.read relative.txt");
    assert_eq!(v, Value::Str(String::new()), "relative read → sentinel");

    let v = eval_one("file.append relative.txt, test");
    assert_eq!(v, Value::Str(String::new()), "relative append → sentinel");

    // ── net.get local HTTPS ───────────────────────────────────────
    let url = format!("{base}/hello");
    let v = eval_one(&format!("net.get \"{url}\""));
    assert_eq!(
        v,
        Value::Str("Hello from test server".to_string()),
        "net.get returns body"
    );

    // ── net.download ──────────────────────────────────────────────
    let dl_path = tmp.path().join("download.txt");
    let dp = dl_path.to_str().expect("dl path str");
    let v = eval_one(&format!("net.download \"{url}\", \"{dp}\""));
    let expected_bytes = "Hello from test server".len() as i64;
    assert_eq!(
        v,
        Value::Num(expected_bytes),
        "net.download returns byte count"
    );

    let v = eval_one(&format!("file.read \"{dp}\""));
    assert_eq!(
        v,
        Value::Str("Hello from test server".to_string()),
        "downloaded file matches response"
    );

    // ── failed HTTPS request sentinel ─────────────────────────────
    let v = eval_one("net.get \"https://localhost:1/nonexistent\"");
    assert_eq!(v, Value::Str(String::new()), "failed net.get → sentinel");

    let v = eval_one("net.download \"https://localhost:1/nonexistent\", \"/tmp/should-not-exist\"");
    assert_eq!(
        v,
        Value::Str(String::new()),
        "failed net.download → sentinel"
    );
}
