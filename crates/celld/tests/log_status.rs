// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! `celld log status` reads a fleet bucket without a running node and reports
//! every open session's ensemble members: live, expired or absent. This test
//! builds that bucket by hand on us-s3gw, so the output shape is pinned
//! independently of any node process. It skips without `tools/s3gw`, like the
//! other laboratory tests; `CELLD_S3GW` overrides the gateway path.

#![allow(clippy::disallowed_methods)]
#![cfg(unix)]

use std::io::Write;
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

fn write_record(root: &Path, node: &str, body: &str) {
    let dir = root.join("nodes");
    std::fs::create_dir_all(&dir).expect("create nodes/");
    let mut file = std::fs::File::create(dir.join(format!("{node}.json"))).expect("create record");
    file.write_all(body.as_bytes()).expect("write record");
}

struct Gateway {
    child: Child,
}

impl Drop for Gateway {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn status_reports_live_expired_absent_and_skips_sealed_sessions() {
    let Some(s3gw) = s3gw_binary() else {
        eprintln!("skipping log status test: tools/s3gw is not built");
        return;
    };
    let root = tempfile::tempdir().expect("temporary root");
    let bucket = "status-check";
    let bucket_root = root.path().join(bucket);
    std::fs::create_dir_all(&bucket_root).expect("create the bucket");

    let now = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX);
    let expired = now.saturating_sub(7_000);
    let live = now + 60_000;

    // The leader's open session names three members: one with no record (and a
    // dated absence), one live and one expired. A sealed session is present to
    // prove the reader ignores it.
    write_record(
        &bucket_root,
        "node-a",
        &format!(
            r#"{{"node":"node-a","expires_ms":{expired},"addr":"127.0.0.1:1000","ownership_index_generation":"gen","log":{{"state":"open","epoch":1,"ensemble":["node-b","node-c","node-d"],"tiered":0,"active":true}}}}"#
        ),
    );
    write_record(
        &bucket_root,
        "node-c",
        &format!(
            r#"{{"node":"node-c","expires_ms":{live},"addr":"127.0.0.1:1001","ownership_index_generation":"g2"}}"#
        ),
    );
    write_record(
        &bucket_root,
        "node-d",
        &format!(
            r#"{{"node":"node-d","expires_ms":{expired},"addr":"127.0.0.1:1002","ownership_index_generation":"g3"}}"#
        ),
    );
    write_record(
        &bucket_root,
        "node-e",
        &format!(
            r#"{{"node":"node-e","expires_ms":{expired},"addr":"127.0.0.1:1003","ownership_index_generation":"g4","log":{{"state":"sealed","epoch":2,"ensemble":["node-z"],"tiered":0,"active":true}}}}"#
        ),
    );
    let marker = bucket_root.join("log/node-a/gen/absent/e1");
    std::fs::create_dir_all(&marker).expect("create the absence marker directory");
    std::fs::write(
        marker.join("node-b.json"),
        format!(r#"{{"first_seen_ms":{}}}"#, now.saturating_sub(5_000)),
    )
    .expect("write the absence marker");

    let port = free_port();
    let log = std::fs::File::create(root.path().join("s3gw.log")).expect("create the gateway log");
    let err = log.try_clone().expect("clone the gateway log");
    let child = Command::new(&s3gw)
        .args([
            "--host".to_owned(),
            "127.0.0.1".to_owned(),
            "--port".to_owned(),
            port.to_string(),
            "--access-key".to_owned(),
            "minioadmin".to_owned(),
            "--secret-key".to_owned(),
            "minioadmin".to_owned(),
            root.path().to_string_lossy().into_owned(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err))
        .spawn()
        .expect("start us-s3gw");
    let _gateway = Gateway { child };
    if !wait_for(Duration::from_secs(30), || {
        TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_millis(300),
        )
        .is_ok()
    }) {
        panic!("us-s3gw never listened");
    }

    let endpoint = format!("http://127.0.0.1:{port}");
    let status = Command::new(env!("CARGO_BIN_EXE_celld"))
        .args([
            "log",
            "status",
            "--bucket",
            &format!("s3://{bucket}"),
            "--endpoint",
            &endpoint,
        ])
        .env("AWS_ACCESS_KEY_ID", "minioadmin")
        .env("AWS_SECRET_ACCESS_KEY", "minioadmin")
        .env("AWS_REGION", "us-east-1")
        .stdin(Stdio::null())
        .output()
        .expect("run celld log status");
    assert!(
        status.status.success(),
        "log status failed: {}\n{}",
        String::from_utf8_lossy(&status.stderr),
        String::from_utf8_lossy(&status.stdout)
    );
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains("node-a/gen"), "session missing:\n{text}");
    assert!(text.contains("node-b") && text.contains("absent"), "{text}");
    assert!(text.contains("node-c") && text.contains("live"), "{text}");
    assert!(
        text.contains("node-d") && text.contains("expired"),
        "{text}"
    );
    assert!(
        !text.contains("node-e"),
        "a sealed session must not appear:\n{text}"
    );
    assert!(text.contains("yes"), "fleet-ack marker missing:\n{text}");

    let json = Command::new(env!("CARGO_BIN_EXE_celld"))
        .args([
            "log",
            "status",
            "--bucket",
            &format!("s3://{bucket}"),
            "--endpoint",
            &endpoint,
            "--json",
        ])
        .env("AWS_ACCESS_KEY_ID", "minioadmin")
        .env("AWS_SECRET_ACCESS_KEY", "minioadmin")
        .env("AWS_REGION", "us-east-1")
        .stdin(Stdio::null())
        .output()
        .expect("run celld log status --json");
    assert!(json.status.success());
    let lines = String::from_utf8_lossy(&json.stdout)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a JSON object"))
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 3, "one object per member");
    let leases = lines
        .iter()
        .filter_map(|line| line.get("lease").and_then(serde_json::Value::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        leases,
        ["absent", "expired", "live"].into_iter().collect(),
        "{lines:?}"
    );
    for line in &lines {
        assert_eq!(
            line.get("session").and_then(serde_json::Value::as_str),
            Some("node-a/gen")
        );
        assert_eq!(
            line.get("may_have_fleet_acks")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }
}
