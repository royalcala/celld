// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! The two-node laboratory case this whole change exists for.
//!
//! A Durable Object on node B keeps a subscription capability whose target lives
//! in a stateless Worker isolate on node A. The object cannot store the stub, so
//! it stores the token; node B is then SIGKILLed and restarted, the object is
//! restored from the bucket with nothing in memory, and the alarm it armed
//! restores the token and pushes a delivery to the session on node A. The client
//! does nothing: no resubscribe, no replay request, no repair.
//!
//! It needs `tools/s3gw` and esbuild; without them it skips, like the other lab
//! tests. `CELLD_S3GW` overrides the gateway path.

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
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => return false,
        }
    }
    let _ = child.kill();
    false
}

fn kill(child: &mut Child) {
    let _ = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGKILL) };
    let _ = wait_for_exit(child, Duration::from_secs(30));
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

fn http_get(port: u16, path: &str) -> std::io::Result<String> {
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    Ok(String::from_utf8_lossy(&response).into_owned())
}

/// The response body, with the chunked framing a celld HTTP response uses stripped.
fn response_body(raw: &str) -> String {
    let Some((_, rest)) = raw.split_once("\r\n\r\n") else {
        return String::new();
    };
    for line in rest.split("\r\n") {
        let line = line.trim();
        if line.is_empty() || line == "0" || line.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        return line.to_owned();
    }
    String::new()
}

/// `/ping` on the fixture Worker; empty while the node is not serving. It never
/// touches the Durable Object, so a readiness probe cannot claim the cell.
fn ping(port: u16) -> String {
    http_get(port, "/ping")
        .map(|raw| response_body(&raw))
        .unwrap_or_default()
}

fn connect_websocket(port: u16, path: &str) -> std::io::Result<TcpStream> {
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\n\
         Host: localhost\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\
         \r\n"
    )?;
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while head.len() < 16 * 1024 {
        stream.read_exact(&mut byte)?;
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let head_text = String::from_utf8_lossy(&head);
    if !head_text.starts_with("HTTP/1.1 101") {
        return Err(std::io::Error::other(format!(
            "WebSocket upgrade failed: {head_text}"
        )));
    }
    Ok(stream)
}

fn read_server_text_frame(stream: &mut TcpStream) -> std::io::Result<String> {
    loop {
        let mut header = [0u8; 2];
        stream.read_exact(&mut header)?;
        let opcode = header[0] & 0x0f;
        let masked = header[1] & 0x80 != 0;
        let mut length = (header[1] & 0x7f) as usize;
        if length == 126 {
            let mut extended = [0u8; 2];
            stream.read_exact(&mut extended)?;
            length = u16::from_be_bytes(extended) as usize;
        } else if length == 127 {
            let mut extended = [0u8; 8];
            stream.read_exact(&mut extended)?;
            length = u64::from_be_bytes(extended) as usize;
        }
        let mut mask = [0u8; 4];
        if masked {
            stream.read_exact(&mut mask)?;
        }
        let mut payload = vec![0u8; length];
        stream.read_exact(&mut payload)?;
        if masked {
            for (index, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[index % 4];
            }
        }
        match opcode {
            0x1 => return Ok(String::from_utf8_lossy(&payload).into_owned()),
            0x8 => return Err(std::io::Error::other("the session socket closed")),
            _ => continue,
        }
    }
}

/// Sends one masked text frame and returns the next text frame the server sends.
fn ws_client_text(stream: &mut TcpStream, text: &str) -> std::io::Result<String> {
    let payload = text.as_bytes();
    let mask = [0x11u8, 0x22, 0x33, 0x44];
    let mut frame = vec![0x81u8];
    if payload.len() < 126 {
        frame.push(0x80 | payload.len() as u8);
    } else {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    }
    frame.extend_from_slice(&mask);
    frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    stream.write_all(&frame)?;
    read_server_text_frame(stream)
}

/// The node named in the bucket's ownership record, if the store is on local disk.
fn owner_record(bucket_root: &Path) -> Option<String> {
    fn walk(dir: &Path, depth: usize, found: &mut Option<PathBuf>) -> bool {
        if depth > 6 || found.is_some() {
            return found.is_some();
        }
        let Some(entries) = std::fs::read_dir(dir).ok() else {
            return false;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if walk(&path, depth + 1, found) {
                    return true;
                }
            } else if path.file_name().is_some_and(|name| name == "own.json") {
                *found = Some(path);
                return true;
            }
        }
        found.is_some()
    }
    let mut found = None;
    walk(bucket_root, 0, &mut found);
    let text = std::fs::read_to_string(found?).ok()?;
    for node in ["node-a", "node-b"] {
        if text.contains(&format!("\"{node}\"")) {
            return Some(node.to_owned());
        }
    }
    None
}

