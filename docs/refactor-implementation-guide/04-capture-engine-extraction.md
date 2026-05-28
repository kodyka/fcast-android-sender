# 04 — Extract `ScreenCaptureCoordinator` + `CaptureEngine`

**Priority:** Highest · **Effort:** High · **Estimated PR size:** 300–500 LOC.

## Goal

Move MediaProjection orchestration, EGL/OpenGL setup, frame throttling, and buffer
hand-off out of `MainActivity` and into two new classes:

- `ScreenCaptureCoordinator` — owns the permission flow, the lifecycle of
  `MediaProjection`, and the foreground-service contract.
- `CaptureEngine` — owns EGL/OpenGL, Y/U/V conversion, framebuffer readback,
  and the GL thread.

After this step, `MainActivity` no longer talks to EGL or to MediaProjection
directly; it forwards to a coordinator.

## Report finding

> "`MainActivity.java` is 1,162 lines / 965 LOC and mixes NativeActivity lifecycle
> handling, media projection permission flow, OpenGL/EGL setup, frame-rate
> throttling, QR scanning, local broadcast handling, JNI command parsing, and graph
> smoke-test code. […] Refactoring anywhere near these files currently has a very
> wide blast radius."

— `deep-research-report-3.md`, "Executive summary" and "Detailed findings".

Recommended target shape from the report:

```kotlin
interface ScreenCaptureCoordinator {
    suspend fun requestPermissions(activity: Activity): CapturePermissionResult
    suspend fun startCapture(result: CapturePermissionResult, config: CaptureConfig)
    suspend fun stopCapture()
}
```

## Pre-state on `main`

`MainActivity.java` currently owns, in order:

| Lines (approx.) | Responsibility                                                  |
|-----------------|-----------------------------------------------------------------|
| 220-232         | EGL handles, GL thread, capture lock, capture limits.           |
| 280-308         | `onDisplayChanged` cleans/re-sets up the EGL pipeline.          |
| 310-323         | `CaptureBroadcastReceiver` inner class.                          |
| 326-356         | `onCreate` — starts GL thread, registers receivers.              |
| ~600-790        | `initializeCapture`, `setupGles`, `cleanupCapture`, frame loop.  |

The actual line numbers will drift as steps 01–03 land; treat the columns above
as anchors and re-grep for `setupGles`, `cleanupCapture`, `initializeCapture`,
`graphSmokeSequenceRan`.

## Target file layout

```
app/src/main/java/org/fcast/android/sender/capture/
├── ScreenCaptureCoordinator.kt        (new, ~150 LOC)
├── CaptureEngine.kt                   (new, ~250 LOC)
├── CaptureConfig.kt                   (new, data class)
└── CapturePermissionResult.kt         (new, sealed class)
```

Kotlin is the recommended language here — `kotlinx.coroutines` makes the
GL-thread / main-thread / I/O-thread choreography tractable. If the team prefers
to land this step in Java first and migrate language separately in step 08, the
public API stays the same; only the file extensions change.

### `CaptureConfig` and `CapturePermissionResult`

```kotlin
package org.fcast.android.sender.capture

data class CaptureConfig(
    val maxWidth: Int = 1920,
    val maxHeight: Int = 1080,
    val maxFps: Int = 30,
)

sealed class CapturePermissionResult {
    data class Granted(val resultCode: Int, val data: android.content.Intent) : CapturePermissionResult()
    data object Denied : CapturePermissionResult()
    data class Failed(val message: String) : CapturePermissionResult()
}
```

### `ScreenCaptureCoordinator`

```kotlin
package org.fcast.android.sender.capture

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine

class ScreenCaptureCoordinator(
    private val context: Context,
    private val engine: CaptureEngine,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val mpm = context.getSystemService(MediaProjectionManager::class.java)
        ?: error("MediaProjectionManager unavailable")

    suspend fun requestPermissions(
        activity: ComponentActivity,
    ): CapturePermissionResult = suspendCancellableCoroutine { cont ->
        val launcher: ActivityResultLauncher<Intent> = activity.registerForActivityResult(
            ActivityResultContracts.StartActivityForResult()
        ) { result ->
            val data = result.data
            cont.resumeWith(Result.success(
                if (result.resultCode == Activity.RESULT_OK && data != null) {
                    CapturePermissionResult.Granted(result.resultCode, data)
                } else {
                    CapturePermissionResult.Denied
                }
            ))
        }
        try {
            launcher.launch(mpm.createScreenCaptureIntent())
        } catch (e: Exception) {
            cont.resumeWith(Result.success(CapturePermissionResult.Failed(e.message ?: "launch failed")))
        }
    }

    fun startCapture(result: CapturePermissionResult.Granted, config: CaptureConfig) {
        // Start the foreground service first — Android 14+ requires it within 5s.
        val svc = Intent(context, ScreenCaptureService::class.java)
            .setAction(ScreenCaptureService.ACTION_RESULT)
            .putExtra("resultCode", result.resultCode)
            .putExtra("data", result.data)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            context.startForegroundService(svc)
        } else {
            context.startService(svc)
        }
        scope.launch(Dispatchers.Default) {
            val projection: MediaProjection = mpm.getMediaProjection(result.resultCode, result.data)
                ?: error("getMediaProjection returned null")
            engine.start(projection, config)
        }
    }

    fun stopCapture() {
        engine.stop()
        context.stopService(Intent(context, ScreenCaptureService::class.java))
    }

    fun shutdown() {
        stopCapture()
        scope.cancel()
    }
}
```

