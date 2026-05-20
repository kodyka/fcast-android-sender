# 13 · File-by-file change list

Summary of the eventual implementation PR. No code is committed by
*this* guide PR — this list is the shopping list for the follow-up.

## 13.1 New files

| Path | Purpose | Step |
|---|---|---|
| `src/backend/gstpop/service.rs` | Rust → Java bridge: `request_service_start`, `request_service_stop`, `ServiceController` trait. | 5 |
| `app/src/main/java/org/fcast/android/sender/GstPopServiceBridge.java` | Java glue class; sole JNI entrypoint for the daemon. | 3 |
| `app/src/main/java/org/fcast/android/sender/GstPopService.java` | Foreground service hosting the daemon. | 4 |
| `app/src/test/java/org/fcast/android/sender/GstPopServiceBridgeTest.java` | Robolectric unit tests for the bridge. | 10 |

## 13.2 Edited files

| Path | Change | Step |
|---|---|---|
| `src/backend/gstpop/embedded.rs` | Add `EmbeddedState`, `EmbeddedStatus`, `start_embedded`, `stop_embedded`, `embedded_status`; keep helpers; delete `ensure_started` after callers migrate. | 2, 6 |
| `src/backend/gstpop/mod.rs` | `pub mod service;` | 5 |
| `src/backend/gstpop/backend.rs` | Drop the implicit `ensure_started` call from `probe`. | 6 |
| `src/backend/lifecycle.rs` | `apply` + `autostart` rewiring; `on_start_gstpop_service` / `on_stop_gstpop_service` registration; 1Hz status poller. | 5, 7 |
| `src/lib.rs` | Add three `Java_org_fcast_android_sender_GstPopServiceBridge_native…` exports; `HOST_RUNTIME`; `android_context()` helper. | 3, 5 |
| `ui/bridge.slint` | Add `MediaBackendState::starting`; new `start-gstpop-service` / `stop-gstpop-service` callbacks; `gstpop-service-state` / `gstpop-service-externally-owned` properties. | 7 |
| `ui/pages/media_backend_page.slint` | Render the `starting` state in the status pill; new "Service" section with Start/Stop buttons; externally-owned hint. | 7 |
| `app/src/main/AndroidManifest.xml` | Add `FOREGROUND_SERVICE_DATA_SYNC` permission; add `<service android:name=".GstPopService" …>` block. | 4 |

## 13.3 Optional follow-up edits

| Path | Change | When |
|---|---|---|
| `README.md` | Pointer to the guide. | 11.8 |
| `.github/workflows/android.yml` | `nm`-based JNI symbol check. | 10.7 |
| `app/src/main/res/values/strings.xml` | Localisable notification strings. | 11.6 |

## 13.4 What does **not** change

- `app/build.gradle` — no new dependencies. The Java new code uses
  only `android.app.*`, `org.json.*`, `androidx.annotation.*`, all
  already on the classpath.
- `vendor/gstpop/**` — entirely untouched.
- `src/backend/migration_backend.rs` — the migration backend is
  unrelated.
- `src/backend/persistence.rs` — `StoredBackendConfig` already has the
  fields needed (`kind`, `gstpop_url`, `gstpop_api_key`,
  `gstpop_pipeline_id`).
- `src/backend/kind.rs` — `BackendKind::GstPop` already exists.

## 13.5 LOC estimate

| Tier | New + edited LOC |
|---|---|
| Rust | ~350 (most of which is `embedded.rs` rewrite) |
| Java | ~250 (`GstPopService` + bridge + Robolectric test) |
| Slint | ~80 (state enum + 2 callbacks + UI block) |
| Manifest | ~5 lines |
| **Total** | **~700** |

## 13.6 Suggested commit breakdown

One PR per milestone (see README §M):

1. `refactor(gstpop): explicit start_embedded/stop_embedded/embedded_status API`
2. `feat(jni): GstPopServiceBridge native exports`
3. `feat(android): GstPopService foreground service + manifest`
4. `refactor(backend): route gst-pop startup through service in BackendLifecycle`
5. `refactor(gstpop): drop implicit start from probe()`
6. `feat(ui): MediaBackendState::Starting + Service section in Media Backend page`
7. `test: unit + Robolectric + Slint integration coverage`

Commits 1–2 can land together (no behaviour change yet). Commit 3
unblocks the service path. Commits 4–5 make the service path
authoritative. Commit 6 surfaces the new states. Commit 7 catches
regressions.

Next: [14-references.md](./14-references.md).