/// All the processes the test owns, so a panic cannot leak a node.
struct Stack {
    gateway: Child,
    nodes: [Option<Child>; 2],
    root: tempfile::TempDir,
    bucket: String,
}

impl Stack {
    fn log(&self, name: &str) -> String {
        std::fs::read_to_string(self.root.path().join(name)).unwrap_or_default()
    }

    fn start_node(
        &mut self,
        index: usize,
        node: &str,
        port: u16,
        internal: u16,
        endpoint: &str,
        log: &str,
    ) -> u16 {
        // A restart must free the listeners first: the replacement binds the
        // same ports.
        if let Some(previous) = self.nodes[index].take() {
            let mut previous = previous;
            kill(&mut previous);
        }
        let out = std::fs::File::create(self.root.path().join(log)).expect("create the node log");
        let err = out.try_clone().expect("clone the node log");
        let child = Command::new(env!("CARGO_BIN_EXE_celld"))
            .args([
                "--bucket".to_owned(),
                format!("s3://{}", self.bucket),
                "--endpoint".to_owned(),
                endpoint.to_owned(),
                "--listen".to_owned(),
                format!("127.0.0.1:{port}"),
                "--internal-listen".to_owned(),
                format!("127.0.0.1:{internal}"),
                "--advertise".to_owned(),
                format!("127.0.0.1:{internal}"),
            ])
            .env("CELLD_NODE", node)
            // The takeover lease, and a waker tick short enough that the alarm a
            // restored object armed is found in test time rather than in a minute.
            .env("CELLD_TTL_MS", "4000")
            .env("CELLD_WAKER_TICK_MS", "2000")
            .env("AWS_ACCESS_KEY_ID", "minioadmin")
            .env("AWS_SECRET_ACCESS_KEY", "minioadmin")
            .env("AWS_REGION", "us-east-1")
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("start a celld node");
        self.nodes[index] = Some(child);
        port
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        for child in self.nodes.iter_mut().flatten() {
            kill(child);
        }
        kill(&mut self.gateway);
    }
}

