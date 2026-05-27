# Migration Runtime → Android service implementation guide

Step-by-step recipe to host the existing Rust **migration runtime**
(`migration::runtime::start_graph_runtime` / `shutdown_graph_runtime`)
inside a dedicated Android foreground service, mirroring the existing
`GstPopService` pattern, with optional Slint UI wiring for an explicit
Start/Stop service surface.

> **Status:** design + recipe only. **No source changes** are made by
> these documents — they are the spec for the eventual implementation
> PR. All code blocks are illustrative; copy them into the listed files
> when implementing.

## How to read

The guide is split one-step-per-file. Sections build on each other —
read them in order on a first pass, then jump back into the section
you're implementing.

| # | File | What it covers |
|---|------|---|
| 0 | [00-plan-review.md](./00-plan-review.md) | Cross-check of the original plan against the actual code on `main` (10 deltas). |
| 1 | [01-rust-jni-bridge.md](./01-rust-jni-bridge.md) | Three new JNI exports + `migration_runtime_status_json` helper in `src/lib.rs`. |
| 2 | [02-java-bridge.md](./02-java-bridge.md) | Full `MigrationRuntimeServiceBridge.java` source. |
| 3 | [03-android-service.md](./03-android-service.md) | Full `MigrationRuntimeService.java` source. |
| 4 | [04-android-manifest.md](./04-android-manifest.md) | `AndroidManifest.xml` diff. |
| 5 | [05-rust-caller-helper.md](./05-rust-caller-helper.md) | New `src/migration/service.rs` — Rust → Java reflection helper. |
| 6 | [06-slint-ui-integration.md](./06-slint-ui-integration.md) | `bridge.slint` + `media_backend.slint` + `media_backend_page.slint` deltas, Rust callback wiring, 1 Hz poller. |
| 7 | [07-build-and-package.md](./07-build-and-package.md) | Symbol matching, ProGuard, cargo features. |
| 8 | [08-test-plan.md](./08-test-plan.md) | Manual + cargo + gradle test plan. |
| 9 | [09-file-change-list.md](./09-file-change-list.md) | File-by-file summary for the implementation PR. |
| 10 | [10-references.md](./10-references.md) | Line-pinned pointers into the current codebase. |

## Suggested milestone breakdown

The 11 step files collapse into 4 implementable milestones:

```
M1. Rust JNI bridge + Java bridge       → steps 1, 2
M2. Foreground MigrationRuntimeService  → steps 3, 4
M3. Rust caller helper                  → step 5
M4. Slint UI surface + 1 Hz poller      → step 6
```

Ship each milestone as its own PR. They are individually mergeable:

* After M1+M2, the service can be started via `adb am start-foreground-service`
  (smoke-testable without touching Rust call sites).
* After M3, in-process Rust code can drive the service through
  `migration::service::request_service_start` / `request_service_stop`.
* After M4, users can toggle the service from the Media Backend panel
  with the same Start/Stop UX gst-pop already has.

## Out of scope

* **Replacing the in-process `migration::runtime` call sites.** The
  service is an additional **host** for the existing runtime; it does
  not remove direct callers in debug/test code (see
  `src/lib.rs:2466-2521`).
* **Multi-process daemon.** Like `GstPopService`, the migration runtime
  service hosts the runtime in the same process as the app. The
  foreground service exists to give the OS a process-priority anchor,
  not to isolate the runtime.
* **Receiver-side / FCast-protocol work.** The runtime continues to
  speak its existing `Command` / `ControllerMessage` JSON protocol; this
  guide doesn't change that.
