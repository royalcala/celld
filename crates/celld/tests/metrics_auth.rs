// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! `/metrics` scrape policy over the real internal listener.
//!
//! The exposition lives on the operator listener, so its default protection is
//! the network boundary. `CELLD_METRICS_TOKEN` narrows that to a scraper that
//! presents `Authorization: Bearer <token>`. This test starts a fleet, deploys
//! a minimal Worker, and then runs one node with a token and one without, so
//! the refusal path, the challenge header, and the unauthenticated default are
//! all measured against the real route. It skips without `tools/s3gw` or
//! `esbuild`, like the other laboratory tests.

#![allow(clippy::disallowed_methods)]
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a free port")
        .local_addr()
        .expect("read the bound port")
        .port()
}

fn wait_for<F>(timeout: Duration, mut ready: F) -> bool
where
    F: FnMut() -> bool,
{
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if ready() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn s3gw_binary() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CELLD_S3GW") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/s3gw/target/release/us-s3gw");
    path.is_file().then_some(path)
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// One HTTP GET against the internal listener, returning the complete response
/// so the assertions can see the status line, headers and body.
fn get(port: u16, path: &str, authorization: Option<&str>) -> std::io::Result<String> {
    let address = format!("127.0.0.1:{port}").parse().expect("parse address");
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
    if let Some(value) = authorization {
        request.push_str(&format!("Authorization: {value}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes())?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    Ok(String::from_utf8_lossy(&response).into_owned())
}

fn status_line(response: &str) -> &str {
    response.lines().next().unwrap_or_default()
}

fn spawn_node(
    endpoint: &str,
    bucket: &str,
    node: &str,
    log: &Path,
    token: Option<&str>,
) -> (ChildGuard, u16, u16) {
    let internal = free_port();
    let public = free_port();
    let out = std::fs::File::create(log).expect("create the node log");
    let err = out.try_clone().expect("clone the node log");
    let mut command = Command::new(env!("CARGO_BIN_EXE_celld"));
    command
        .args([
            "--bucket".to_owned(),
            format!("s3://{bucket}"),
            "--endpoint".to_owned(),
            endpoint.to_owned(),
            "--listen".to_owned(),
            format!("127.0.0.1:{public}"),
            "--internal-listen".to_owned(),
            format!("127.0.0.1:{internal}"),
            "--advertise".to_owned(),
            format!("127.0.0.1:{internal}"),
        ])
        .env("CELLD_NODE", node)
        .env("CELLD_TTL_MS", "4000")
        .env("CELLD_WAKER_TICK_MS", "2000")
        .env("AWS_ACCESS_KEY_ID", "minioadmin")
        .env("AWS_SECRET_ACCESS_KEY", "minioadmin")
        .env("AWS_REGION", "us-east-1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    if let Some(token) = token {
        command.env("CELLD_METRICS_TOKEN", token);
    }
    (
        ChildGuard(command.spawn().expect("start a celld node")),
        internal,
        public,
    )
}

/// Wait for the internal listener to answer the exposition at all, so a
/// refusal assertion is about the policy and not a not-yet-served route.
fn wait_for_internal(port: u16, token: Option<&str>, log: &Path) -> bool {
    wait_for(Duration::from_secs(180), || {
        get(port, "/metrics", token).is_ok()
    }) || {
        eprintln!(
            "node never answered:\n{}",
            std::fs::read_to_string(log).unwrap_or_default()
        );
        false
    }
}

/// Drive one public request so the registry has a counter to expose. The
/// metrics registry only renders series the runtime has already recorded, so
/// without traffic the exposition is empty even when the route serves it.
fn warm_the_registry(public: u16) {
    assert!(
        wait_for(Duration::from_secs(120), || {
            get(public, "/", None).is_ok_and(|response| status_line(&response).contains("200"))
        }),
        "the public listener never served the Worker"
    );
}

#[test]
fn metrics_scrape_policy_on_the_internal_listener() {
    let Some(s3gw) = s3gw_binary() else {
        eprintln!("skipping metrics auth test: tools/s3gw is not built");
        return;
    };
    if Command::new("esbuild").arg("--version").output().is_err() {
        eprintln!("skipping metrics auth test: esbuild is not on PATH");
        return;
    }

    let root = tempfile::tempdir().expect("temporary root");
    let bucket = format!("celld-metrics-{}", std::process::id());
    std::fs::create_dir_all(root.path().join(&bucket)).expect("create the bucket directory");
    let s3_port = free_port();
    let endpoint = format!("http://127.0.0.1:{s3_port}");

    let gw_out = std::fs::File::create(root.path().join("s3gw.log")).expect("create the log");
    let gw_err = gw_out.try_clone().expect("clone the log");
    let _gateway = ChildGuard(
        Command::new(&s3gw)
            .args([
                "--host".to_owned(),
                "127.0.0.1".to_owned(),
                "--port".to_owned(),
                s3_port.to_string(),
                "--access-key".to_owned(),
                "minioadmin".to_owned(),
                "--secret-key".to_owned(),
                "minioadmin".to_owned(),
                root.path().to_string_lossy().into_owned(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::from(gw_out))
            .stderr(Stdio::from(gw_err))
            .spawn()
            .expect("start us-s3gw"),
    );
    assert!(
        wait_for(Duration::from_secs(30), || TcpStream::connect_timeout(
            &format!("127.0.0.1:{s3_port}").parse().unwrap(),
            Duration::from_millis(300)
        )
        .is_ok()),
        "us-s3gw never listened"
    );

    // A node waits for the fleet pointer before it serves, so deploy first.
    let wrangler =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello/wrangler.jsonc");
    let deployed = Command::new(env!("CARGO_BIN_EXE_celld"))
        .args([
            "deploy".to_owned(),
            wrangler.to_string_lossy().into_owned(),
            "--bucket".to_owned(),
            format!("s3://{bucket}"),
            "--endpoint".to_owned(),
            endpoint.clone(),
        ])
        .env("AWS_ACCESS_KEY_ID", "minioadmin")
        .env("AWS_SECRET_ACCESS_KEY", "minioadmin")
        .env("AWS_REGION", "us-east-1")
        .stdin(Stdio::null())
        .output()
        .expect("run celld deploy");
    assert!(
        deployed.status.success(),
        "celld deploy failed: {}\n{}",
        String::from_utf8_lossy(&deployed.stderr),
        String::from_utf8_lossy(&deployed.stdout)
    );

    // ── The token policy ────────────────────────────────────────────────────
    let token = "scrape-secret-9f3a";
    let token_log = root.path().join("node-token.log");
    let (_token_node, token_port, token_public) = spawn_node(
        &endpoint,
        &bucket,
        "node-metrics-token",
        &token_log,
        Some(token),
    );
    assert!(
        wait_for_internal(token_port, Some(token), &token_log),
        "the token node never answered"
    );
    warm_the_registry(token_public);

    let anonymous = get(token_port, "/metrics", None).expect("scrape without a token");
    assert!(
        status_line(&anonymous).contains("401"),
        "an anonymous scrape must be refused, got: {}",
        status_line(&anonymous)
    );
    assert!(
        anonymous
            .to_ascii_lowercase()
            .contains("www-authenticate: bearer"),
        "a 401 must carry the Bearer challenge, got: {anonymous}"
    );
    assert!(
        !anonymous.contains("celld_http_requests_total"),
        "a refused scrape must not leak the exposition"
    );

    let wrong =
        get(token_port, "/metrics", Some("Bearer wrong-token")).expect("scrape with a wrong token");
    assert!(
        status_line(&wrong).contains("401"),
        "a wrong bearer must be refused, got: {}",
        status_line(&wrong)
    );

    let authorized = get(token_port, "/metrics", Some(&format!("Bearer {token}")))
        .expect("scrape with the token");
    assert!(
        status_line(&authorized).contains("200"),
        "the configured token must authenticate, got: {}",
        status_line(&authorized)
    );
    assert!(
        authorized.contains("celld_http_requests_total"),
        "an authorized scrape must carry the exposition, got: {authorized}"
    );

    // ── The network-boundary default ────────────────────────────────────────
    let open_log = root.path().join("node-open.log");
    let (_open_node, open_port, open_public) =
        spawn_node(&endpoint, &bucket, "node-metrics-open", &open_log, None);
    assert!(
        wait_for_internal(open_port, None, &open_log),
        "the open node never answered"
    );
    warm_the_registry(open_public);

    let open = get(open_port, "/metrics", None).expect("scrape the open node");
    assert!(
        status_line(&open).contains("200"),
        "the default must serve the private listener, got: {}",
        status_line(&open)
    );
    assert!(
        open.contains("celld_http_requests_total"),
        "the default exposition must be present, got: {open}"
    );
}