#[test]
fn a_restored_object_rearms_a_session_capability_across_nodes() {
    let Some(s3gw) = s3gw_binary() else {
        eprintln!("skipping 2-node persisted-capability test: tools/s3gw is not built");
        return;
    };
    if Command::new("esbuild").arg("--version").output().is_err() {
        eprintln!("skipping 2-node persisted-capability test: esbuild is not on PATH");
        return;
    }

    let root = tempfile::tempdir().expect("temporary root");
    let bucket = format!("celld-fleet-{}", std::process::id());
    std::fs::create_dir_all(root.path().join(&bucket)).expect("create the bucket directory");
    let s3_port = free_port();
    let endpoint = format!("http://127.0.0.1:{s3_port}");

    let gw_out = std::fs::File::create(root.path().join("s3gw.log")).expect("create the log");
    let gw_err = gw_out.try_clone().expect("clone the log");
    let gateway = Command::new(&s3gw)
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
        .expect("start us-s3gw");
    let mut stack = Stack {
        gateway,
        nodes: [None, None],
        root,
        bucket: bucket.clone(),
    };
    if !wait_for(Duration::from_secs(30), || {
        TcpStream::connect_timeout(
            &format!("127.0.0.1:{s3_port}").parse().unwrap(),
            Duration::from_millis(300),
        )
        .is_ok()
    }) {
        panic!("us-s3gw never listened:\n{}", stack.log("s3gw.log"));
    }

    // The deployment: one Worker, one Durable Object class.
    let project = stack.root.path().join("project");
    std::fs::create_dir_all(&project).expect("create the project");
    std::fs::write(
        project.join("wrangler.jsonc"),
        include_str!("fixtures/persisted-capability-2node/wrangler.jsonc"),
    )
    .expect("write the wrangler config");
    std::fs::write(
        project.join("worker.js"),
        include_str!("fixtures/persisted-capability-2node/worker.js"),
    )
    .expect("write the worker");
    let deployed = Command::new(env!("CARGO_BIN_EXE_celld"))
        .args([
            "deploy".to_owned(),
            project
                .join("wrangler.jsonc")
                .to_string_lossy()
                .into_owned(),
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
        "deploy failed: {}\n{}",
        String::from_utf8_lossy(&deployed.stderr),
        String::from_utf8_lossy(&deployed.stdout)
    );

    let a_port = free_port();
    let a_internal = free_port();
    let b_port = free_port();
    let b_internal = free_port();
    let bucket_root = stack.root.path().join(&bucket);

    // Node B first, and it claims the cell: the object has to live on the node
    // that is not the session's.
    stack.start_node(1, "node-b", b_port, b_internal, &endpoint, "node-b.log");
    if !wait_for(Duration::from_secs(120), || ping(b_port) == "pong") {
        panic!("node-b never became ready:\n{}", stack.log("node-b.log"));
    }
    let _ = http_get(b_port, "/claim");
    if !wait_for(Duration::from_secs(60), || {
        owner_record(&bucket_root).as_deref() == Some("node-b")
    }) {
        panic!(
            "node-b never owned the cell (owner={:?}):\n{}",
            owner_record(&bucket_root),
            stack.log("node-b.log")
        );
    }

    // Node A serves the session.
    stack.start_node(0, "node-a", a_port, a_internal, &endpoint, "node-a.log");
    if !wait_for(Duration::from_secs(120), || ping(a_port) == "pong") {
        panic!("node-a never became ready:\n{}", stack.log("node-a.log"));
    }
    eprintln!(
        "topology: session on node-a:{a_port}, object owned by {:?}",
        owner_record(&bucket_root)
    );

    let mut socket = connect_websocket(a_port, "/session").expect("open the session socket");

    // Arm: the session on A hands its capability to the object on B, which keeps
    // the token and nothing else.
    let armed = ws_client_text(&mut socket, "arm").expect("arm the subscription");
    assert_eq!(armed, "armed", "the object did not arm: {armed}");
    eprintln!("armed on node-b, token persisted in object storage");

    // The wake entry ships asynchronously; the runbook says to allow a few
    // seconds before a hard kill, or the owner dies with an unpublished entry.
    std::thread::sleep(Duration::from_secs(5));

    // Hard-kill node B and bring it back. Its cell, its storage and its alarm are
    // now the bucket's: nothing of the old process survives.
    stack.start_node(1, "node-b", b_port, b_internal, &endpoint, "node-b.log");
    if !wait_for(Duration::from_secs(120), || ping(b_port) == "pong") {
        panic!("node-b never came back:\n{}", stack.log("node-b.log"));
    }
    if !wait_for(Duration::from_secs(60), || {
        owner_record(&bucket_root).as_deref() == Some("node-b")
    }) {
        panic!(
            "the restarted node-b did not own the cell (owner={:?}):\n{}",
            owner_record(&bucket_root),
            stack.log("node-b.log")
        );
    }
    eprintln!("node-b restarted (fresh process, fresh isolate: the object kept only its rows)");

    // The delivery: the alarm fires on the restored object, `restoreCapability`
    // rebuilds the handle from the stored token, and the call crosses to the
    // session isolate on node A. The client sent nothing.
    socket
        .set_read_timeout(Some(Duration::from_secs(120)))
        .expect("set the socket timeout");
    let delivered = read_server_text_frame(&mut socket);
    match &delivered {
        Ok(frame) if frame == "delivered:alarm" => {
            eprintln!("the session on node-a received {frame:?} with no client action");
        }
        other => {
            let outcome = http_get(b_port, "/outcome")
                .map(|raw| response_body(&raw))
                .unwrap_or_default();
            let b_log = stack.log("node-b.log");
            let a_log = stack.log("node-a.log");
            panic!(
                "the restored capability did not deliver to the session on node A: {other:?}\n\
                 object outcome: {outcome}\n--- node-b ---\n{b_log}\n--- node-a ---\n{a_log}"
            );
        }
    }

    // The second half of the fence: with node A gone, the same stored token must
    // fail typed rather than resolve to whatever now has that handle number.
    let rearmed = ws_client_text(&mut socket, "rearm").expect("rearm the alarm");
    assert_eq!(rearmed, "rearmed", "the object did not rearm: {rearmed}");
    if let Some(mut node_a) = stack.nodes[0].take() {
        kill(&mut node_a);
    }
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut outcome = String::new();
    while Instant::now() < deadline {
        outcome = http_get(b_port, "/outcome")
            .map(|raw| response_body(&raw))
            .unwrap_or_default();
        if outcome.starts_with("err:") {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    assert!(
        outcome.starts_with("err:owner-moved")
            || outcome.starts_with("err:owner-gone")
            || outcome.starts_with("err:transport")
            || outcome.starts_with("err:stale-generation"),
        "a delivery to a dead session did not fail typed: {outcome:?}\n{}",
        stack.log("node-b.log")
    );
    eprintln!("with node-a gone the same stored token failed typed: {outcome}");
}
