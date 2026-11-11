// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! CELLD-14: a node-log session whose ensemble names a node that left no
//! lease record must seal on its own.
//!
//! This is the reported shape, on the reserved `__KvNamespace` cell. node-a
//! (fleet posture) owns the namespace cell and serves a fleet-acknowledged
//! write, so its log record may have fleet acknowledgements. node-b joins as a
//! bucket-posture follower: it never opens a log of its own, so when it dies
//! the dead-node GC deletes its lease record outright instead of tombstoning
//! it. node-a's session still names node-b. After node-a dies, a new node takes
//! the namespace over through `celld kv` and recovery must seal the session
//! without anyone deleting `log/<session>/` by hand.
//!
//! The test first requires the refusal (`no complete true witness ... member(s)
//! undecided`), then the bounded-loss declaration and the seal, then a KV read
//! that proves the namespace serves again. It needs `tools/s3gw` and esbuild;
//! without them it skips, like the other laboratory tests. `CELLD_S3GW`
//! overrides the gateway path.

#![allow(clippy::disallowed_methods)]
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const NAMESPACE: &str = "example-values";
const KEY: &str = "c14-key";
const VALUE: &str = "c14-value";

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

fn body(port: u16, path: &str) -> String {
    http_get(port, path)
        .map(|raw| response_body(&raw))
        .unwrap_or_default()
}

/// One `celld kv` command against the fleet bucket.
fn kv(bucket: &str, endpoint: &str, arguments: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_celld"));
    command
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
        .stdin(Stdio::null());
    command.output().expect("run celld kv")
}

/// All the processes the test owns, so a panic cannot leak a node.
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
            // A takeover lease short enough for the test, and a waker tick
            // that retires a dead follower's record in test time.
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
        let child = command.spawn().expect("start a celld node");
        self.nodes[index] = Some(child);
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

/// The node lease record, parsed just enough to read the folded log.
fn node_record(root: &Path, node: &str) -> Option<serde_json::Value> {
    let bytes = std::fs::read(root.join("nodes").join(format!("{node}.json"))).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[test]
fn a_kv_namespace_naming_a_vanished_node_seals_without_manual_deletion() {
    let Some(s3gw) = s3gw_binary() else {
        eprintln!("skipping node-log churn test: tools/s3gw is not built");
        return;
    };
    if Command::new("esbuild").arg("--version").output().is_err() {
        eprintln!("skipping node-log churn test: esbuild is not on PATH");
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

    // The deployment: one Worker with a KV binding, so the runtime publishes
    // the reserved __KvNamespace class.
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

    // node-a first, in fleet posture. It creates and owns the namespace cell.
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

    // node-b joins in bucket posture. It serves as a follower, but it never
    // opens a log of its own: its lease carries `log: None`, and the dead-node
    // GC deletes that record instead of tombstoning it.
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

    // A second namespace write is now acknowledged by the follower, which
    // marks the session may_have_fleet_acks.
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

    // Retire node-b first. node-a stays alive and its dead-node GC deletes the
    // log-less record, so the ensemble now names a node the bucket does not know.
    if let Some(mut node_b) = stack.nodes[1].take() {
        kill(&mut node_b);
    }
    if !wait_for(Duration::from_secs(120), || {
        !bucket_root.join("nodes/node-b.json").exists()
    }) {
        panic!(
            "the dead-node GC did not retire node-b's record:\n{}",
            stack.log("node-a.log")
        );
    }
    assert!(
        !bucket_root.join("nodes/node-b.json").exists(),
        "node-b's lease record survived, so the reproduction shape is wrong"
    );

    // Now kill the owner. Its session still names node-b.
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

    // The namespace read takes the cell over. Recovery refuses first (the
    // member's absence is fresh), records it, and seals after the grace window.
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
        c_log.contains("no complete true witness among {\"node-b\"} and 1 member(s) undecided"),
        "the refusal was not exercised before the seal:\n{c_log}"
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
        c_log.contains("recorded the absence"),
        "recovery never recorded node-b's absence:\n{c_log}"
    );

    // The seal is durable.
    let record = node_record(&bucket_root, "node-a").expect("node-a's record");
    assert_eq!(
        record
            .get("log")
            .and_then(|log| log.get("state"))
            .and_then(serde_json::Value::as_str),
        Some("sealed"),
        "node-a's session is not sealed: {record}"
    );
    eprintln!("the KV namespace sealed node-a's session and served again");
}
