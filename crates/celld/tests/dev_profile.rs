// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! Dev-level proof for `celld dev --profile`: start the supervisor, exercise a
//! Worker, stop it, and read a real V8 `.cpuprofile` from disk.

// This test drives a real host process and its filesystem on wall-clock time,
// which is exactly what the banned in-process helpers are for.
#![allow(clippy::disallowed_methods)]
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a free port")
        .local_addr()
        .expect("read the bound port")
        .port()
}

fn http_ok(port: u16) -> bool {
    let Ok(address) = format!("127.0.0.1:{port}").parse() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    if stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buffer = [0_u8; 64];
    match stream.read(&mut buffer) {
        Ok(read) => buffer[..read].starts_with(b"HTTP/1.1 200"),
        Err(_) => false,
    }
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

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => return None,
        }
    }
    let _ = child.kill();
    None
}

fn profile_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "cpuprofile")
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[test]
fn dev_writes_a_non_empty_cpu_profile() {
    if Command::new("esbuild").arg("--version").output().is_err() {
        eprintln!("skipping dev profile test: esbuild is not on PATH");
        return;
    }

    let project = tempfile::tempdir().expect("temporary project");
    let profiles = project.path().join("profiles");
    std::fs::write(
        project.path().join("wrangler.jsonc"),
        r#"{
            "name": "profile-fixture",
            "main": "worker.js",
            "compatibility_date": "2026-09-01"
        }"#,
    )
    .expect("write the wrangler config");
    std::fs::write(
        project.path().join("worker.js"),
        r#"
            function spin(n) {
                let x = 0;
                for (let i = 0; i < n; i++) x = (x + i * 7) % 1000003;
                return x;
            }
            spin(20_000_000);
            export default {
                fetch() {
                    spin(20_000_000);
                    return new Response("ok");
                }
            };
        "#,
    )
    .expect("write the worker");

    let port = free_port();
    let internal = free_port();
    let log = std::fs::File::create(project.path().join("dev.log")).expect("create the log");
    let err = log.try_clone().expect("clone the log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_celld"))
        .arg("dev")
        .arg(project.path())
        .arg("--profile")
        .arg(&profiles)
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

    let log_text = || {
        std::fs::read_to_string(project.path().join("dev.log")).unwrap_or_else(|_| String::new())
    };

    if !wait_for(Duration::from_secs(90), || http_ok(port)) {
        let _ = child.kill();
        panic!("celld dev never served a request:\n{}", log_text());
    }
    for _ in 0..3 {
        assert!(http_ok(port), "the Worker stopped answering");
    }

    // SIGINT is what an operator presses. The supervisor turns it into the
    // graceful same-node shutdown, where profiles are stopped and written.
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }
    let status = wait_for_exit(&mut child, Duration::from_secs(90));
    assert!(
        status.is_some(),
        "celld dev did not exit after SIGINT:\n{}",
        log_text()
    );

    let files = profile_files(&profiles);
    assert!(
        !files.is_empty(),
        "no .cpuprofile was written to {}:\n{}",
        profiles.display(),
        log_text()
    );

    let mut sampled = 0;
    let mut named_spin = false;
    for file in &files {
        let bytes = std::fs::read(file).expect("read the profile");
        let profile: serde_json::Value = serde_json::from_slice(&bytes).expect("profile is JSON");
        let nodes = profile["nodes"].as_array().map(Vec::len).unwrap_or(0);
        let samples = profile["samples"].as_array().map(Vec::len).unwrap_or(0);
        assert!(nodes > 0, "{} has no nodes", file.display());
        sampled = sampled.max(samples);
        named_spin |= profile["nodes"].as_array().is_some_and(|nodes| {
            nodes
                .iter()
                .any(|node| node["callFrame"]["functionName"] == "spin")
        });
    }
    assert!(
        sampled > 0,
        "profiles contain no samples: {:?}",
        files
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
    );
    assert!(
        named_spin,
        "the profile does not name the hot JavaScript frame: {:?}",
        files
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
    );
    eprintln!(
        "dev profile test: {} file(s), {sampled} samples, named spin: {named_spin}",
        files.len()
    );
}
