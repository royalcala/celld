// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! Proves a fenced transient capability can be written down and revived.
//!
//! A Durable Object cannot store the stub its session handed it: the entry id
//! dies with that isolate, so storage refuses it (asserted here, and in
//! `retained_callback.rs`). What survives is the token -- signed by the fleet
//! key, naming the owner's node, isolate incarnation and dial address -- so an
//! object stores the string and revives a handle later. This test drives both
//! halves plus every refusal that keeps an unfenced handle unstorable.

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
    let _ =
        stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    response.starts_with(b"HTTP/1.")
}

fn http_get(port: u16, path: &str) -> std::io::Result<String> {
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(20)))?;
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

fn connect_websocket(port: u16, path: &str) -> std::io::Result<TcpStream> {
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(20)))?;
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
    stream.set_read_timeout(Some(Duration::from_secs(20)))?;
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
fn a_fenced_session_capability_survives_as_a_token() {
    if Command::new("esbuild").arg("--version").output().is_err() {
        eprintln!("skipping persisted-capability test: esbuild is not on PATH");
        return;
    }

    let project = tempfile::tempdir().expect("temporary project");
    std::fs::write(
        project.path().join("wrangler.jsonc"),
        include_str!("fixtures/persisted-capability/wrangler.jsonc"),
    )
    .expect("write the wrangler config");
    std::fs::write(
        project.path().join("worker.js"),
        include_str!("fixtures/persisted-capability/worker.js"),
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

    if !wait_for(Duration::from_secs(90), || http_ready(port)) {
        let _ = child.kill();
        panic!("celld dev never became ready:\n{}", log_text());
    }

    let result = (|| -> std::io::Result<Vec<(String, String)>> {
        let mut verdicts = Vec::new();
        // The open socket pins the isolate that owns the session capability, as
        // a browser tab does.
        let mut socket = connect_websocket(port, "/session")?;
        let armed = ws_client_text(&mut socket, "arm")?;
        verdicts.push(("arm".to_owned(), armed));
        verdicts.push((
            "stored".to_owned(),
            response_body(&http_get(port, "/stored")?),
        ));
        verdicts.push((
            "persist".to_owned(),
            response_body(&http_get(port, "/persist")?),
        ));
        verdicts.push((
            "forged".to_owned(),
            response_body(&http_get(port, "/forged")?),
        ));
        verdicts.push((
            "not-a-capability".to_owned(),
            response_body(&http_get(port, "/not-a-capability")?),
        ));
        verdicts.push((
            "durable".to_owned(),
            response_body(&http_get(port, "/durable")?),
        ));
        // The object holds no stub: this delivery can only use the stored token.
        let delivered = ws_client_text(&mut socket, "deliver")?;
        verdicts.push(("deliver".to_owned(), delivered));
        Ok(verdicts)
    })();

    let _ = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    let _ = wait_for_exit(&mut child, Duration::from_secs(60));

    let verdicts = match result {
        Ok(verdicts) => verdicts,
        Err(error) => panic!(
            "the persisted-capability fixture did not answer: {error}\n{}",
            log_text()
        ),
    };
    let verdict = |name: &str| {
        verdicts
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };

    for (name, value) in &verdicts {
        eprintln!("{name} = {value}");
    }

    assert!(
        verdict("arm").starts_with("armed:string:"),
        "capabilityToken() did not return a string token: {:?}",
        verdict("arm")
    );
    // A string is ordinary storage: no stub envelope, no durable identity.
    assert!(
        verdict("stored").starts_with("string:"),
        "the stored token did not read back as a string: {:?}",
        verdict("stored")
    );
    // The stub itself is still refused: the rule for handles without a fence is
    // unchanged, and a token is the only way across.
    assert!(
        verdict("persist").starts_with("refused:")
            && verdict("persist").contains("only store stubs with durable identity"),
        "storing the transient stub was not refused: {:?}",
        verdict("persist")
    );
    // Reviving verifies the MAC before it builds a handle.
    assert!(
        verdict("forged").starts_with("err:forbidden:"),
        "a forged token revived instead of failing typed: {:?}",
        verdict("forged")
    );
    assert!(
        verdict("not-a-capability").starts_with("err:capabilityToken() takes a capability"),
        "capabilityToken() accepted a plain object: {:?}",
        verdict("not-a-capability")
    );
    assert!(
        verdict("durable").starts_with("err:capabilityToken() takes a transient capability"),
        "capabilityToken() accepted a durable restore handle: {:?}",
        verdict("durable")
    );
    // The revive itself: a handle rebuilt from the stored token reaches the
    // session target that no stub in the object points at any more.
    assert_eq!(
        verdict("deliver"),
        "delivered:session:alarm",
        "the revived capability did not reach the session\n{}",
        log_text()
    );
}
