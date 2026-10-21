//! Host-side observability: bounded trace spans and a minimal metrics registry.
//!
//! Everything here is deliberately low-cardinality and redacted: names and
//! attribute values pass through the fleet redactor, values are truncated, and
//! the metrics registry only accepts the names/labels the runtime itself
//! emits. Nothing in this module may carry a request/response body, a header
//! set, a token or a secret.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Monotonic microseconds for spans/timers. Production uses the execution
/// domain clock; unit tests have no host domain, so they get a fake ticking
/// clock (the assertions only check sample counts, not wall time).
fn mono_us() -> u64 {
    #[cfg(not(test))]
    {
        crate::asyncrt::mono_us()
    }
    #[cfg(test)]
    {
        static CLOCK: AtomicU64 = AtomicU64::new(0);
        CLOCK.fetch_add(1_000, Ordering::Relaxed)
    }
}

/// Longest trace span name retained.
pub const MAX_SPAN_NAME: usize = 128;
/// Longest request id retained on a span.
pub const MAX_REQUEST_ID: usize = 128;
/// Attributes kept per span.
pub const MAX_SPAN_ATTRIBUTES: usize = 16;
/// Longest attribute key.
pub const MAX_ATTRIBUTE_KEY: usize = 64;
/// Longest attribute value.
pub const MAX_ATTRIBUTE_VALUE: usize = 256;
/// Live spans retained by the host. A span the JS side never ends is dropped
/// once this cap is reached, so a buggy Worker cannot grow the map without
/// bound.
pub const MAX_LIVE_SPANS: usize = 4096;

fn sanitize(value: &str, limit: usize) -> String {
    let redacted = crate::secrets::redact(value);
    if redacted.len() <= limit {
        return redacted;
    }
    let mut end = limit;
    while !redacted.is_char_boundary(end) {
        end -= 1;
    }
    redacted[..end].to_owned()
}

fn sanitize_label(value: &str) -> String {
    let value = sanitize(value, 80);
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MetricKey {
    name: &'static str,
    labels: Vec<(&'static str, String)>,
}

#[derive(Default)]
struct Metrics {
    counters: Mutex<BTreeMap<MetricKey, u64>>,
    durations: Mutex<BTreeMap<MetricKey, (u64, u64)>>,
}

impl Metrics {
    fn counter(&self, name: &'static str, labels: &[(&'static str, &str)], by: u64) {
        let key = MetricKey {
            name,
            labels: labels
                .iter()
                .map(|(key, value)| (*key, sanitize_label(value)))
                .collect(),
        };
        *self.counters.lock().unwrap().entry(key).or_default() += by;
    }

    fn duration(&self, name: &'static str, labels: &[(&'static str, &str)], seconds: f64) {
        let key = MetricKey {
            name,
            labels: labels
                .iter()
                .map(|(key, value)| (*key, sanitize_label(value)))
                .collect(),
        };
        let nanos = (seconds.max(0.0) * 1e9) as u64;
        let mut durations = self.durations.lock().unwrap();
        let entry = durations.entry(key).or_default();
        entry.0 = entry.0.saturating_add(nanos);
        entry.1 = entry.1.saturating_add(1);
    }

    fn render(&self) -> String {
        let counters = self.counters.lock().unwrap();
        let durations = self.durations.lock().unwrap();
        let mut seen = std::collections::BTreeSet::new();
        let mut out = String::new();
        for (key, value) in counters.iter() {
            if seen.insert((key.name, false)) {
                out.push_str(&format!(
                    "# HELP {} celld runtime counter\n# TYPE {} counter\n",
                    key.name, key.name
                ));
            }
            out.push_str(&render_sample(key.name, &key.labels));
            out.push(' ');
            out.push_str(&value.to_string());
            out.push('\n');
        }
        for (key, (nanos, count)) in durations.iter() {
            let base = key.name.strip_suffix("_seconds").unwrap_or(key.name);
            if seen.insert((base, true)) {
                // Only one summary pair per base name; HELP/TYPE once.
                let _ = base;
            }
            out.push_str(&render_sample(
                &format!("{}_seconds_sum", base),
                &key.labels,
            ));
            out.push_str(&format!(" {:.6}\n", *nanos as f64 / 1e9));
            out.push_str(&render_sample(
                &format!("{}_seconds_count", base),
                &key.labels,
            ));
            out.push_str(&format!(" {}\n", count));
        }
        out
    }
}

