# 00 — Plan review (cross-check against the actual code)

The submitted Version 1 / Version 2 plans are accurate at a high level
(mirror `GstPopService`, add 3 JNI exports, add a manifest entry). The
following deltas were found by reading the current source on `main`
and **MUST** be respected when implementing — they are the spots where
naïve copy-paste from `GstPopService` will not compile or will behave
incorrectly. Subsequent step files resolve every row.

| # | Plan statement | Reality on `main` | Action |
|---|----------------|-------------------|--------|
| 1 | "Return JSON with at least `{"state":"running"\|"stopped"\|"error", ...}` to match the pattern GstPopService uses." | `migration::runtime::start_graph_runtime()` returns `Result<()>` — there is **no** `EmbeddedStatus`-equivalent struct that already serialises to that shape. Source: `src/migration/runtime.rs:302-320`. | The JNI layer must **synthesise** the JSON itself. See [01-rust-jni-bridge.md §1.2](./01-rust-jni-bridge.md#12-status-json-shape). |
| 2 | "Follow the same pattern as the existing GstPopServiceBridge JNI methods around line 2994-3037" — uses `HOST_RUNTIME.block_on(...)`. | `start_graph_runtime` / `shutdown_graph_runtime` are **synchronous** (they spawn their own `std::thread::Builder` threads internally). There is no async future to drive. Source: `src/lib.rs:88-94` defines `HOST_RUNTIME`; `runtime.rs:302-320`. | Do **not** wrap calls in `HOST_RUNTIME.block_on(async { … })`. Call them directly on the JNI binder thread — the work returns in milliseconds. |
| 3 | "Update notification text from 'gst-pop' to 'Migration Runtime'." Implicit: copy `describe(statusJson)` 1:1. | `GstPopService.describe(...)` reads `bind` and `port` fields from the status JSON. The migration runtime has no port concept exposed in its status. Source: `app/src/main/java/org/fcast/android/sender/GstPopService.java:138-156`. | The `describe(...)` helper in `MigrationRuntimeService` must read only `state` and `last_error`. **Delete** the bind/port references. See [03-android-service.md](./03-android-service.md). |
| 4 | "Use foregroundServiceType=\"dataSync\" (same as GstPopService)." | Correct — the matching permission `android.permission.FOREGROUND_SERVICE_DATA_SYNC` is already declared. Source: `AndroidManifest.xml:7`. | Use as planned. No new `<uses-permission>` needed. |
| 5 | "Notification ID 3 (GstPopService uses 2, ScreenCaptureService uses 1)." | Confirmed. Sources: `ScreenCaptureService.java:57` (`startForeground(1, …)`); `GstPopService.java:26` (`NOTIFICATION_ID = 2`). | Use `3` as planned. |
| 6 | Version 2: "After the existing GstPopService declaration (line 30)…" | The `GstPopService` block in `AndroidManifest.xml` actually spans lines **26-30**. | Insert the new `<service>` after line 30. See [04-android-manifest.md](./04-android-manifest.md). |
| 7 | "MainActivity.java — may need to import/reference the new service (e.g., for starting it from UI)." | The existing `GstPopServiceBridge` is **not** called from `MainActivity` either. It is invoked from Rust via JNI reflection in `src/backend/gstpop/service.rs`. Source: `MainActivity.java` contains zero `GstPop*` references. | Do **not** add anything to `MainActivity.java` for the migration runtime service either. Wiring is from Rust — see [05-rust-caller-helper.md](./05-rust-caller-helper.md). |
| 8 | "queryStatus() — calls nativeGetMigrationRuntimeStatus()" | The natural Rust implementation is `try_handle_command_json("{\"getinfo\":{}}")`. That function **always returns a String** (never errors). Source: `runtime.rs:349-366`. | Status JNI export wraps that call and re-shapes the response into the `{state, …}` envelope. See [01-rust-jni-bridge.md §1.1](./01-rust-jni-bridge.md#11-three-new-exports). |
| 9 | "start(Context, String) — starts the MigrationRuntimeService via Intent." | The migration runtime currently takes **no** start-time config. The `configJson` parameter is preserved purely for symmetry and forward-compatibility. | Plumb it through, but ignore it on the Rust side for now. Mark this in code comments. See [02-java-bridge.md](./02-java-bridge.md). |
| 10 | Not mentioned in plan. | `GstPopService` performs an "externally_owned" check that triggers a 500 ms self-stop dance. Source: `GstPopService.java:126-133`. | The migration runtime is **always in-process** — there is no external owner. **Omit** the `isExternallyOwned` branch entirely from `MigrationRuntimeService`. See [03-android-service.md](./03-android-service.md). |

The rest of the plan (file names, package, action constants, channel
id, private static native declarations) is correct as written.

## Quick sanity check

Before opening the implementation PR, verify each row above against
your local checkout. The line numbers cited here are stable as of
the commit at the time of writing (see
[10-references.md](./10-references.md) for the cross-reference table)
but you should re-check them after any rebase.
