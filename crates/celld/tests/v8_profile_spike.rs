//! P3 spike evidence: the pinned `v8` crate can produce a real V8 CPU profile
//! through the Inspector protocol (`Profiler.enable/start/stop`) without
//! patching or upgrading the binding. This is a research prototype, not a
//! product path.

use std::cell::RefCell;
use std::rc::Rc;

use celld::js::Engine;
use v8::inspector::{
    Channel, ChannelImpl, StringBuffer, StringView, V8Inspector, V8InspectorClient,
    V8InspectorClientImpl, V8InspectorClientTrustLevel,
};

#[derive(Default)]
struct Captured {
    responses: Vec<(i32, String)>,
    notifications: Vec<String>,
}

#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Captured>>);

impl ChannelImpl for Capture {
    fn send_response(&self, call_id: i32, message: v8::UniquePtr<StringBuffer>) {
        let message = message.unwrap().string().to_string();
        self.0.borrow_mut().responses.push((call_id, message));
    }

    fn send_notification(&self, message: v8::UniquePtr<StringBuffer>) {
        self.0
            .borrow_mut()
            .notifications
            .push(message.unwrap().string().to_string());
    }

    fn flush_protocol_notifications(&self) {}
}

struct Client;

impl V8InspectorClientImpl for Client {}

fn dispatch(session: &v8::inspector::V8InspectorSession, message: &str) {
    session.dispatch_protocol_message(StringView::from(message.as_bytes()));
}

#[test]
fn inspector_produces_a_real_v8_cpu_profile() {
    Engine::init();

    let mut isolate = v8::Isolate::new(v8::CreateParams::default());
    let inspector = V8Inspector::create(&mut isolate, V8InspectorClient::new(Box::new(Client)));

    v8::scope!(let scope, &mut isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);

    let empty = StringView::from(&b""[..]);
    inspector.context_created(context, 1, empty, empty);

    let capture = Capture::default();
    let session = inspector.connect(
        1,
        Channel::new(Box::new(capture.clone())),
        StringView::from(&b"{}"[..]),
        V8InspectorClientTrustLevel::Untrusted,
    );

    dispatch(&session, r#"{"id":1,"method":"Profiler.enable"}"#);
    dispatch(
        &session,
        r#"{"id":2,"method":"Profiler.start","params":{"samplingInterval":100}}"#,
    );

    let source = v8::String::new(
        scope,
        r#"
            function spin(n) {
              let x = 0;
              for (let i = 0; i < n; i++) x = (x + i * 7) % 1000003;
              return x;
            }
            spin(8_000_000);
        "#,
    )
    .unwrap();
    let script = v8::Script::compile(scope, source, None).unwrap();
    script.run(scope).unwrap();

    dispatch(&session, r#"{"id":3,"method":"Profiler.stop"}"#);

    let captured = capture.0.borrow();
    let stop = captured
        .responses
        .iter()
        .find(|(id, _)| *id == 3)
        .expect("Profiler.stop response");
    let message: serde_json::Value = serde_json::from_str(&stop.1).expect("Profiler.stop JSON");
    let profile = &message["result"]["profile"];
    let nodes = profile["nodes"].as_array().expect("profile nodes");
    let samples = profile["samples"].as_array().expect("profile samples");
    assert!(
        nodes
            .iter()
            .any(|node| node["callFrame"]["functionName"] == "spin"),
        "profile should name the hot function"
    );
    assert!(!samples.is_empty(), "profile should contain samples");

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/spikes/v8-profile-spike.cpuprofile");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, serde_json::to_vec_pretty(profile).unwrap()).unwrap();

    eprintln!(
        "P3 profile: {} nodes, {} samples, {} bytes -> {}",
        nodes.len(),
        samples.len(),
        stop.1.len(),
        out.display()
    );

    inspector.context_destroyed(context);
}
