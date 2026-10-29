// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! Proves a frame sent from a Durable Object alarm reaches an already-open
//! WebSocket. The `executeCode` live path sends metadata/messages from alarms,
//! not only from `webSocketMessage`, so an output-gate regression here means
//! the browser never sees agent updates.

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

fn connect_websocket(port: u16) -> std::io::Result<TcpStream> {
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.write_all(
        b"GET / HTTP/1.1\r\n\
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

fn read_server_text_frame(stream: &mut TcpStream) -> std::io::Result<String> {
    loop {
        let mut header = [0u8; 2];
        stream.read_exact(&mut header)?;
        let opcode = header[0] & 0x0f;
        let masked = header[1] & 0x80 != 0;
        let mut len = (header[1] & 0x7f) as usize;
        if len == 126 {
            let mut extended = [0u8; 2];
            stream.read_exact(&mut extended)?;
            len = u16::from_be_bytes(extended) as usize;
        } else if len == 127 {
            let mut extended = [0u8; 8];
            stream.read_exact(&mut extended)?;
            len = u64::from_be_bytes(extended) as usize;
        }
        let mut mask = [0u8; 4];
        if masked {
            stream.read_exact(&mut mask)?;
        }
        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload)?;
        if masked {
            for (index, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[index % 4];
            }
        }
        match opcode {
            0x1 => return Ok(String::from_utf8_lossy(&payload).into_owned()),
            0x8 => {
                return Err(std::io::Error::other(
                    "server closed before sending the alarm frame",
                ))
            }
            _ => continue,
        }
    }
}

#[test]
fn alarm_frame_reaches_open_websocket() {
    if Command::new("esbuild").arg("--version").output().is_err() {
        eprintln!("skipping alarm WebSocket test: esbuild is not on PATH");
        return;
    }

    let project = tempfile::tempdir().expect("temporary project");
    std::fs::write(
        project.path().join("wrangler.jsonc"),
        include_str!("fixtures/alarm-websocket/wrangler.jsonc"),
    )
    .expect("write the wrangler config");
    std::fs::write(
        project.path().join("worker.js"),
        include_str!("fixtures/alarm-websocket/worker.js"),
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

    let result = (|| -> std::io::Result<String> {
        let mut stream = connect_websocket(port)?;
        read_server_text_frame(&mut stream)
    })();

    let _ = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    let _ = wait_for_exit(&mut child, Duration::from_secs(60));

    match result {
        Ok(frame) => assert_eq!(frame, "desde-alarm"),
        Err(error) => panic!(
            "alarm frame did not reach the open WebSocket: {error}\n{}",
            log_text()
        ),
    }
}