### `CaptureEngine`

`CaptureEngine` owns the GL thread, EGL handles, the OES SurfaceTexture, and the
framebuffer readback path. The public API is small:

```kotlin
package org.fcast.android.sender.capture

import android.media.projection.MediaProjection
import android.os.HandlerThread

class CaptureEngine(
    private val onFrame: (FrameRef) -> Unit,
) {
    // ── public ────────────────────────────────────────────────────────────────
    fun start(projection: MediaProjection, config: CaptureConfig) { /* moved from MainActivity */ }
    fun stop()  { /* moved from MainActivity */ }
    fun release() { /* moved from MainActivity */ }
    fun onDisplayChanged(newSize: android.graphics.Point) { /* moved from MainActivity */ }

    // ── internal: moved from MainActivity verbatim, then cleaned up ──────────
    private var glThread: HandlerThread? = null
    // … EGL handles, virtualDisplay, surfaceTexture, captureLock, etc.

    private fun setupGles() { /* moved */ }
    private fun cleanupCapture(release: Boolean) { /* moved */ }
}
```

`FrameRef` is the type currently passed to native code (search `MainActivity.java`
for the JNI signature that takes capture buffers; mirror its parameters here as a
small data class). If extracting that type is non-obvious, leave a `TODO(step-04)`
and keep using the existing direct call into `nativeFrame*` for one PR.

## Migration recipe (do this in order)

1. **Create the four new files with empty bodies.** Land them as a no-op so the
   compile succeeds.
2. **Move `CaptureConfig` fields first.** Replace the three `userMax*` fields in
   `MainActivity` with a `CaptureConfig` instance. The activity still owns it for
   one PR.
3. **Move EGL / GL / SurfaceTexture into `CaptureEngine`.** Delete the moved
   blocks from `MainActivity`; the activity forwards to `engine.start` /
   `engine.stop` / `engine.onDisplayChanged`.
4. **Move MediaProjection acquisition into `ScreenCaptureCoordinator`.** Replace
   the activity's `getMediaProjection` call with `coordinator.startCapture`.
5. **Wire the permission flow.** Replace `startActivityForResult` (and the legacy
   `onActivityResult`) with `coordinator.requestPermissions(this)`.
6. **Remove dead code from `MainActivity`.** After steps 3–5, `setupGles`,
   `cleanupCapture`, EGL handles, `glThread`, `captureLock`, and the projection
   callback should all be gone from the activity file.

Each numbered move can in principle be its own PR; the report's effort estimate
("High") assumes the whole set lands as a single milestone, but each numbered move
above leaves the tree in a buildable state on its own.

## Testing

| Test                                                              | How                                                                      |
|-------------------------------------------------------------------|--------------------------------------------------------------------------|
| Capture starts and stops with the same UX                         | Manual.                                                                  |
| Display rotation still re-creates the pipeline                    | `adb shell wm size`, `adb shell wm density`.                             |
| GL thread reused across start/stop cycles                         | Add temporary `Log.d` in `CaptureEngine.start`; observe thread reuse.   |
| Memory does not climb across 10 start/stop cycles                 | `adb shell dumpsys meminfo org.fcast.android.sender` before/after loop. |
| Slint headless UI tests still pass                                | `cargo test -p fcastsender --test ui_snapshots`.                         |
| `MainActivity.java` line count drops                              | `wc -l app/src/main/java/org/fcast/android/sender/MainActivity.java` — should land closer to 700 LOC. |
| Android Lint                                                       | `./gradlew :app:lint`.                                                   |
| Optional instrumentation test                                     | New `:app:connectedAndroidTest` job exercising `ScreenCaptureCoordinator.startCapture` with a fake projection. |

## Rollback

Each of the six migration sub-steps is independently revertable. The riskiest sub-
step is **step 3** (EGL move). If a regression appears only after that step,
revert it and leave the empty `CaptureEngine` class in place — that costs nothing
and keeps the future re-attempt small.

## Follow-ups (not in this PR)

- Replace static service-bridge access with injected coordinators — **Step 05**.
- Move `MainActivity` itself to Kotlin and reduce it to a thin shell — **Step 08**.
