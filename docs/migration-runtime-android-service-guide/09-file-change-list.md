# 09 — File change list

Summary of every file the eventual implementation PR will touch.
Numbers are approximate LOC for the new content; existing files only
show the **delta** size.

## 9.1 Files to CREATE

| # | File | LOC | Step |
|---|------|-----|------|
| 1 | `app/src/main/java/org/fcast/android/sender/MigrationRuntimeService.java` | ~150 | [03](./03-android-service.md) |
| 2 | `app/src/main/java/org/fcast/android/sender/MigrationRuntimeServiceBridge.java` | ~70 | [02](./02-java-bridge.md) |
| 3 | `src/migration/service.rs` | ~110 | [05](./05-rust-caller-helper.md) |

## 9.2 Files to MODIFY

| # | File | Δ LOC | Step |
|---|------|--------|------|
| 4 | `app/src/main/AndroidManifest.xml` | +5 | [04](./04-android-manifest.md) |
| 5 | `src/lib.rs` | +60 | [01](./01-rust-jni-bridge.md) |
| 6 | `src/migration/mod.rs` | +1 (`pub mod service;`) | [05 §5.2](./05-rust-caller-helper.md#52-module-registration) |
| 7 | `ui/bridge.slint` | +5 | [06 §6.1](./06-slint-ui-integration.md#61-new-properties--callbacks-in-uibridgeslint) |
| 8 | `ui/state/media_backend.slint` | +5 | [06 §6.2](./06-slint-ui-integration.md#62-mirror-in-uistatemedia_backendslint) |
| 9 | `ui/pages/media_backend_page.slint` | +55 (one new `SettingsSection`) | [06 §6.3](./06-slint-ui-integration.md#63-new-service-section-in-uipagesmedia_backend_pageslint) |
| 10 | `src/backend/lifecycle.rs` | +60 (two callbacks + 1 Hz poller) | [06 §6.4](./06-slint-ui-integration.md#64-rust-callback-wiring-srcbackendlifecyclers) – [§6.5](./06-slint-ui-integration.md#65-1-hz-status-poller) |

## 9.3 Files to NOT touch

For clarity, the following plausible-looking files do **not** need
changes:

| File | Why not |
|------|---------|
| `app/src/main/java/org/fcast/android/sender/MainActivity.java` | The existing gst-pop service is also not referenced from `MainActivity`. Service wiring flows through Rust (`backend::gstpop::service` for gst-pop, `migration::service` for the runtime) — see [00 row 7](./00-plan-review.md). |
| `app/build.gradle` | No new dependencies, no new permissions, no new source dirs. See [07 §7.1](./07-build-and-package.md#71-no-buildgradle-changes). |
| `src/migration/runtime.rs` | The runtime is consumed as-is. Adding fields to a runtime-side status struct would require also adjusting `lib.rs` and adding tests; the minimum-viable JNI export synthesises the JSON envelope in `lib.rs` directly. |
| `Cargo.toml` | All required crates (`jni`, `serde_json`, `anyhow`, `tracing`) are already direct dependencies. |
| `.pre-commit-config.yaml` | The Slint snippets in step 6 are hook-clean as written; no rule changes needed. |
| `ui/theme.slint` | All tokens referenced in the new Slint block already exist. |

## 9.4 Implementation order

A minimal-risk order that keeps every intermediate PR green:

1. **PR A (Rust + Java bridge skeleton).** Files 1, 2, 4, 5. The
   service is now installable and `adb am start-foreground-service`
   works. No Rust call sites or Slint UI consume it yet.
2. **PR B (Rust caller helper).** Files 3, 6. `migration::service`
   compiles on all targets. Still no callers.
3. **PR C (Slint UI surface).** Files 7, 8, 9, 10. The Media Backend
   panel grows the Start/Stop service section. Users can toggle the
   migration runtime service the same way they can gst-pop.

Each PR is individually reviewable, individually mergeable, and
individually reversible. Aim for ≤300 LOC per PR.
