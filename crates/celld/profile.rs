// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! Opt-in V8 CPU profiles for `celld dev`.
//!
//! The pinned `v8` crate has no direct `CpuProfiler` binding, but it exposes
//! the Inspector protocol, whose `Profiler` domain produces a real
//! `.cpuprofile`. This module owns the inspector session for one isolate and
//! writes the profile when the isolate is stopped.
//!
//! Disabled unless [`configure`] ran in this process. Only `celld dev` calls
//! it, and its child node requires the local development store, so a normal
//! `celld` node never profiles. Profiles stay on the local filesystem.

// Profiles are operator-owned local development files, not node storage.
#![allow(clippy::disallowed_methods)]

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use v8::inspector::{
    Channel, ChannelImpl, StringBuffer, StringView, V8Inspector, V8InspectorClient,
    V8InspectorClientImpl, V8InspectorClientTrustLevel, V8InspectorSession,
};

/// V8's default sampling period. One sample per millisecond is enough to find
/// hot JavaScript frames without the tighter 100 µs interval the spike used.
const SAMPLING_INTERVAL_US: u32 = 1_000;

static PROFILE_DIR: OnceLock<PathBuf> = OnceLock::new();
static NEXT_PROFILE: AtomicU64 = AtomicU64::new(1);

/// Point every isolate created after this call at `dir`.
///
/// The process sets this once, before the engine loads a Worker. Repeating it
/// is a no-op so a reload after a failed start does not abort.
pub fn configure(dir: PathBuf) -> std::io::Result<()> {
    std::fs::create_dir_all(&dir)?;
    let _ = PROFILE_DIR.set(dir);
    Ok(())
}

pub fn enabled() -> bool {
    PROFILE_DIR.get().is_some()
}

#[derive(Default)]
struct Captured {
    responses: Vec<(i32, String)>,
}

/// The inspector channel's responses for one session. The protocol calls the
/// channel synchronously from `dispatch_protocol_message`, so the stop reply
/// is already here when the dispatch returns.
#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Captured>>);

impl ChannelImpl for Capture {
    fn send_response(&self, call_id: i32, message: v8::UniquePtr<StringBuffer>) {
        let message = message.unwrap().string().to_string();
        self.0.borrow_mut().responses.push((call_id, message));
    }

    fn send_notification(&self, message: v8::UniquePtr<StringBuffer>) {
        // Nothing in the profile path needs notifications. Consume the buffer
        // so its C++ allocation is released.
        drop(message);
    }

    fn flush_protocol_notifications(&self) {}
}

struct Client;

impl V8InspectorClientImpl for Client {}

fn dispatch(session: &V8InspectorSession, message: &str) {
    session.dispatch_protocol_message(StringView::from(message.as_bytes()));
}

/// One isolate's profiler.
///
/// The inspector and session use raw pointers into the isolate and `Rc` state,
/// so they are not `Send` on their own. A `WorkerIsolate` moves between tokio
/// threads; every V8 access is serialized by its `v8::Locker`, and this value
/// is only touched while that lock is held, so it is safe to carry.
pub(crate) struct Session {
    path: PathBuf,
    capture: Capture,
    inspector: V8Inspector,
    session: Option<V8InspectorSession>,
    running: bool,
}

unsafe impl Send for Session {}

impl Session {
    /// Create the inspector for a freshly-created isolate. Returns `None`
    /// when profiling is off, so the default path allocates nothing.
    pub(crate) fn new(isolate: &mut v8::Isolate) -> Option<Self> {
        let dir = PROFILE_DIR.get()?.clone();
        // Without this, optimized functions are sampled as `(program)` and the
        // profile cannot name the hot JavaScript frame. It is the crate's
        // existing switch for exactly this case.
        isolate.use_detailed_source_positions_for_profiling();
        let id = NEXT_PROFILE.fetch_add(1, Ordering::Relaxed);
        let capture = Capture::default();
        let inspector = V8Inspector::create(isolate, V8InspectorClient::new(Box::new(Client)));
        Some(Self {
            // The sequence restarts with each node, and `celld dev` starts a
            // new node on reload; the pid keeps the old generation's file.
            path: dir.join(format!("slot-{id:04}-{}.cpuprofile", std::process::id())),
            capture,
            inspector,
            session: None,
            running: false,
        })
    }

    /// Announce the isolate's context and start sampling. Called from the
    /// context scope, on the isolate's thread, once the context exists.
    pub(crate) fn attach(&mut self, context: v8::Local<v8::Context>) {
        let empty = StringView::from(&b""[..]);
        self.inspector.context_created(context, 1, empty, empty);
        let session = self.inspector.connect(
            1,
            Channel::new(Box::new(self.capture.clone())),
            StringView::from(&b"{}"[..]),
            V8InspectorClientTrustLevel::Untrusted,
        );
        dispatch(&session, r#"{"id":1,"method":"Profiler.enable"}"#);
        dispatch(
            &session,
            &format!(
                r#"{{"id":2,"method":"Profiler.start","params":{{"samplingInterval":{SAMPLING_INTERVAL_US}}}}}"#
            ),
        );
        self.session = Some(session);
        self.running = true;
    }

    /// Stop sampling, write the profile, and detach the context. Idempotent:
    /// shutdown may have stopped it before the worker is dropped.
    pub(crate) fn stop(&mut self, context: Option<v8::Local<v8::Context>>) {
        if self.running {
            self.running = false;
            if let Some(session) = &self.session {
                dispatch(session, r#"{"id":3,"method":"Profiler.stop"}"#);
                self.write_profile();
            }
        }
        if let Some(context) = context {
            self.inspector.context_destroyed(context);
        }
        self.session.take();
    }

    fn write_profile(&self) {
        let response = self
            .capture
            .0
            .borrow()
            .responses
            .iter()
            .rev()
            .find(|(id, _)| *id == 3)
            .map(|(_, body)| body.clone());
        let Some(body) = response else {
            eprintln!("celld profile: Profiler.stop produced no response");
            return;
        };
        let Ok(message) = serde_json::from_str::<serde_json::Value>(&body) else {
            eprintln!("celld profile: Profiler.stop returned invalid JSON");
            return;
        };
        let Some(profile) = message
            .get("result")
            .and_then(|result| result.get("profile"))
        else {
            eprintln!("celld profile: Profiler.stop returned no profile");
            return;
        };
        let bytes = match serde_json::to_vec(profile) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("celld profile: could not encode the profile: {error}");
                return;
            }
        };
        match std::fs::write(&self.path, bytes) {
            Ok(()) => eprintln!("celld profile: {}", self.path.display()),
            Err(error) => eprintln!(
                "celld profile: could not write {}: {error}",
                self.path.display()
            ),
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // The session holds pointers into the isolate; it must go before the
        // inspector and before the isolate itself.
        self.session.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_names_are_per_isolate() {
        let first = NEXT_PROFILE.fetch_add(1, Ordering::Relaxed);
        let second = NEXT_PROFILE.fetch_add(1, Ordering::Relaxed);
        assert_ne!(first, second);
    }
}
