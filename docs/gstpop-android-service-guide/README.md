# gst-pop → Android service implementation guide

Step-by-step implementation guide for moving the embedded gst-pop
daemon out of `GstPopBackend::probe()` and into a dedicated Android
service, with clear ownership across **Rust ↔ JNI ↔ Java ↔ Slint**.

> **Status:** design + recipe only. No source changes are made by these
> documents.

## How to read

The guide is split one-step-per-file. Sections build on each other —
read them in order on a first pass, then jump back into the section
you're implementing.

| # | File | What it covers |
|---|---|---|
| 0 | [00-plan-review.md](./00-plan-review.md) | Cross-check of the original 15-step plan against the actual code (7 deltas). |
| 1 | [01-continuity-contract.md](./01-continuity-contract.md) | Decision matrix: when a service is actually needed. |
| 2 | [02-rust-daemon-api.md](./02-rust-daemon-api.md) | Full Rust API: `EmbeddedState`, `EmbeddedStatus`, `start_embedded`, `stop_embedded`, `embedded_status`. |
| 3 | [03-jni-and-java-bridge.md](./03-jni-and-java-bridge.md) | Three new JNI exports + the `GstPopServiceBridge` Java glue class. |
| 4 | [04-android-service.md](./04-android-service.md) | Full `GstPopService` (foreground service) + `AndroidManifest.xml` diff. |
| 5 | [05-rewire-lifecycle.md](./05-rewire-lifecycle.md) | `BackendLifecycle::apply` + `autostart` rewiring, plus the reusable `android_context()` helper. |
| 6 | [06-tighten-probe.md](./06-tighten-probe.md) | Remove the implicit start from `GstPopBackend::probe()`. |
| 7 | [07-slint-ui-state.md](./07-slint-ui-state.md) | `bridge.slint` + `media_backend_page.slint` deltas, new `Starting` state, Start/Stop buttons, status pill. |
| 8 | [08-shutdown-policy.md](./08-shutdown-policy.md) | Per-trigger shutdown matrix. |
| 9 | [09-race-recovery.md](./09-race-recovery.md) | Five concrete race / recovery cases with defensive code. |
| 10 | [10-test-plan.md](./10-test-plan.md) | Rust unit, Slint integration, Robolectric, on-device manual. |
| 11 | [11-cleanup-checklist.md](./11-cleanup-checklist.md) | What to delete after the new path works. |
| 12 | [12-open-decisions.md](./12-open-decisions.md) | Foreground-service type, binder vs poll, API-key handling, restart-on-failure mode. |
| 13 | [13-file-change-list.md](./13-file-change-list.md) | File-by-file summary for the eventual implementation PR. |
| 14 | [14-references.md](./14-references.md) | Pointers into the current codebase. |

## Suggested milestone breakdown

The 14 steps collapse into 6 implementable milestones:

```
M1. Decide continuity contract           → step 1
M2. Refactor Rust daemon control         → step 2
M3. Add JNI + Java bridge                → step 3
M4. Add foreground GstPopService         → step 4
M5. Rewire lifecycle + remove implicit   → steps 5, 6
    start from probe
M6. Slint UI state + tests + cleanup     → steps 7, 8–11
```

Ship each milestone as its own PR. They are individually mergeable and
the daemon keeps working between milestones (probe-start stays as the
fallback until M5).

## Out of scope

- Migration-runtime routing changes. The `MigrationBackend` continues
  to handle in-process pipeline work regardless of the selected media
  backend. Moving gst-pop into a service does **not** solve that.
- Receiver-side / FCast-protocol work. The service hosts gst-pop only;
  WHEP signalling, mDNS discovery, and the sender pipeline are
  untouched.
- Multi-process daemon. The whole point of using a same-process
  foreground service is to keep `ServerHandle` live in the cdylib
  without a binder hop.
