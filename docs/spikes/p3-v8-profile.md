# P3 spike — V8 CPU profiles for `celld dev --profile`

Status: **research complete, mechanism proven**. No upgrade and no V8 patch are
required. The `celld dev --profile` / `CELLD_PROFILE` wiring is a follow-up.

Date: 2026-10-22
Base: `celld-os/rpc` @ `adde2defacb5d90cf6a2041ca3c2aa95c18607fb` (C4)
Branch: `spike/v8-profile`
Toolchain: rustc/cargo 1.95.0, x86_64 Linux

## Question

Can `celld dev --profile` / `CELLD_PROFILE` produce a real V8 CPU profile with
a supported binding on the pinned V8, or is an upgrade/patch required?

## Answer

Yes, through the **V8 Inspector protocol**, which the pinned `v8` crate already
exposes. The crate has no direct `CpuProfiler::StartProfiling` binding, but
`v8::inspector::V8Inspector` + `V8InspectorSession::dispatch_protocol_message`
support the CDP `Profiler.enable/start/stop` domain. A prototype on this branch
produced a genuine `.cpuprofile` (59 samples, hot frame `spin` with 55 hits).

- No upgrade is needed for profiling. `v8` 152.2.0 (the newest published
  version) has the same profiler surface as 152.1.0.
- No vendored patch is needed. The crate's own `binding.cc` already includes
  `v8-profiler.h`, so a direct `CpuProfiler` binding is *possible*, but it
  would require forking/patching `rusty_v8`; the inspector avoids that.
- `deno_core` is not a dependency (`grep -c deno_core Cargo.lock` → 0), so it
  imposes no compatibility constraint here. If celld ever adopts it, it uses
  the same inspector route.

## Pinned versions

| item | value |
| --- | --- |
| workspace `v8` | `v8 = "152.1"` (`Cargo.toml`) |
| lockfile | `v8 152.1.0`, crates.io, checksum `eb60870e…5ccf83` |
| latest published | `152.2.0` (`cargo info v8`) |
| deno_core | absent |

## Commands

```bash
git switch -c spike/v8-profile celld-os/rpc
cargo info v8
# audit the pinned crate
grep -n "pub fn .*profil\|CpuProfiler" ~/.cargo/registry/src/*/v8-152.1.0/src/isolate.rs
grep -rn "StartProfiling\|StopProfiling\|GetCpuProfile" ~/.cargo/registry/src/*/v8-152.1.0/src
# audit the newest crate
curl -sL -A 'celld-p3-research/1.0' \
  https://crates.io/api/v1/crates/v8/152.2.0/download -o v8-152.2.0.crate
tar xzf v8-152.2.0.crate
grep -n "pub fn .*profil\|CpuProfiler" v8-152.2.0/src/isolate.rs
grep -rn "StartProfiling\|StopProfiling\|GetCpuProfile" v8-152.2.0/src
# prototype (writes target/spikes/v8-profile-spike.cpuprofile)
cargo test -p celld --test v8_profile_spike -- --nocapture
```

## Symbol audit

| symbol | 152.1.0 | 152.2.0 |
| --- | --- | --- |
| `Isolate::collect_cpu_profiler_sample` | yes | yes |
| `Isolate::use_detailed_source_positions_for_profiling` | yes | yes |
| `Isolate::cpu_profiler_metadata_size` | yes | yes |
| `CpuProfiler::StartProfiling` / `StopProfiling` / `GetProfile` | **no** | **no** |
| `inspector::V8Inspector::create` / `connect` | yes | yes |
| `inspector::V8InspectorSession::dispatch_protocol_message` | yes | yes |
| CDP `Profiler.enable` / `start` / `stop` | yes | yes |
| `v8-profiler.h` vendored in the crate | yes | yes |

`CollectSample` is not a profile: it emits individual samples and needs an
external collector. The Inspector path already aggregates samples into a
profile tree and serializes it as `.cpuprofile`.

## Prototype

`crates/celld/tests/v8_profile_spike.rs` (spike artifact, integration test):

1. `celld::js::Engine::init()`, create an isolate.
2. `V8Inspector::create` + `connect` with a channel that captures responses.
3. Dispatch `Profiler.enable`, `Profiler.start` (100 µs sampling).
4. Run a CPU-bound JS function (`spin(8_000_000)`).
5. Dispatch `Profiler.stop`, parse `result.profile`, assert the hot function
   and non-empty samples, write `target/spikes/v8-profile-spike.cpuprofile`.

Result:

```text
P3 profile: 5 nodes, 59 samples, 1298 bytes
node 5 spin hitCount=55
test inspector_produces_a_real_v8_cpu_profile ... ok
```

This is a real profile (timestamps, node tree, per-frame hit counts), not a
synthetic file.

## Integration design for `celld dev --profile` / `CELLD_PROFILE`

Feasible without touching V8; the work is engine lifecycle plumbing.

1. **Per-isolate session.** celld creates isolates in `js::Worker::load_config`
   (`v8::Isolate::new`). Create one inspector + session there, on the isolate
   thread, owned by the worker slot.
2. **Context hooks.** Call `inspector.context_created(...)` where celld creates
   its `v8::Context`, and `context_destroyed` when it is dropped. Without this
   the profile still records frames, but URLs/context names are poorer.
3. **Control path.** `celld dev --profile [DIR]` / `CELLD_PROFILE=DIR`
   (default `.celld/profiles`), dev-only and off by default. Start after
   `ready`, stop on SIGINT, reload, or `q`; write one `<slot>.cpuprofile` per
   isolate and print the paths (same style as the new `metrics` line).
4. **Threading.** Inspector sessions use `Rc<RefCell<…>>` and must run on the
   isolate thread. The dev controller should send start/stop into each slot
   rather than calling V8 from the CLI thread.
5. **Safety rails.** Never enable this in `celld serve`; no HTTP endpoint; no
   request bodies, headers, or tokens. Profiles contain script URLs and
   function names, so they stay local and are written only when opted in.
6. **Defaults.** Sampling interval 1000 µs (V8 default); document the
   overhead. Multi-isolate profiles are separate files; a shared wall-clock
   window is a refinement, not a blocker.

Files/effort: `js.rs` (isolate/context hooks), `dev.rs` (flag + shutdown
plumbing), a small slot registry, and a dev-level test (start dev with
`--profile`, hit a worker, stop, assert a non-empty profile). Roughly
200–350 LOC plus tests. Risk is moderate because it touches isolate lifecycle,
but it is dev-only and opt-in.

## Alternatives and tradeoffs

| option | JS frames | per-isolate | cost |
| --- | --- | --- | --- |
| Inspector `Profiler.*` (**recommended**) | yes | yes | supported; ~200–350 LOC lifecycle plumbing |
| Direct `CpuProfiler` bindings | yes | yes | needs a `rusty_v8` fork/`[patch.crates-io]`; headers already vendored; ongoing maintenance |
| V8 `--prof` + tick processor | yes | no (process log) | init flag, external `v8.log` processing, tooling not shipped |
| Host `perf`/pprof (+ V8 perf map) | partial/needs `--perf-basic-prof` | no | native + JS symbolization, extra flags, perf privileges |
| `collect_cpu_profiler_sample` | samples only | yes | no aggregation/tree; needs a collector |

## Recommendation

**Use the Inspector path. Do not upgrade or patch V8 for profiling.**
152.1.0 already supports it, and 152.2.0 adds nothing relevant. Keep P3 marked
pending in C4 until the dev-only `--profile` plumbing above lands; the risk is
in isolate lifecycle, not in V8 support.
