# gstpop-runtime Android-first migration plan

Step-by-step porting plan for `crates/gstpop-runtime`, derived from
[`deep-research-gstpop-demon.md`](./deep-research-gstpop-demon.md) and scoped
for **Android MVP first, other platforms later**.

> **Per-step detail files (with full code examples) live in
> [`gstpop-android-mvp-plan/`](./gstpop-android-mvp-plan/README.md).** Each
> step below links to its detail file.

The guiding principle: `vendor/gstpop` already contains the daemon core. Treat
`crates/gstpop-runtime` as a thin **app-facing façade**, not a place to
re-implement daemon internals.

## Success criteria for the Android MVP

The first milestone is done when, on an `arm64-v8a` device:

- App starts an embedded gstpop server on `127.0.0.1`.
- App connects via `GstPopClient`.
- App can `create_pipeline`, `play`, `pause`, `stop`, `get_position`.
- App shuts the runtime down cleanly and idempotently.
- APK builds reproducibly under the current Gradle/NDK setup.

Anything outside that list is deferred to Phase 2 or 3.

---

## Phase 1 — Android MVP (highest priority)

### Step 1. `EmbeddedConfig` + `start_embedded_with_config()`

**Files:** `crates/gstpop-runtime/src/embedded.rs`,
`crates/gstpop-runtime/src/lib.rs`.

- Add `EmbeddedConfig { bind, port, api_key, allowed_origins }` with
  `EmbeddedConfig::localhost(port)` constructor.
- Add `pub async fn start_embedded_with_config(cfg: EmbeddedConfig) -> EmbeddedStatus`.
- Refactor existing `start_embedded(port)` to delegate to it (preserve current
  default: loopback, no auth, no origins, `no_dbus = true`).
- Surface bind/startup errors via `EmbeddedStatus.last_error` (do not silently
  swallow).
- Re-export `EmbeddedConfig` from `lib.rs`.

**Done when:** existing callers compile unchanged AND a new caller can pass a
custom `EmbeddedConfig`.

### Step 2. Preserve vendored `TcpListener` pre-bind behavior

**Files:** `vendor/gstpop/src/server.rs`,
`vendor/gstpop/src/websocket/server.rs`.

- Verified existing divergence from upstream (server binds before spawn).
- Add a comment marking this as an intentional Android-relevant deviation so a
  future upstream sync does not accidentally revert it.
- No code change needed unless the upcoming upstream sync touches these files.

### Step 3. Typed client helpers

**Files:** new `crates/gstpop-runtime/src/typed_client.rs`,
`crates/gstpop-runtime/Cargo.toml` (add `typed-client` feature),
`crates/gstpop-runtime/src/lib.rs` (feature-gated re-export).

Implement `TypedGstPopClient` wrapping `GstPopClient` with methods:

- `create_pipeline(description) -> String`
- `list_pipelines() -> Vec<PipelineSummary>`
- `play / pause / stop(pipeline_id: Option<&str>)`
- `remove_pipeline(id)`
- `update_pipeline(id, description)`
- `get_position(id: Option<&str>) -> PositionInfo`

Use `serde_json::from_value` for results. No transport changes.

**Done when:** unit tests show typed calls produce the same JSON-RPC frames as
the manual `client.call(...)` equivalents.

### Step 4. Android-safe media path handling

**Files:** new `crates/gstpop-runtime/src/media.rs` behind `media-tools`
feature.

- Port `normalize_uri` as `normalise_media_input(input, base_dir: Option<&Path>)`.
  - On Android: caller must pass `Some(app_files_dir)`.
  - On desktop: `None` falls back to `current_dir()` (upstream behavior).
- Port `build_playbin_description` as `build_playbin_description_cross(...)`
  with the same explicit-base-dir signature and sink-name validation.
- Do **not** call `std::env::current_dir()` from any Android code path.

**Done when:** unit tests cover absolute path, relative path with base,
relative path without base (error on Android-style usage), Windows drive
letters, and a pre-formed URI passthrough.

### Step 5. Embedded-server integration tests

**Files:** `crates/gstpop-runtime/tests/embedded_integration.rs`.

Tests must cover:

- Embedded server starts on an ephemeral port.
- Two concurrent `start_embedded` calls converge without panic.
- Bind failure (port in use) surfaces as `EmbeddedState::Error` with non-empty
  `last_error`.
- `TypedGstPopClient` can connect and round-trip `create_pipeline` → `play` →
  `stop` → `remove_pipeline`.
- `stop_embedded()` is idempotent.

These tests gate every later step.

### Step 6. Android arm64 build validation

**Scope:** verify the existing build pipeline still works after Steps 1–5.

- Keep `app/build.gradle` `abiFilters "arm64-v8a"` as-is.
- Confirm `ANDROID_NDK_HOME` and `GSTREAMER_ROOT_ANDROID` flow through the
  root `build.rs` without changes.
- Build: `cargo ndk -t arm64-v8a -o app/src/main/jniLibs build --release`.
- Package: `./gradlew :app:assembleDebug`.
- Smoke-test on device: open app, observe embedded server start, exercise
  one play/pause cycle.

Defer all other ABIs.

---

## Phase 2 — Android polish (medium priority)

### Step 7. Optional JNI bridge in `gstpop-runtime`

**Only do this if** the app crate's current JNI is awkward or duplicated. If
the app already owns a working JNI layer, skip.

If pursued:

- New module `crates/gstpop-runtime/src/android_jni.rs` gated on
  `#[cfg(all(target_os = "android", feature = "android-jni"))]`.
