# 08 — Split `MainActivity.java` & migrate the Android shell to Kotlin

**Priority:** Medium · **Effort:** High · **Estimated PR size:** 3 PRs of ~300 LOC each.

## Goal

Reduce `MainActivity.java` from 1158 LOC to a thin shell (target: ≤ 300 LOC) by
moving every responsibility into a dedicated class, and migrate those classes to
Kotlin. This unlocks `lifecycleScope`, `StateFlow`, and `kotlinx-coroutines-test`
for the remaining steps.

## Report finding

> "Keep JNI method names stable, but move Android boundary classes to Kotlin for
> better coroutine/state-holder ergonomics."
>
> "Kotlin coroutines + `StateFlow` in the Android shell — Replaces callback/
> event-bus style orchestration and makes state-holder extraction easier."

— `deep-research-report-3.md`, "Refactor plan" and "Proposed target architecture".

## Pre-state on `main`

`MainActivity.java` is 1158 LOC and currently owns, by responsibility:

| Responsibility                                                   | Approximate ownership                                |
|-----------------------------------------------------------------|------------------------------------------------------|
| NativeActivity lifecycle / Slint hosting                        | `onCreate`, `onBackPressed`, key events.             |
| MediaProjection permission flow                                  | `onActivityResult`, projection callback.            |
| EGL/OpenGL setup and frame loop                                  | `setupGles`, `cleanupCapture`, frame pump.          |
| Frame-rate throttling                                            | `Instant.now()` / `Duration.between()` inline.      |
| QR scanning                                                      | ZXing launcher + result parsing.                    |
| Local broadcast handling                                         | `CaptureBroadcastReceiver` (removed in step 02).    |
| JNI command parsing                                              | `nativeGraphCommand` callers.                       |
| Graph smoke-test code                                            | `graphSmokeSequenceRan`.                            |

Steps 02–04 already extract some of this. Step 08 finishes the job.

## Target layout

```
app/src/main/java/org/fcast/android/sender/
├── MainActivity.kt                  (≤ 300 LOC, NativeActivity shell only)
├── FcastApp.kt                       (step 05)
├── AppGraph.kt                       (step 05)
├── capture/
│   ├── ScreenCaptureCoordinator.kt   (step 04)
│   └── CaptureEngine.kt              (step 04)
├── runtime/
│   ├── RuntimeBridge.kt              (step 05)
│   └── JniRuntimeBridge.kt           (step 05)
├── qr/
│   └── QrScannerLauncher.kt          (NEW — moves ZXing here)
├── shell/
│   ├── SenderController.kt           (NEW — state holder)
│   └── UiState.kt                    (NEW — sealed)
└── data/
    └── AndroidSecretStore.kt         (step 06)
```

## What `MainActivity.kt` looks like after this step

```kotlin
package org.fcast.android.sender

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch
import org.fcast.android.sender.capture.ScreenCaptureCoordinator
import org.fcast.android.sender.qr.QrScannerLauncher
import org.fcast.android.sender.shell.SenderController
import org.fcast.android.sender.shell.UiState

class MainActivity : ComponentActivity() {

    private lateinit var coordinator: ScreenCaptureCoordinator
    private lateinit var qr: QrScannerLauncher
    private lateinit var controller: SenderController

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val graph = (application as FcastApp).graph

        coordinator = graph.captureCoordinator
        qr          = QrScannerLauncher(this, graph.runtime)
        controller  = SenderController(graph.runtime, coordinator, qr)

        lifecycleScope.launch {
            controller.uiState.collect { onUiStateChanged(it) }
        }
    }

    override fun onDestroy() {
        controller.shutdown()
        coordinator.shutdown()
        super.onDestroy()
    }

    private fun onUiStateChanged(state: UiState) {
        // Slint native window already observes its own bridge globals — this
        // hook stays for non-Slint affordances (e.g. system back gestures).
    }

    // ── Native shim re-declarations ──────────────────────────────────────────
    // JNI symbol names are stable (see step 07); these declarations stay until
    // the corresponding callers move into Kotlin classes.
    external fun nativeBackPressed()
    external fun nativeCaptureStarted(width: Int, height: Int)
    external fun nativeCaptureStopped()
    external fun nativeCaptureCancelled(reason: String)
    external fun nativeProcessFrame(/* …existing params… */)
    external fun nativeQrScanResult(payload: String)
    external fun nativeGraphCommand(json: String): String

    companion object {
        init { System.loadLibrary("fcastsender") }
        const val ACTION_MEDIA_PROJECTION_STARTED = "org.fcast.android.sender.MEDIA_PROJECTION_STARTED"
    }
}
```

That is the entire shell. The capture pipeline, projection callback, EGL
handles, ZXing launcher, graph-smoke code, and broadcast-receiver inner class
have all moved into dedicated classes.

## `SenderController` — the state holder

