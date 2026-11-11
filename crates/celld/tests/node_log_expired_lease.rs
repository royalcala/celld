// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! CELLD-16: an ensemble member whose lease record is expired past the grace
//! and whose stored address now belongs to a different process answers 404.
//! Recovery used to count that answer as "undecided" forever, so the session
//! never sealed. After the grace the member is conclusively gone: recovery
//! declares the bounded loss, drains the retained bundle and seals.
//!
//! The production shape, reproduced here: node-a owns `__KvNamespace` and a
//! fleet-acked write; node-b joins as a bucket-posture follower. After node-b
//! dies its lease record is replaced with a long-expired one whose addr is a
//! stale server that answers 404. node-a dies and node-c takes the namespace
//! over. It must seal without anyone deleting `log/<session>/` by hand.
//!
//! Needs `tools/s3gw` and esbuild; skips otherwise. `CELLD_S3GW` overrides the
//! gateway path.

#![allow(clippy::disallowed_methods)]
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const NAMESPACE: &str = "example-values";
const KEY: &str = "c16-key";
const VALUE: &str = "c16-value";

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

fn kill(child: &mut Child) {
    let _ = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGKILL) };
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
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

fn body(port: u16, path: &str) -> String {
    http_get(port, path)
        .map(|raw| response_body(&raw))
        .unwrap_or_default()
}

fn kv(bucket: &str, endpoint: &str, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_celld"))
        .arg("kv")
        .args(arguments)
        .args([
            "--bucket",
            &format!("s3://{bucket}"),
            "--endpoint",
            endpoint,
        ])
        .env("AWS_ACCESS_KEY_ID", "minioadmin")
        .env("AWS_SECRET_ACCESS_KEY", "minioadmin")
        .env("AWS_REGION", "us-east-1")
        .stdin(Stdio::null())
        .output()
        .expect("run celld kv")
}

/// A stale address that answers every request with 404, the way a different
/// process on the old port answers a member it does not know.
struct StalePeer {
    port: u16,
    hits: Arc<AtomicUsize>,
}

impl StalePeer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the stale peer");
        let port = listener
            .local_addr()
            .expect("read the stale peer port")
            .port();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                counter.fetch_add(1, Ordering::Relaxed);
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                let _ = stream.flush();
            }
        });
        Self { port, hits }
    }
}

struct Stack {
    gateway: Child,
    nodes: [Option<Child>; 3],
    root: tempfile::TempDir,
    bucket: String,
}

impl Stack {
    fn log(&self, name: &str) -> String {
        std::fs::read_to_string(self.root.path().join(name)).unwrap_or_default()
    }

    fn bucket_root(&self) -> PathBuf {
        self.root.path().join(&self.bucket)
    }

    #[allow(clippy::too_many_arguments)]
    fn start_node(
        &mut self,
        index: usize,
        node: &str,
        port: u16,
        internal: u16,
        endpoint: &str,
        log: &str,
        extra: &[(&str, &str)],
    ) {
        if let Some(previous) = self.nodes[index].take() {
            let mut previous = previous;
            kill(&mut previous);
        }
        let out = std::fs::File::create(self.root.path().join(log)).expect("create the node log");
        let err = out.try_clone().expect("clone the node log");
        let mut command = Command::new(env!("CARGO_BIN_EXE_celld"));
        command
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
            .env("CELLD_TTL_MS", "4000")
            .env("CELLD_WAKER_TICK_MS", "2000")
            .env("AWS_ACCESS_KEY_ID", "minioadmin")
            .env("AWS_SECRET_ACCESS_KEY", "minioadmin")
            .env("AWS_REGION", "us-east-1")
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err));
        for (name, value) in extra {
            command.env(name, value);
        }
        self.nodes[index] = Some(command.spawn().expect("start a celld node"));
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