fn render_sample(name: &str, labels: &[(&'static str, String)]) -> String {
    if labels.is_empty() {
        return name.to_owned();
    }
    let body = labels
        .iter()
        .map(|(key, value)| {
            let escaped = value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n");
            format!("{key}=\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{name}{{{body}}}")
}

fn metrics() -> &'static Metrics {
    static METRICS: OnceLock<Metrics> = OnceLock::new();
    METRICS.get_or_init(Metrics::default)
}

/// Adds to a host counter. Label values are sanitized; callers only pass
/// closed sets of values (transport names, error codes).
pub fn inc(name: &'static str, labels: &[(&'static str, &str)]) {
    metrics().counter(name, labels, 1);
}

/// Adds `by` to a host counter.
pub fn add(name: &'static str, labels: &[(&'static str, &str)], by: u64) {
    metrics().counter(name, labels, by);
}

/// Records one duration sample. Rendered as `_seconds_sum`/`_seconds_count`.
pub fn observe(name: &'static str, labels: &[(&'static str, &str)], seconds: f64) {
    metrics().duration(name, labels, seconds);
}

/// Renders the Prometheus text exposition. Contains no request data.
pub fn render() -> String {
    metrics().render()
}

/// Metrics the JS host wrapper is allowed to increment. Arbitrary names are
/// ignored so user code cannot explode cardinality or smuggle values.
pub fn js_metric(name: &str) {
    match name {
        "http_requests" => inc("celld_http_requests_total", &[]),
        "stream_reads" => inc("celld_stream_reads_total", &[]),
        "stream_writes" => inc("celld_stream_writes_total", &[]),
        "stream_cancels" => inc("celld_stream_cancels_total", &[]),
        _ => {}
    }
}

/// Times one operation and records it on drop, even on early error returns.
pub struct Timer {
    name: &'static str,
    labels: Vec<(&'static str, &'static str)>,
    started: u64,
    done: bool,
}

impl Timer {
    pub fn start(name: &'static str, labels: &[(&'static str, &'static str)]) -> Self {
        Self {
            name,
            labels: labels.to_vec(),
            started: mono_us(),
            done: false,
        }
    }

    pub fn finish(&mut self) {
        if !self.done {
            self.done = true;
            observe(
                self.name,
                &self.labels,
                mono_us().saturating_sub(self.started) as f64 / 1_000_000.0,
            );
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.finish();
    }
}

struct TraceSpan {
    name: String,
    request_id: String,
    parent: u64,
    started: u64,
    attrs: BTreeMap<String, String>,
}

fn traces() -> &'static Mutex<BTreeMap<u64, TraceSpan>> {
    static TRACES: OnceLock<Mutex<BTreeMap<u64, TraceSpan>>> = OnceLock::new();
    TRACES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn trace_seq() -> &'static AtomicU64 {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    &SEQ
}

fn sanitize_attr(key: &str, value: &str) -> Option<(String, String)> {
    let key = sanitize(key, MAX_ATTRIBUTE_KEY);
    if key.is_empty() {
        return None;
    }
    Some((key, sanitize(value, MAX_ATTRIBUTE_VALUE)))
}

/// Starts a host span and emits a structured `span_begin` log. Returns the
/// host span id (0 means the span was rejected).
pub fn trace_begin(name: &str, attrs_json: Option<&str>, parent: u64, request_id: &str) -> u64 {
    let name = {
        let name = sanitize(name, MAX_SPAN_NAME);
        if name.is_empty() {
            "span".to_owned()
        } else {
            name
        }
    };
    let request_id = sanitize(request_id, MAX_REQUEST_ID);
    let mut attrs = BTreeMap::new();
    if let Some(raw) = attrs_json {
        if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(raw) {
            for (key, value) in map.into_iter().take(MAX_SPAN_ATTRIBUTES) {
                let value = match value {
                    serde_json::Value::String(value) => value,
                    serde_json::Value::Number(value) => value.to_string(),
                    serde_json::Value::Bool(value) => value.to_string(),
                    _ => continue,
                };
                if let Some((key, value)) = sanitize_attr(&key, &value) {
                    attrs.insert(key, value);
                }
            }
        }
    }
    let id = trace_seq().fetch_add(1, Ordering::Relaxed);
    inc("celld_trace_spans_total", &[]);
    {
        let mut spans = traces().lock().unwrap();
        if spans.len() >= MAX_LIVE_SPANS {
            if let Some(oldest) = spans.keys().next().copied() {
                spans.remove(&oldest);
            }
        }
        spans.insert(
            id,
            TraceSpan {
                name: name.clone(),
                request_id: request_id.clone(),
                parent,
                started: mono_us(),
                attrs: attrs.clone(),
            },
        );
    }
    inc("celld_trace_spans_total", &[("status", "started")]);
    let attrs = serde_json::to_string(&attrs).unwrap_or_else(|_| "{}".to_owned());
    tracing::info!(
        target: "celld::trace",
        event = "span_begin",
        span = id,
        parent,
        name = %name,
        request_id = %request_id,
        attrs = %attrs,
    );
    id
}

