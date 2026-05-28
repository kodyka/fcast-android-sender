# 99 — References

## Primary source

This guide is a step-by-step implementation translation of the deep-research
refactor report supplied with the originating task. It does **not** introduce
new architectural opinions on top of the report; where the report and the
verified state of `main` disagree, the verified state of `main` wins and the
discrepancy is recorded in the relevant step.

- **Deep-research report** (attached to the originating task) —
  `deep-research-report-3.md`. Sections referenced by name throughout this
  guide:
  - "Executive summary"
  - "Repository overview and dependency graph"
  - "Detailed findings and issue register"
  - "Refactor plan"
  - "Proposed target architecture and libraries"
  - "Testing, CI, risk and rollback"
  - "Open questions and limitations"

## Verified code references (anchors in `main`)

These line numbers were verified on `main` at the time of writing. Re-grep
before applying any step — line numbers drift as steps land.

### Java / Kotlin shell

- `app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java`
  - `:44-65` — `onStartCommand` with null deref + `START_STICKY`.
  - `:16, :61` — `LocalBroadcastManager` import + `sendBroadcast`.
- `app/src/main/java/org/fcast/android/sender/MainActivity.java`
  - `:225` — `private HandlerThread glThread;`
  - `:228` — `private final ReentrantLock captureLock = new ReentrantLock();`
  - `:310-321` — `class CaptureBroadcastReceiver`.
  - `:323` — `receiver` field.
  - `:325-355` — `onCreate` body (GL thread start, LBM register, display
    listener register).
  - `:347` — `LocalBroadcastManager.getInstance(this).registerReceiver(...)`.
  - `:350` — `displayManager.registerDisplayListener(this, ...)`.
- `app/src/main/java/org/fcast/android/sender/GstPopService.java`
  - `:35-71` — `onStartCommand` synchronously calls `nativeStart`.
- `app/src/main/java/org/fcast/android/sender/MigrationRuntimeService.java`
  - `:35-71` — same pattern as GstPopService.

### Rust runtime

- `src/lib.rs` — 3076 LOC; JNI exports at `:2600-3120`. Specific entries:
  - `:2601` `Java_..._MainActivity_nativeGraphCommand`.
  - `:2634` `Java_..._FCastDiscoveryListener_serviceFound`.
  - `:2739` `Java_..._FCastDiscoveryListener_serviceLost`.
  - `:2756` `Java_..._MainActivity_nativeCaptureStarted`.
  - `:2770` `Java_..._MainActivity_nativeCaptureStopped`.
  - `:2784` `Java_..._MainActivity_nativeCaptureCancelled`.
  - `:2945` `Java_..._MainActivity_nativeProcessFrame`.
  - `:2962` `Java_..._MainActivity_nativeQrScanResult`.
  - `:2979` `Java_..._MainActivity_nativeBackPressed`.
  - `:3003` `Java_..._GstPopServiceBridge_nativeStartGstPopServiceHost`.
  - `:3020` `Java_..._GstPopServiceBridge_nativeStopGstPopServiceHost`.
  - `:3034` `Java_..._GstPopServiceBridge_nativeGetGstPopServiceStatus`.
  - `:3059` `Java_..._MigrationRuntimeServiceBridge_nativeStartMigrationRuntimeHost`.
  - `:3082` `Java_..._MigrationRuntimeServiceBridge_nativeStopMigrationRuntimeHost`.
  - `:3098` `Java_..._MigrationRuntimeServiceBridge_nativeGetMigrationRuntimeStatus`.
- `src/backend/persistence.rs:9-15` — `StoredBackendConfig` carries
  `gstpop_api_key: Option<String>`.
- `src/backend/mod.rs:30-39` — process-global `BACKEND: Lazy<RwLock<Arc<dyn MediaBackend>>>`.

### Manifest

- `app/src/main/AndroidManifest.xml` — declares one `MainActivity` plus three
  services (`ScreenCaptureService`, `GstPopService`, `MigrationRuntimeService`),
  each with `foregroundServiceType="dataSync"`.

### Build / CI