- Expose **only**:
  - `nativeStartEmbedded(port: jint) -> jstring` (JSON status)
  - `nativeStopEmbedded() -> jstring`
  - `nativeStatus() -> jstring`
- Single shared tokio runtime in a `Lazy<Mutex<Runtime>>`.
- Do not expose per-RPC JNI methods; let Kotlin talk to the WebSocket.

### Step 8. Media discovery wrapper

- Re-export `discover_uri()` through `media-tools`.
- Add a thin `discover(input, base_dir)` helper that normalises first.
- Useful for UI metadata (duration, streams) before playback.

### Step 9. Typed protocol model

- Replace string state/event fields in `protocol.rs` with typed enums
  (`PipelineState`, `PipelineEventKind`, etc.) mirroring
  `vendor/gstpop/src/gst/event.rs`.
- Add `#[serde(other)]` fallthrough for forward-compatibility.

---

## Phase 3 — Desktop and cross-platform (low priority)

### Step 10. Desktop tooling feature

`desktop-tools` feature exposes:

- `registry::get_elements` / `get_element` wrappers
- `inspect_format` text formatter
- Plugin listing helpers

Keep out of default mobile build.

### Step 11. Multi-ABI Android

Add ABIs in this order, only after arm64 is stable:

1. `x86_64-linux-android` (emulator)
2. `armv7-linux-androideabi` (only if 32-bit devices required)
3. `i686-linux-android` (rarely needed)

Requires matching `GSTREAMER_ROOT_ANDROID` ABI libs and broader `abiFilters`.

### Step 12. Separate desktop CLI crate

New crate `crates/gstpop-cli` (not in `gstpop-runtime`) hosting:

- `daemon`, `play`, `launch`, `discover`, `inspect` subcommands
- Upstream-style clap parsing and CLI tests
- `wait_for_shutdown()` signal handling (Unix + Ctrl-C)

The mobile runtime stays free of `clap`, `tracing-subscriber`, and signal
plumbing.

---

## Explicitly not in scope for Phase 1

| Item | Why deferred |
|---|---|
| Full CLI subcommands | Not needed for Android app |
| DBus | Linux-only; already `cfg`-gated out |
| Process signal handling | Android lifecycle owned by the OS, not the runtime |
| Registry/inspect surfaces | Debug tooling, not playback path |
| Upstream CLI test parity | Only matters with a CLI crate |
| Multi-ABI packaging | arm64-only is simpler and sufficient |
| LAN-exposed WebSocket | Adds auth, origin, and permission risk |

---

## Feature flag layout

```toml
# crates/gstpop-runtime/Cargo.toml
[features]
default = []
typed-client = []
media-tools = []
desktop-tools = []
android-jni = []

[target.'cfg(target_os = "android")'.dependencies]
jni = { workspace = true, optional = true }
ndk-context = { version = "0.1.1", optional = true }
```

Recommended build commands:

```bash
# Android MVP
cargo build -p gstpop-runtime --features "typed-client media-tools"

# Android MVP with in-crate JNI
cargo build -p gstpop-runtime --features "typed-client media-tools android-jni"

# Desktop / debug
cargo build -p gstpop-runtime --features "typed-client media-tools desktop-tools"
```

---

## Linear porting checklist

Execute strictly in this order; do not jump ahead:

1. [ ] [Step 1 — `EmbeddedConfig` + `start_embedded_with_config`](./gstpop-android-mvp-plan/step-01-embedded-config.md)
2. [ ] [Step 2 — Preserve vendored `TcpListener` pre-bind](./gstpop-android-mvp-plan/step-02-preserve-prebind.md)
3. [ ] [Step 3 — Typed client helpers](./gstpop-android-mvp-plan/step-03-typed-client.md)
4. [ ] [Step 4 — Android-safe media path handling](./gstpop-android-mvp-plan/step-04-android-safe-media.md)
5. [ ] [Step 5 — Embedded-server integration tests](./gstpop-android-mvp-plan/step-05-integration-tests.md)
6. [ ] [Step 6 — Android arm64 build validation](./gstpop-android-mvp-plan/step-06-android-arm64-build.md)
7. [ ] [Step 7 — Optional JNI bridge](./gstpop-android-mvp-plan/step-07-jni-bridge.md)
8. [ ] [Step 8 — Media discovery wrapper](./gstpop-android-mvp-plan/step-08-media-discovery.md)
9. [ ] [Step 9 — Typed protocol enums](./gstpop-android-mvp-plan/step-09-typed-protocol.md)
10. [ ] [Step 10 — Desktop tooling feature](./gstpop-android-mvp-plan/step-10-desktop-tools.md)
11. [ ] [Step 11 — Multi-ABI Android packaging](./gstpop-android-mvp-plan/step-11-multi-abi-android.md)
12. [ ] [Step 12 — Separate desktop CLI crate](./gstpop-android-mvp-plan/step-12-desktop-cli-crate.md)
13. [ ] [Step 13 — Signal handling (CLI only)](./gstpop-android-mvp-plan/step-13-signal-handling.md)

---

## Open questions to resolve before starting

- Does the existing app-crate JNI already cover what Step 7 would add? If yes,
  skip Step 7 entirely.
- Is `app/build.gradle`'s arm64-only `abiFilters` intentional and the GStreamer
  Android tarball available for arm64 only, or is multi-ABI possible today?
- Should `normalise_media_input` accept an Android `Context` indirectly via
  `ndk-context`, or always require the caller to pass `base_dir` explicitly?
  (Recommendation: explicit `base_dir` — keeps the runtime testable on desktop.)
