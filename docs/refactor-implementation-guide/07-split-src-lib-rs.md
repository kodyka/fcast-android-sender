# 07 — Split `src/lib.rs`

**Priority:** High · **Effort:** High · **Estimated PR size:** 4–6 small PRs of ~250 LOC each.

## Goal

Carve up the 3076-line `src/lib.rs` into focused modules with stable JNI symbol
names. The aim is to keep every exported `Java_org_fcast_android_sender_*` symbol
exactly where it is on the ABI surface, while moving every helper and orchestration
function out into modules that can be tested and reasoned about independently.

## Report finding

> "`src/lib.rs` is 3,153 lines / 2,903 LOC and also acts as a major integration
> hub. […] In both places, multiple architectural layers are collapsed into single
> files: platform lifecycle, service control, media capture, rendering, JNI
> boundary, UI state, and backend orchestration."

— `deep-research-report-3.md`, "Detailed findings".

> "Extract Android JNI helpers, session orchestration, discovery/device
> connection, banner/state-handling, and backend switching into separate Rust
> modules/crates."

— same document, "Refactor plan".

## Pre-state on `main`

Verified offsets (`rg -n "^pub fn|^pub extern|^#\[unsafe\(no_mangle\)\]|^mod " src/lib.rs`):

| Range          | Content                                                                              |
|----------------|--------------------------------------------------------------------------------------|
| 1–162          | Crate-level imports; `RecordingTickerState`, `PlatformApp`, `PanelStack`.            |
| 164–560        | GStreamer init helpers, command-probe HTTP, legacy HTTP test helpers, graph-smoke.   |
| 571–668        | `JavaMethod` enum and dispatch helpers; `resolve_android_files_dir`.                 |
| 667–1290       | `Application` struct and impls (backend bootstrap, status items).                    |
| 1291–1420      | `default_presets`, `default_quick_actions`.                                          |
| 1421–2594      | `android_main` body — the giant entry point.                                          |
| 2594–2999      | JNI entries called from `MainActivity` (graph command, frame, QR, back, etc.).       |
| 3000–3120      | JNI entries called from `GstPopServiceBridge` / `MigrationRuntimeServiceBridge`.     |
| 3124+          | Inline `mod phase9_dispatch_tests`.                                                   |

Counts confirmed by `wc -l src/lib.rs` = 3076.

## Target module tree

```
src/
├── lib.rs                       (≤ 200 LOC: re-exports, JNI symbols only)
├── app.rs                       (NEW — composition root introduced in step 05)
├── platform/
│   ├── mod.rs
│   ├── gst_init.rs              (lines 164–178 today)
│   ├── platform_app.rs          (lines 73–162 today)
│   └── panel_stack.rs           (lines 128–162 today)
├── jni_bridge/
│   ├── mod.rs                   (declares the submodules)
│   ├── helpers.rs               (jstring_to_string, process_frame, JavaMethod, dispatch)
│   ├── main_activity.rs         (nativeGraphCommand, nativeProcessFrame, nativeQrScanResult,
│   │                              nativeBackPressed, nativeCaptureStarted/Stopped/Cancelled)
│   ├── discovery.rs             (FCastDiscoveryListener_serviceFound/serviceLost)
│   ├── gstpop_bridge.rs         (3 nativeStart/Stop/Status symbols for GstPop)
│   └── migration_bridge.rs      (3 nativeStart/Stop/Status symbols for MigrationRuntime)
├── command/
│   ├── mod.rs
│   ├── probe.rs                 (command_probe_addr, send_http_request)
│   ├── http_runner.rs           (run_graph_http_command, run_graph_command)
│   └── legacy_tests.rs          (run_legacy_http_*_test, run_graph_smoke_test)
├── application/
│   ├── mod.rs                   (struct Application)
│   ├── status.rs                (build_status_items)
│   └── defaults.rs              (default_presets, default_quick_actions)
└── android_main.rs              (the entry-point body, ~1000 LOC after extraction)
```

After this set of PRs, `lib.rs` contains only:

- `pub mod` declarations.
- Crate-level imports.
- Tracing/log-ring setup.
- The `Java_org_fcast_android_sender_*` `#[unsafe(no_mangle)]` exports, **each
  delegating to a function in `jni_bridge::*`**.

## Refactor recipe

This is the highest-effort step in the plan, so it is split into sub-PRs. Each
sub-PR is independently shippable.

### PR 7.1 — Move helpers (mechanical)

Move with `git mv`-equivalent edits:

- `jstring_to_string`, `process_frame`, `JavaMethod`, `call_java_method_no_args`
  → `src/jni_bridge/helpers.rs`.
- `command_probe_addr`, `send_http_request` → `src/command/probe.rs`.
- `default_presets`, `default_quick_actions` → `src/application/defaults.rs`.
- `build_status_items` → `src/application/status.rs`.

Add a top-of-file note in each new module:

```rust
//! Extracted from src/lib.rs as part of refactor step 07.
//! Do not add unrelated functions here without splitting the module further.
```

Touch the JNI symbols only to switch their helper calls to the new paths.

### PR 7.2 — Split JNI symbols by callsite

For each `Java_org_fcast_android_sender_*` symbol, move its body into one of:

- `src/jni_bridge/main_activity.rs`
- `src/jni_bridge/discovery.rs`
- `src/jni_bridge/gstpop_bridge.rs`
- `src/jni_bridge/migration_bridge.rs`

…and keep a wafer-thin re-export in `lib.rs`:

```rust
// src/lib.rs
mod jni_bridge;

#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MainActivity_nativeGraphCommand<'local>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
    json: jni::objects::JString<'local>,
) -> jni::objects::JString<'local> {
    jni_bridge::main_activity::native_graph_command(env, class, json)
}
```

