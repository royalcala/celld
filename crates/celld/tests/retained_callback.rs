// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! Proves a callback kept past the call that delivered it survives the caller releasing its own
//! copy. An RPC system layered above celld (Cap'n Web) disposes the callee's copy of a parameter
//! stub when the call completes; a receiver that keeps it calls `dup()`. That `dup()` must reach
//! the stored target, not only celld's reference table, or the first later call finds a released
//! stub. The fixture models the release the way Cap'n Web performs it.

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

fn http_ready(port: u16) -> bool {
    let Ok(address) = format!("127.0.0.1:{port}").parse() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(2_000)));
    let _ = stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    response.starts_with(b"HTTP/1.")
}

fn http_get(port: u16, path: &str) -> std::io::Result<String> {
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
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

fn connect_websocket(port: u16) -> std::io::Result<TcpStream> {
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    stream.write_all(
        b"GET /socket HTTP/1.1\r\n\
          Host: localhost\r\n\
          Upgrade: websocket\r\n\
          Connection: Upgrade\r\n\
          Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
          Sec-WebSocket-Version: 13\r\n\
          \r\n",
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

/// Sends one masked text frame and returns the first text frame the server sends back.
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
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut header = [0u8; 2];
    stream.read_exact(&mut header)?;
    let mut length = (header[1] & 0x7f) as usize;
    if length == 126 {
        let mut extended = [0u8; 2];
        stream.read_exact(&mut extended)?;
        length = u16::from_be_bytes(extended) as usize;
    }
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body)?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[test]
fn retained_callback_survives_the_callers_release() {
    if Command::new("esbuild").arg("--version").output().is_err() {
        eprintln!("skipping retained-callback test: esbuild is not on PATH");
        return;
    }

    let project = tempfile::tempdir().expect("temporary project");
    std::fs::write(
        project.path().join("wrangler.jsonc"),
        include_str!("fixtures/retained-callback/wrangler.jsonc"),
    )
    .expect("write the wrangler config");
    std::fs::write(
        project.path().join("worker.js"),
        include_str!("fixtures/retained-callback/worker.js"),
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
        .arg("--logs")
        .arg("--clean")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err))
        .spawn()
        .expect("start celld dev");

    let log_text = || std::fs::read_to_string(&log_path).unwrap_or_default();

    if !wait_for(Duration::from_secs(90), || http_ready(port)) {
        let _ = child.kill();
        panic!("celld dev never became ready:\n{}", log_text());
    }

    let result = (|| -> std::io::Result<String> {
        // The open socket keeps the isolate that owns the module state, as a client's session does.
        let _stream = connect_websocket(port)?;
        let started = http_get(port, "/start")?;
        if !started.starts_with("HTTP/1.1 200") {
            return Err(std::io::Error::other(format!(
                "start failed: {}",
                started.lines().next().unwrap_or_default()
            )));
        }
        // A returned RpcTarget must answer its prototype methods.
        let lease = response_body(&http_get(port, "/lease")?);
        if lease != "true" {
            return Err(std::io::Error::other(format!(
                "returned RpcTarget method failed: {lease:?}")));
        }
        // A receiver that calls the callback *inside* the call that delivered it must reach it:
        // a subscription's streamGeneration and replay happen that way.
        // A session capability has no durable identity: the runtime refuses to persist it, so a
        // Durable Object cannot keep a subscriber across a restore by storing the stub.
        let persisted = response_body(&http_get(port, "/persist")?);
        if !(persisted.starts_with("refused:")
            && persisted.contains("only store stubs with durable identity"))
        {
            return Err(std::io::Error::other(format!(
                "session capability storage was not refused as expected: {persisted}")));
        }

        let during = response_body(&http_get(port, "/during")?);
        if during != "during:during" {
            return Err(std::io::Error::other(format!(
                "re-entrant callback failed from a fetch: {during:?}")));
        }
        // Same, from a WebSocket message handler that is awaiting the object's reply -- the shape a
        // session's subscribe call takes.
        let mut socket = connect_websocket(port)?;
        let frame = ws_client_text(&mut socket, "during")?;
        if frame != "result:during:during" {
            return Err(std::io::Error::other(format!(
                "re-entrant callback failed from a websocket message: {frame:?}")));
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut outcome = String::new();
        while Instant::now() < deadline {
            outcome = response_body(&http_get(port, "/result")?);
            if !outcome.is_empty() && outcome != "null" {
                return Ok(outcome);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(std::io::Error::other(format!(
            "the retained callback produced no outcome; last response: {outcome:?}"
        )))
    })();

    let _ = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    let _ = wait_for_exit(&mut child, Duration::from_secs(60));

    match result {
        Ok(outcome) => assert_eq!(outcome, "ok", "the retained callback failed"),
        Err(error) => panic!(
            "the retained callback did not report: {error}\n{}",
            log_text()
        ),
    }
}
