// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! Proves Dynamic Worker RPC tails reach the caller: the fixture loads a
//! Worker, calls `verify()` and `run()` over RPC, waits for the Tail Worker's
//! event, and returns the logs. This is the `executeCode` shape that used to
//! time out because only `fetch` delivered tails.

// This test drives a real host process and its filesystem on wall-clock time.
#![allow(clippy::disallowed_methods)]
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a free port")
        .local_addr()
        .expect("read the bound port")
        .port()
}

fn decode_chunked(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut offset = 0;
    while offset < body.len() {
        let Some(line_end) = body[offset..].iter().position(|byte| *byte == b'\r') else {
            break;
        };
        let line_end = offset + line_end;
        let size = std::str::from_utf8(&body[offset..line_end])
            .unwrap_or("0")
            .split(';')
            .next()
            .unwrap_or("0")
            .trim();
        let Ok(size) = usize::from_str_radix(size, 16) else {
            break;
        };
        if size == 0 {
            break;
        }
        let start = line_end + 2;
        let end = (start + size).min(body.len());
        out.extend_from_slice(&body[start..end]);
        offset = end + 2;
    }
    out
}

fn http_get(port: u16, path: &str) -> Option<(u16, Vec<u8>)> {
    let address = format!("127.0.0.1:{port}").parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(500)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .ok()?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).ok()?;
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&response[..split]).to_ascii_lowercase();
    let status = head
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let body = &response[split + 4..];
    let body = if head.contains("transfer-encoding: chunked") {
        decode_chunked(body)
    } else {
        body.to_vec()
    };
    Some((status, body))
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

#[test]
fn rpc_tail_logs_reach_the_caller() {
    if Command::new("esbuild").arg("--version").output().is_err() {
        eprintln!("skipping RPC tail test: esbuild is not on PATH");
        return;
    }

    let project = tempfile::tempdir().expect("temporary project");
    std::fs::write(
        project.path().join("wrangler.jsonc"),
        include_str!("fixtures/execute-code-tails/wrangler.jsonc"),
    )
    .expect("write the wrangler config");
    std::fs::write(
        project.path().join("worker.js"),
        include_str!("fixtures/execute-code-tails/worker.js"),
    )
    .expect("write the worker");

    let port = free_port();
    let internal = free_port();
    let log_path = project.path().join("dev.log");
    let log = std::fs::File::create(&log_path).expect("create the log");
    let err = log.try_clone().expect("clone the log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_celld"))
        .arg("dev")
        .arg(project.path())
        .arg("--port")
        .arg(port.to_string())
        .arg("--internal-port")
        .arg(internal.to_string())
        .arg("--no-watch")
        .arg("--clean")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err))
        .spawn()
        .expect("start celld dev");

    let log_text = || std::fs::read_to_string(&log_path).unwrap_or_default();

    if !wait_for(Duration::from_secs(90), || {
        http_get(port, "/health").is_some_and(|(status, _)| status == 200)
    }) {
        let _ = child.kill();
        panic!("celld dev never became ready:\n{}", log_text());
    }

    let (status, body) = http_get(port, "/run").expect("call /run");
    assert_eq!(status, 200, "{}", log_text());
    let response: serde_json::Value =
        serde_json::from_slice(&body).expect("the fixture answers JSON");

    let _ = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    let _ = wait_for_exit(&mut child, Duration::from_secs(60));

    assert_eq!(
        response["timedOut"],
        false,
        "the caller timed out waiting for tails:\n{}",
        log_text()
    );
    assert_eq!(response["result"], "run-result");

    let events = response["events"].as_array().expect("events array");
    let run = events
        .iter()
        .find(|event| event["event"]["rpcMethod"] == "run")
        .unwrap_or_else(|| panic!("no run tail event in {events:?}"));
    assert!(
        events
            .iter()
            .any(|event| event["event"]["rpcMethod"] == "verify"),
        "the verify tail event should also be reported: {events:?}"
    );
    assert_eq!(run["outcome"], "ok");

    let logs = run["logs"].as_array().expect("run logs");
    let joined = logs
        .iter()
        .map(|log| {
            log["message"]
                .as_array()
                .map(|parts| {
                    parts
                        .iter()
                        .map(|part| {
                            part.as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| part.to_string())
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("hello from executeCode 42"),
        "run logs missing the console line: {joined:?}"
    );
    assert!(
        joined.contains("{\"nested\":true}"),
        "run logs missing the object line: {joined:?}"
    );

    eprintln!(
        "RPC tail test: rpcMethod=run, {} log line(s), no timeout",
        logs.len()
    );
}