fn node_record(root: &Path, node: &str) -> Option<serde_json::Value> {
    let bytes = std::fs::read(root.join("nodes").join(format!("{node}.json"))).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[test]
fn a_long_expired_member_with_a_stale_address_seals_after_the_grace() {
    let Some(s3gw) = s3gw_binary() else {
        eprintln!("skipping expired-lease test: tools/s3gw is not built");
        return;
    };
    if Command::new("esbuild").arg("--version").output().is_err() {
        eprintln!("skipping expired-lease test: esbuild is not on PATH");
        return;
    }

    let root = tempfile::tempdir().expect("temporary root");
    let bucket = format!("celld-expired-{}", std::process::id());
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
        nodes: [None, None, None],
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

    let wrangler = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kv/wrangler.jsonc");
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
        "deploy failed: {}\n{}",
        String::from_utf8_lossy(&deployed.stderr),
        String::from_utf8_lossy(&deployed.stdout)
    );

    let a_port = free_port();
    let a_internal = free_port();
    let b_port = free_port();
    let b_internal = free_port();
    let c_port = free_port();
    let c_internal = free_port();
    let bucket_root = stack.bucket_root();

    stack.start_node(
        0,
        "node-a",
        a_port,
        a_internal,
        &endpoint,
        "node-a.log",
        &[],
    );
    if !wait_for(Duration::from_secs(120), || {
        body(a_port, "/missing") == "Not found."
    }) {
        panic!("node-a never became ready:\n{}", stack.log("node-a.log"));
    }
    let put = kv(&bucket, &endpoint, &["put", NAMESPACE, KEY, VALUE]);
    assert!(
        put.status.success(),
        "initial kv put failed: {}\n{}",
        String::from_utf8_lossy(&put.stderr),
        String::from_utf8_lossy(&put.stdout)
    );

    stack.start_node(
        1,
        "node-b",
        b_port,
        b_internal,
        &endpoint,
        "node-b.log",
        &[("CELLD_DURABILITY", "bucket")],
    );
    if !wait_for(Duration::from_secs(120), || {
        body(b_port, "/missing") == "Not found."
    }) {
        panic!("node-b never became ready:\n{}", stack.log("node-b.log"));
    }
    if !wait_for(Duration::from_secs(120), || {
        stack.log("node-a.log").contains("members={\"node-b\"}")
    }) {
        panic!(
            "node-a never recruited node-b:\n{}",
            stack.log("node-a.log")
        );
    }
    let put = kv(&bucket, &endpoint, &["put", NAMESPACE, KEY, VALUE]);
    assert!(
        put.status.success(),
        "fleet kv put failed: {}\n{}",
        String::from_utf8_lossy(&put.stderr),
        String::from_utf8_lossy(&put.stdout)
    );
    if !wait_for(Duration::from_secs(120), || {
        node_record(&bucket_root, "node-a")
            .and_then(|record| record.get("log").cloned())
            .and_then(|log| log.get("active").and_then(serde_json::Value::as_bool))
            == Some(true)
    }) {
        panic!(
            "node-a's session never recorded a fleet acknowledgement:\n{}",
            stack.log("node-a.log")
        );
    }

    // Replace node-b's lease with the production shape: long expired, and its
    // stored address now belongs to a different process that answers 404. The
    // folded `log` keeps the record alive so the dead-node GC tombstones it
    // instead of deleting it.
    if let Some(mut node_b) = stack.nodes[1].take() {
        kill(&mut node_b);
    }
    let stale = StalePeer::start();
    let now = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX);
    let member = serde_json::json!({
        "node": "node-b",
        "expires_ms": now.saturating_sub(60_000),
        "addr": format!("127.0.0.1:{}", stale.port),
        "ownership_index_generation": "stale",
        "log": {
            "state": "sealed",
            "epoch": 1,
            "ensemble": ["node-a"],
            "tiered": 0,
            "active": true
        }
    });
    std::fs::write(
        bucket_root.join("nodes/node-b.json"),
        serde_json::to_vec(&member).expect("serialize the stale member"),
    )
    .expect("write the stale member record");

    if let Some(mut node_a) = stack.nodes[0].take() {
        kill(&mut node_a);
    }
    std::thread::sleep(Duration::from_secs(1));

    stack.start_node(
        2,
        "node-c",
        c_port,
        c_internal,
        &endpoint,
        "node-c.log",
        &[],
    );

    let mut last = String::new();
    if !wait_for(Duration::from_secs(240), || {
        let get = kv(&bucket, &endpoint, &["get", NAMESPACE, KEY]);
        last = format!(
            "{}{}",
            String::from_utf8_lossy(&get.stdout),
            String::from_utf8_lossy(&get.stderr)
        );
        get.status.success() && String::from_utf8_lossy(&get.stdout).trim() == VALUE
    }) {
        panic!(
            "the namespace never served again through node-c; last output: {last}\n--- node-c ---\n{}\n--- node-a ---\n{}",
            stack.log("node-c.log"),
            stack.log("node-a.log")
        );
    }

    let c_log = stack.log("node-c.log");
    assert!(
        stale.hits.load(Ordering::Relaxed) > 0,
        "the stale address was never probed:\n{c_log}"
    );
    assert!(
        c_log.contains("declared bounded loss"),
        "recovery did not declare the bounded loss:\n{c_log}"
    );
    assert!(
        c_log.contains("node log recovered and sealed"),
        "recovery did not seal the session:\n{c_log}"
    );
    assert!(
        !c_log.contains("refusing to seal while member state remains unverified"),
        "the old refusal still happened:\n{c_log}"
    );
    let record = node_record(&bucket_root, "node-a").expect("node-a's record");
    assert_eq!(
        record
            .get("log")
            .and_then(|log| log.get("state"))
            .and_then(serde_json::Value::as_str),
        Some("sealed"),
        "node-a's session is not sealed: {record}"
    );
    eprintln!("the stale expired member sealed after the grace");
}
