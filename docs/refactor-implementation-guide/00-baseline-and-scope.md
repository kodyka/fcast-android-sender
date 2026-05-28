# 00 — Baseline & Scope

> Read this before any of the per-step files. It establishes the ground truth that
> every later step refers back to.

## Baseline (verified on `main`)

These numbers are taken from `wc -l` on `main` at the time of writing. Re-verify
before applying any step.

| File / area                                                              | Size      | Why it matters                                                         |
|--------------------------------------------------------------------------|-----------|------------------------------------------------------------------------|
| `app/src/main/java/org/fcast/android/sender/MainActivity.java`           | 1158 LOC  | NativeActivity shell + EGL + capture + QR + LBM + JNI command parsing. |
| `app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java`   |   84 LOC  | One-shot MediaProjection hand-off, currently `START_STICKY`.            |
| `app/src/main/java/org/fcast/android/sender/GstPopService.java`          |  174 LOC  | Foreground host for gst-pop runtime. Calls `nativeStart()` synchronously. |
| `app/src/main/java/org/fcast/android/sender/MigrationRuntimeService.java`|  154 LOC  | Foreground host for migration runtime. Same shape as GstPopService.    |
| `app/src/main/java/org/fcast/android/sender/GstPopServiceBridge.java`    |   70 LOC  | Static JNI bridge.                                                     |
| `app/src/main/java/org/fcast/android/sender/MigrationRuntimeServiceBridge.java` | 73 LOC | Static JNI bridge.                                                |
| `src/lib.rs`                                                             | 3076 LOC  | JNI entry points + session orchestration + discovery + UI state.       |
| `src/backend/persistence.rs`                                             |   42 LOC  | Plain JSON `backend.json` with `gstpop_api_key: Option<String>`.        |
| `src/backend/mod.rs`                                                     |   53 LOC  | Global `BACKEND: Lazy<RwLock<Arc<dyn MediaBackend>>>`.                  |

## Confirmed findings from the report

The report cites several specific code patterns. Cross-check against `main`:

1. **`ScreenCaptureService` is sticky and dereferences `intent` without a null guard.**
   `ScreenCaptureService.java:45-65` reads `intent.getIntExtra(...)` and
   `intent.getParcelableExtra(...)`, then returns `START_STICKY` on line 64.
2. **`LocalBroadcastManager` is the only cross-process channel between
   `ScreenCaptureService` and `MainActivity`.** Both files import
   `androidx.localbroadcastmanager.content.LocalBroadcastManager`; the service sends
   via `LocalBroadcastManager.getInstance(this).sendBroadcast(...)` and the activity
   registers a matching receiver in `onCreate`.
3. **`MainActivity` registers but never unregisters.** `MainActivity.java:347`
   registers the broadcast receiver and `MainActivity.java:350` registers a display
   listener; no `onDestroy`/`onStop` override and no matching unregister calls exist
   in the file. The `HandlerThread` created at `MainActivity.java:341-343` is never
   `quit()`ed.
4. **Mixed locking.** `MainActivity.java:228` declares
   `private final ReentrantLock captureLock = new ReentrantLock();` and the capture
   path uses both `captureLock.lock()` / `captureLock.unlock()` (lines 633, 639,
   662, 751, 775) and `synchronized (captureLock) { … }` (lines 728, 775).
5. **`GstPopService` and `MigrationRuntimeService` call `nativeStart()` on the
   service main thread.** `GstPopService.java:46` and
   `MigrationRuntimeService.java:46` invoke the static `nativeStart(...)` directly
   inside `onStartCommand`, after `startForeground` but on the main thread.
6. **`backend.json` mixes secrets and config.** `src/backend/persistence.rs:13`
   declares `pub gstpop_api_key: Option<String>` on the same struct that holds the
   non-secret URL and pipeline id.
7. **Backend is a process-global singleton.** `src/backend/mod.rs:30-35` exposes a
   `static BACKEND: Lazy<RwLock<Arc<dyn MediaBackend>>>` and `current()` reads it.

## Report findings that need additional verification before landing

These are taken from the report but the cited line numbers do **not** match
`main`. The architectural claim is plausible; the *exact location* needs to be
re-derived before writing a patch. Per-step files mark these explicitly.

- "Helper functions reconstruct `JavaVM` and `JObject` from raw pointers using
  `unsafe { ...from_raw(...) }`" — true for `src/lib.rs` in general; cited line
  ranges (`2364-2417`, `3216-3345`, `3797-3821`) do not all fall inside the actual
  3076-LOC file. Step 07 re-locates these before refactoring.
- "`gst-pop` smoke tests must run with `--test-threads=1`." — true at the workflow
  level. Step 10 quotes the actual CI flags from the workflow file.

## Scope of this refactor

In scope:

- All Java files under `app/src/main/java/org/fcast/android/sender/`.
- `src/lib.rs`, `src/backend/`, and the JNI-adjacent helpers it pulls in.
- `app/src/main/AndroidManifest.xml` service declarations.
- `app/build.gradle`, the Gradle wrapper, and CI workflow files when explicitly
  named in a step.

Out of scope for this guide (deferred to a follow-up plan):

- Slint UI redesign. The existing `ui/` tree is treated as fixed.
- `crates/migration-runtime` and `crates/gstpop-runtime` internal architecture.
- `vendor/gstpop` and any `kodyka/fcast` git dependencies.

## Conventions used in every step

- **Pre-state** — the code as it exists on `main`, with verified line numbers.
- **Target state** — the code after the step is applied.
- **Diff** — a unified diff that can be applied to a fresh branch off `main`.
- **Testing** — exactly what to run locally and in CI.
- **Rollback** — what to revert and what data to keep if the step has to be undone.

If any of those five sections is missing from a per-step file, that step is not
ready to merge.