```kotlin
package org.fcast.android.sender.shell

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.fcast.android.sender.capture.CaptureConfig
import org.fcast.android.sender.capture.ScreenCaptureCoordinator
import org.fcast.android.sender.qr.QrScannerLauncher
import org.fcast.android.sender.runtime.BackendKind
import org.fcast.android.sender.runtime.BackendStatus
import org.fcast.android.sender.runtime.RuntimeBridge

class SenderController(
    private val runtime: RuntimeBridge,
    private val coordinator: ScreenCaptureCoordinator,
    private val qr: QrScannerLauncher,
) {
    private val scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Default)

    private val _uiState = MutableStateFlow<UiState>(UiState.Disconnected)
    val uiState: StateFlow<UiState> = _uiState.asStateFlow()

    fun startBackend(kind: BackendKind, configJson: String): Job = scope.launch {
        _uiState.value = UiState.Starting(kind)
        val status: BackendStatus = runtime.startEmbeddedBackend(kind, configJson)
        _uiState.value = when (status.state) {
            "running"  -> UiState.Connected(kind, status.message)
            "error"    -> UiState.Error(status.message ?: "unknown")
            else       -> UiState.Disconnected
        }
    }

    fun stopBackend(kind: BackendKind): Job = scope.launch {
        runtime.stopEmbeddedBackend(kind)
        _uiState.value = UiState.Disconnected
    }

    fun shutdown() {
        scope.cancel()
    }
}
```

```kotlin
package org.fcast.android.sender.shell

import org.fcast.android.sender.runtime.BackendKind

sealed class UiState {
    data object Disconnected : UiState()
    data class Starting(val kind: BackendKind) : UiState()
    data class Connected(val kind: BackendKind, val message: String?) : UiState()
    data class Error(val message: String) : UiState()
}
```

These map 1:1 onto the existing Slint `AppState` enum if one exists; otherwise
they live alongside Slint's UI state and are wired through whichever `Bridge`
global the project already uses.

## Gradle / Kotlin setup

If the project does not already apply the Kotlin plugin:

```diff
 // app/build.gradle
 plugins {
     id 'com.android.application'
+    id 'org.jetbrains.kotlin.android'
 }
 android {
+    kotlinOptions { jvmTarget = '17' }
 }
 dependencies {
+    implementation 'org.jetbrains.kotlin:kotlin-stdlib'
+    implementation 'org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0'
+    testImplementation 'org.jetbrains.kotlinx:kotlinx-coroutines-test:1.9.0'
+    implementation 'androidx.activity:activity-ktx'
+    implementation 'androidx.lifecycle:lifecycle-runtime-ktx'
 }
```

`kotlinx-coroutines-test` is the official, vendor-supported test dispatcher;
the report flags it explicitly as the right choice for new controller/state-
holder tests.

## Migration recipe

Suggested PR breakdown:

- **PR 8.1 — Add Kotlin support + extract QR.** Land Kotlin plugin and move
  ZXing into `qr/QrScannerLauncher.kt`. No other behaviour changes.
- **PR 8.2 — Introduce `SenderController` + `UiState`.** `MainActivity` still
  calls into the runtime directly; `SenderController` is added in parallel,
  used only for the start/stop actions that already round-trip through
  `RuntimeBridge` (step 05).
- **PR 8.3 — Convert `MainActivity.java` → `MainActivity.kt`.** Mechanical
  conversion with IntelliJ's "Convert Java File to Kotlin", then manual
  cleanup. Keep `external fun` declarations matching the JNI symbol names.

Each PR is independently shippable — the language migration is the riskiest, so
land 8.1 and 8.2 first and let them soak for a release.

## Testing

| Test                                                              | How                                                                                          |
|-------------------------------------------------------------------|----------------------------------------------------------------------------------------------|
| Bytecode parity check                                              | `./gradlew :app:assembleDebug` and `aapt dump badging app-debug.apk` — package name unchanged. |
| JNI symbol resolution                                              | `nm -D --defined-only libfcastsender.so | grep -i mainactivity` — same set as before.        |
| New `SenderController` tests                                       | `:app:testDebugUnitTest` using `kotlinx-coroutines-test`'s `TestScope`.                       |
| Capture / QR / discovery happy paths                              | Manual, on a device.                                                                          |
| Slint headless UI tests                                            | `cargo test -p fcastsender --test ui_snapshots`.                                              |
| `MainActivity` size shrinks                                        | `wc -l app/src/main/java/org/fcast/android/sender/MainActivity.{kt,java}` — target ≤ 400 LOC. |

## Rollback

Each PR is its own revert. The Kotlin → Java conversion is the only step where
revert order matters: revert 8.3 before reverting 8.1, otherwise you end up
with Kotlin files in a project that has no Kotlin plugin applied.

## Follow-ups (not in this PR)

- Consolidate CI/CD — **Step 09**.
- Add Android-side tests against the new controller — **Step 10**.
- Toolchain upgrade — **Step 11**.