The body in `jni_bridge::main_activity::native_graph_command` takes the same
signature, minus the `extern "C"` and `no_mangle`. Symbol names are unchanged,
so the Android side does not need to rebuild.

### PR 7.3 — Carve up `Application`

`Application` (lines 667–1290) is the second-largest single item in the file.
Move it to `src/application/mod.rs` as-is, then split:

- Status pipeline → `src/application/status.rs`.
- Backend bootstrap → `src/application/bootstrap.rs`.
- Preset/quick-action defaults → `src/application/defaults.rs` (already in 7.1).
- Slint-callback wiring (the parts that touch `MainWindow`) → leave in
  `application/mod.rs` for now; it can be split further in a follow-up.

### PR 7.4 — Move `android_main`

Move `android_main` and all of its private helpers to `src/android_main.rs`. The
function signature stays identical:

```rust
// src/lib.rs
mod android_main;

// existing `#[no_mangle]` ABI entry that JNI bootstrap calls — unchanged.
```

### PR 7.5 — Inline tests follow the code

Move `mod phase9_dispatch_tests` next to whichever module owns the code it
exercises. Use `#[cfg(test)]` consistently.

### PR 7.6 — Promote stable modules to crates

Only after 7.1–7.5 are merged and stable:

- `crates/runtime-core` — extract `command/`, `application/`, and the runtime-
  side traits (`MediaBackend`, `BackendRegistry`).
- `crates/android-jni` — extract `jni_bridge/` and `platform/`. This crate is
  the only one that depends on the `jni` crate.
- `crates/backend-migration` and `crates/backend-gstpop` already exist as
  `crates/migration-runtime` and `crates/gstpop-runtime`; align the names if the
  team wants the report's naming, or leave them alone.

This last sub-step is the only one that touches `Cargo.toml` and the crate
graph; 7.1–7.5 are pure file moves.

## Symbol-stability checklist

Every `Java_org_fcast_android_sender_*` symbol on `main` (verified by
`rg -n "^pub extern" src/lib.rs`):

```
Java_org_fcast_android_sender_MainActivity_nativeGraphCommand
Java_org_fcast_android_sender_FCastDiscoveryListener_serviceFound
Java_org_fcast_android_sender_FCastDiscoveryListener_serviceLost
Java_org_fcast_android_sender_MainActivity_nativeCaptureStarted
Java_org_fcast_android_sender_MainActivity_nativeCaptureStopped
Java_org_fcast_android_sender_MainActivity_nativeCaptureCancelled
Java_org_fcast_android_sender_MainActivity_nativeProcessFrame
Java_org_fcast_android_sender_MainActivity_nativeQrScanResult
Java_org_fcast_android_sender_MainActivity_nativeBackPressed
Java_org_fcast_android_sender_GstPopServiceBridge_nativeStartGstPopServiceHost
Java_org_fcast_android_sender_GstPopServiceBridge_nativeStopGstPopServiceHost
Java_org_fcast_android_sender_GstPopServiceBridge_nativeGetGstPopServiceStatus
Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStartMigrationRuntimeHost
Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStopMigrationRuntimeHost
Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeGetMigrationRuntimeStatus
```

After PR 7.2, run:

```bash
nm -D --defined-only target/aarch64-linux-android/debug/libfcastsender.so \
    | grep Java_org_fcast_android_sender \
    | sort > /tmp/symbols.after.txt
```

Compare against the same listing taken before the PR. The diff must be empty.

## Audit raw-pointer reconstruction

The report flags `unsafe { ...from_raw(...) }` reconstruction of `JavaVM` and
`JObject`. The cited line ranges (`2364-2417`, `3216-3345`, `3797-3821`) don't
all fall inside the actual 3076-LOC file — re-locate before patching:

```bash
rg -n 'from_raw|unsafe \{|JavaVM' src/lib.rs
```

For each hit, audit:

1. Does the caller hold a valid pointer for the duration of the call? Document
   it with a `// SAFETY:` comment in the same patch.
2. Can it be replaced with a `&JavaVM` borrow held in a `OnceCell` set during
   the bootstrap JNI entry? Prefer this.
3. If `JObject::from_raw(ptr)` is on a pointer obtained from a `GlobalRef`,
   prefer holding the `GlobalRef` and calling `.as_obj()`.

Do the safety audit as PR 7.7 — last in the series — to avoid mixing it with
mechanical moves.

## Testing

| Test                                                                | How                                                                  |
|---------------------------------------------------------------------|----------------------------------------------------------------------|
| Symbol stability                                                    | Diff `nm -D` before/after each PR.                                  |
| `cargo test -p fcastsender`                                          | Must pass after every PR.                                            |
| `cargo test -p fcastsender -- --test-threads=1`                      | Some legacy tests rely on global state — leave the gate in place.   |
| Headless Slint UI tests                                              | `cargo test -p fcastsender --test ui_snapshots`.                     |
| Android debug build                                                  | `./gradlew :app:assembleDebug`.                                      |
| `wc -l src/lib.rs` shrinks                                           | Target ≤ 400 LOC by end of 7.4.                                       |
| Migration smoke                                                      | The existing `scripts/` migration smoke (if present) — unchanged.   |

## Rollback

Each sub-PR is its own revert. Because every change in 7.1–7.5 is a pure code
move (no behavioural diff), reverting any one of them is safe. 7.6 (crate
promotion) touches `Cargo.toml` and the workspace `members` list — revert that
PR as a unit; do not revert the workspace change without also reverting the
file moves.

## Follow-ups (not in this PR)

- Move `MainActivity.java` to Kotlin and align the JNI symbol names against the
  new module tree — **Step 08**.
- Toolchain upgrade — **Step 11**.
