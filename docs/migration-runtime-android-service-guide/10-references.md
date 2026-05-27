# 10 — References (line-pinned)

Every cross-reference in this guide that points into the current
codebase. Line numbers are accurate as of the commit at the time of
writing — re-check after any rebase.

## 10.1 Java sources (Android)

| Topic | File | Lines |
|-------|------|-------|
| `GstPopService` template (full lifecycle) | `app/src/main/java/org/fcast/android/sender/GstPopService.java` | 1-174 |
| `describe()` with `bind`/`port` fields | `app/src/main/java/org/fcast/android/sender/GstPopService.java` | 138-156 |
| `isExternallyOwned()` + 500 ms self-stop dance | `app/src/main/java/org/fcast/android/sender/GstPopService.java` | 126-133, 166-173 |
| `GstPopServiceBridge` template | `app/src/main/java/org/fcast/android/sender/GstPopServiceBridge.java` | 1-70 |
| Notification ID precedent (`= 1`) | `app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java` | 57 |
| `MainActivity` contains zero `GstPop*` references | `app/src/main/java/org/fcast/android/sender/MainActivity.java` | grep gives 0 hits |
| Manifest insertion point | `app/src/main/AndroidManifest.xml` | 26-30 |
| `FOREGROUND_SERVICE` + `FOREGROUND_SERVICE_DATA_SYNC` permissions | `app/src/main/AndroidManifest.xml` | 5, 7 |

## 10.2 Rust sources

| Topic | File | Lines |
|-------|------|-------|
| Existing GstPop JNI exports (template) | `src/lib.rs` | 2988-3037 |
| `parse_gstpop_config_port` (insertion-anchor helper) | `src/lib.rs` | 3040-3044 |
| `HOST_RUNTIME` lazy_static (NOT needed here) | `src/lib.rs` | 82-95 |
| `jstring_to_string` helper | `src/lib.rs` | 2588-2602 |
| `AndroidCtx` + `android_context` | `src/lib.rs` | 97-110 |
| Debug-only Slint callbacks consuming `start_migration_server` etc. | `src/lib.rs` | 2466-2521 |
| `migration::runtime::start_graph_runtime` | `src/migration/runtime.rs` | 302-310 |
| `migration::runtime::shutdown_graph_runtime` | `src/migration/runtime.rs` | 312-320 |
| `migration::runtime::handle_command_json` | `src/migration/runtime.rs` | 334-347 |
| `migration::runtime::try_handle_command_json` | `src/migration/runtime.rs` | 349-366 |
| Internal refresh + command-server thread spawn | `src/migration/runtime.rs` | 40-73 |
| Rust → Java reflection precedent (gst-pop) | `src/backend/gstpop/service.rs` | 1-102 |
| `load_app_class` helper to copy | `src/backend/gstpop/service.rs` | 13-35 |
| `request_service_start` template | `src/backend/gstpop/service.rs` | 38-65 |
| `request_service_stop` template | `src/backend/gstpop/service.rs` | 67-92 |
| Lifecycle: gst-pop start/stop callbacks (template) | `src/backend/lifecycle.rs` | 77-98 |
| Lifecycle: 1 Hz gst-pop poller (template) | `src/backend/lifecycle.rs` | 100-124 |

## 10.3 Slint sources

| Topic | File | Lines |
|-------|------|-------|
| `MediaBackendKind` / `MediaBackendState` enums | `ui/bridge.slint` | 50-61 |
| Media-backend selector properties | `ui/bridge.slint` | 318-324 |
| `start-gstpop-service` / `stop-gstpop-service` callbacks (template) | `ui/bridge.slint` | 348-353 |
| `MediaBackend` global (template to mirror) | `ui/state/media_backend.slint` | 5-29 |
| `gstpop-service-state` properties on `MediaBackend` | `ui/state/media_backend.slint` | 17-18 |
| Existing gst-pop `SERVICE` section in Media Backend page | `ui/pages/media_backend_page.slint` | 131-187 |
| Status-pill colour expression (template) | `ui/pages/media_backend_page.slint` | 149-154 |
| Status text expression (template) | `ui/pages/media_backend_page.slint` | 156-167 |
| Start/Stop button row (template) | `ui/pages/media_backend_page.slint` | 170-184 |

## 10.4 Related guides

| Guide | Notes |
|-------|-------|
| [`docs/gstpop-android-service-guide/README.md`](../gstpop-android-service-guide/README.md) | Sibling guide for the gst-pop foreground service — same architectural pattern. |
| [`docs/gstpop-android-service-guide/03-jni-and-java-bridge.md`](../gstpop-android-service-guide/03-jni-and-java-bridge.md) | Gst-pop's three JNI exports — direct template for [01](./01-rust-jni-bridge.md). |
| [`docs/gstpop-android-service-guide/04-android-service.md`](../gstpop-android-service-guide/04-android-service.md) | Gst-pop's full service file — direct template for [03](./03-android-service.md). |
| [`docs/gstpop-android-service-guide/07-slint-ui-state.md`](../gstpop-android-service-guide/07-slint-ui-state.md) | Gst-pop's Slint surface — direct template for [06](./06-slint-ui-integration.md). |
| [`docs/service-abstraction-refactor-guide/STEP-03-migration-service-wrapper.md`](../service-abstraction-refactor-guide/STEP-03-migration-service-wrapper.md) | A different angle on "migration runtime as a service" — a Rust `ServiceManager` trait wrapper. Cross-reference for context; **not** an alternative to this guide. |

## 10.5 Pre-commit config

| File | Why it matters |
|------|----------------|
| `.pre-commit-config.yaml` | Two hooks (`forbid-raw-hex-colors`, `forbid-hard-coded-font-size`) gate Slint changes. The snippets in step 6 are designed to pass both — see [06 §6.6](./06-slint-ui-integration.md#66-pre-commit-hook-reminder). |