/// Sets or replaces one attribute on a live host span.
pub fn trace_attribute(id: u64, key: &str, value: &str) {
    let Some((key, value)) = sanitize_attr(key, value) else {
        return;
    };
    if let Some(span) = traces().lock().unwrap().get_mut(&id) {
        if span.attrs.len() < MAX_SPAN_ATTRIBUTES || span.attrs.contains_key(&key) {
            span.attrs.insert(key, value);
        }
    }
}

/// Ends a host span and emits a structured `span_end` log.
pub fn trace_end(id: u64, status: &str) {
    let Some(span) = traces().lock().unwrap().remove(&id) else {
        return;
    };
    let status = sanitize(status, 32);
    inc("celld_trace_spans_total", &[("status", "ended")]);
    let attrs = serde_json::to_string(&span.attrs).unwrap_or_else(|_| "{}".to_owned());
    tracing::info!(
        target: "celld::trace",
        event = "span_end",
        span = id,
        parent = span.parent,
        name = %span.name,
        request_id = %span.request_id,
        status = %status,
        duration_ms = mono_us().saturating_sub(span.started) as f64 / 1_000.0,
        attrs = %attrs,
    );
}

/// Test hook: the redacted attributes stored for a live span.
#[cfg(test)]
pub(crate) fn test_span_attributes(id: u64) -> Option<Vec<(String, String)>> {
    traces().lock().unwrap().get(&id).map(|span| {
        span.attrs
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_sanitize_labels_and_render() {
        inc("celld_test_total", &[("code", "owner-moved")]);
        inc("celld_test_total", &[("code", "owner-moved")]);
        inc("celld_test_total", &[("code", "has space/{x}")]);
        let text = render();
        assert!(text.contains("celld_test_total{code=\"owner-moved\"} 2"));
        assert!(text.contains("celld_test_total{code=\"has_space__x_\"} 1"));
    }

    #[test]
    fn duration_timer_records_one_sample() {
        let mut timer = Timer::start("celld_test_seconds", &[("transport", "local")]);
        timer.finish();
        let text = render();
        assert!(text.contains("celld_test_seconds_count{transport=\"local\"} 1"));
        assert!(text.contains("celld_test_seconds_sum{transport=\"local\"}"));
    }

    #[test]
    fn span_attributes_are_bounded_and_redacted() {
        let secret = "trace-secret-observability-unit".to_owned();
        crate::secrets::register_redactions(&[crate::secrets::PlainSecret {
            name: "TRACE_TEST".to_owned(),
            kind: crate::secrets::SecretKind::Text,
            value: secret.as_bytes().to_vec(),
        }]);
        let attrs = serde_json::json!({
            "safe": "value",
            "token": secret,
            "nested": {"not": "a scalar"},
        })
        .to_string();
        let id = trace_begin("test span", Some(&attrs), 7, "req-1");
        let stored = test_span_attributes(id).unwrap();
        assert!(stored.iter().any(|(k, v)| k == "safe" && v == "value"));
        assert!(stored
            .iter()
            .any(|(k, v)| k == "token" && v == "[redacted]"));
        assert!(!stored.iter().any(|(_, v)| v.contains(&secret)));
        assert!(!stored.iter().any(|(k, _)| k == "nested"));
        trace_end(id, "ok");
        assert!(test_span_attributes(id).is_none());
    }

    #[test]
    fn span_name_and_request_id_are_capped() {
        let long = "x".repeat(MAX_SPAN_NAME * 2);
        let id = trace_begin(&long, None, 0, &long);
        let spans = traces().lock().unwrap();
        let span = spans.get(&id).unwrap();
        assert!(span.name.len() <= MAX_SPAN_NAME);
        assert!(span.request_id.len() <= MAX_REQUEST_ID);
        drop(spans);
        trace_end(id, "ok");
    }
}