- `gradle/wrapper/gradle-wrapper.properties:4` — Gradle 8.9.
- `gradle/libs.versions.toml:2` — AGP 8.7.0.
- `app/build.gradle:7,20,21` — `compileSdk 34`, `minSdk 26`, `targetSdk 34`.
- `.github/workflows/android-release-apk.yml` — debug APK build (despite the
  name).
- `.github/workflows/gstpop-smoke.yml:101-103` — `cargo test backend::gstpop --
  --include-ignored --test-threads=1` with comment explaining the gate.
- `.gitlab-ci.yml` — manual `assembleRelease`, produces
  `app-release-unsigned.apk` as a 30-day artifact.
- `.github/actions/android-ci-setup` — composite action that installs SDK +
  NDK r25c + GStreamer 1.28.0. No caching.

## Report findings vs. `main` state

| Report claim                                                          | Verified on `main`?              | Action                                                                 |
|-----------------------------------------------------------------------|----------------------------------|------------------------------------------------------------------------|
| `ScreenCaptureService` returns `START_STICKY` and reads `intent`-extras without a guard | Yes — `ScreenCaptureService.java:49, 64` | Step 01.                                                              |
| `LocalBroadcastManager` is used between service and activity          | Yes                              | Step 02.                                                              |
| `MainActivity` registers but never unregisters                        | Yes — no `onDestroy/onStop`      | Step 03.                                                              |
| `captureLock` mixes monitor + explicit lock APIs                      | Yes                              | Step 03 normalises to `ReentrantLock` only.                            |
| `GstPopService` / `MigrationRuntimeService` call `nativeStart` on the service main thread | Yes — `:46` in both files | Noted; step 05 introduces a typed bridge but does not move the call off the main thread (kept for future). |
| `gstpop_api_key` stored in plain `backend.json`                       | Yes — `persistence.rs:13`         | Step 06.                                                              |
| `BACKEND: Lazy<RwLock<…>>` global                                     | Yes — `backend/mod.rs:30`         | Step 05.                                                              |
| `MainActivity.java` is 965 LOC (report)                              | 1158 LOC on `main`                | Step 08 — use actual size in the PR description.                       |
| `src/lib.rs` is 2,903 / 3,153 LOC (report)                            | 3076 LOC on `main`                | Step 07 — use actual size in the PR description.                       |
| Cited `src/lib.rs` ranges `2364-2417`, `3216-3345`, `3797-3821`       | Only `2364-2417` is in-range     | Step 07 sub-PR 7.7 re-locates the raw-pointer audit before patching.   |
| AGP 8.7 / Gradle 8.9 / SDK 34                                          | Yes                              | Step 11.                                                              |
| GitLab pipeline produces unsigned APK                                  | Yes                              | Step 09.                                                              |
| `--test-threads=1` required for gst-pop suite                          | Yes — workflow line 101-103       | Step 10 — drop once step 07 removes the global state.                  |

## External / official references mentioned in the report

The report cites primary-source documentation. Where you need an authoritative
link rather than a deep-research citation, prefer:

- Android architecture guidance — <https://developer.android.com/topic/architecture>
- Android Gradle Plugin release notes — <https://developer.android.com/build/releases/gradle-plugin>
- Android 16 behavioural changes — <https://developer.android.com/about/versions/16/behavior-changes-all>
- `LocalBroadcastManager` deprecation — <https://developer.android.com/reference/androidx/localbroadcastmanager/content/LocalBroadcastManager>
- Jetpack DataStore — <https://developer.android.com/topic/libraries/architecture/datastore>
- `EncryptedSharedPreferences` (security-crypto) — <https://developer.android.com/topic/security/data>
- Kotlin coroutines — <https://kotlinlang.org/docs/coroutines-overview.html>
- `kotlinx-coroutines-test` — <https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-test/>
- Foreground service types (Android 14+) — <https://developer.android.com/about/versions/14/changes/fgs-types-required>

These supplement, not replace, the report. When a step's snippet differs from a
primary doc, follow the primary doc.

## Document version

Generated on `main` of the `kodyka/fcast-android-sender` repository at the
revision present when this PR opened. Re-run the verification grep commands
shown throughout the per-step files before applying any step on a newer `main`.
