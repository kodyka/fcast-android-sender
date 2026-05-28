# 03 — `MainActivity` lifecycle cleanup

**Priority:** Highest · **Effort:** Medium · **Estimated PR size:** ~80 LOC in one file.

## Goal

Pair every "register" / "start" / "acquire" in `MainActivity` with a matching
"unregister" / "quit" / "release" in `onDestroy` (and where appropriate `onStop`).
Today the activity registers a broadcast receiver, registers a display listener,
starts a `HandlerThread`, holds EGL state, and never tears any of it down.

## Report finding

> "In `onCreate()`, it initialises GStreamer, creates a `HandlerThread`, registers
> a local broadcast receiver, and registers a display listener. In the same file I
> did not find `unregisterReceiver`, `displayManager.unregisterDisplayListener`,
> `glThread.quit`, or an `onDestroy()` override. That is a concrete leak /
> shutdown-risk pattern, especially for an object that also owns EGL state and
> MediaProjection state."

— `deep-research-report-3.md`, "Detailed findings".

## Pre-state on `main`

`MainActivity.java` (verified line refs):

- `225` — `private HandlerThread glThread;`
- `227` — `private DisplayManager displayManager;`
- `323` — `private final CaptureBroadcastReceiver receiver = new CaptureBroadcastReceiver();`
- `338` — `projectionCallback = new ProjectionCallback();`
- `339` — `mediaProjectionManager = (MediaProjectionManager) getSystemService(MEDIA_PROJECTION_SERVICE);`
- `341-343` — `glThread = new HandlerThread("OpenGLThread"); glThread.start(); glHandler = new Handler(glThread.getLooper());`
- `347` — `LocalBroadcastManager.getInstance(this).registerReceiver(receiver, filter);`
- `350` — `displayManager.registerDisplayListener(this, new Handler(getMainLooper()));`

There is **no** `onDestroy`, **no** `onStop`, **no** `unregisterReceiver`,
**no** `unregisterDisplayListener`, **no** `glThread.quit()` /
`glThread.quitSafely()`, and **no** `mediaProjection.unregisterCallback(...)` in
the file (confirmed by ripgrep across the file).

## Target state

Add an `onDestroy` (and `onStop` for media projection) override that unwinds every
resource acquired in `onCreate`. The order matters: callbacks first, then the
thread that services those callbacks.

```java
@Override
protected void onDestroy() {
    Log.d(TAG, "onDestroy");

    // 1. Stop the capture pipeline first so frame callbacks stop firing.
    captureLock.lock();
    try {
        cleanupCapture(true);
    } finally {
        captureLock.unlock();
    }

    // 2. Drop the listener bus added in step 02 (no-op if step 02 not yet landed).
    CaptureResultBus.clearListener(captureListener);

    // 3. Unregister the system-side observers.
    if (displayManager != null) {
        displayManager.unregisterDisplayListener(this);
    }

    // 4. MediaProjection callbacks (registered in initializeCapture).
    if (mediaProjection != null) {
        try {
            mediaProjection.unregisterCallback(projectionCallback);
        } catch (Exception e) {
            Log.w(TAG, "MediaProjection.unregisterCallback failed", e);
        }
        mediaProjection.stop();
        mediaProjection = null;
    }

    // 5. Quit the OpenGL thread. quitSafely drains the queue first.
    if (glThread != null) {
        glThread.quitSafely();
        glThread = null;
        glHandler = null;
    }

    // 6. EGL teardown (must run on the GL thread normally — but we just quit
    //    it, so run it inline on this thread before super.onDestroy()).
    releaseEglResources();

    super.onDestroy();
}

@Override
protected void onStop() {
    super.onStop();
    // Keep MediaProjection alive while user is away briefly; release only on
    // explicit stop or finish(). If a regression is observed where the system
    // tears down the projection while the activity is just backgrounded, move
    // step 5/6 cleanup here.
}
```

### `releaseEglResources` (extracted helper)

`MainActivity` already holds the four EGL handles (`eglDisplay`, `eglSurface`,
`eglContext`, plus the `Surface` and `SurfaceTexture`). The teardown is a single
fixed sequence; wrap it once:

```java
private void releaseEglResources() {
    if (eglSurface != EGL14.EGL_NO_SURFACE) {
        EGL14.eglDestroySurface(eglDisplay, eglSurface);
        eglSurface = EGL14.EGL_NO_SURFACE;
    }
    if (eglContext != EGL14.EGL_NO_CONTEXT) {
        EGL14.eglDestroyContext(eglDisplay, eglContext);
        eglContext = EGL14.EGL_NO_CONTEXT;
    }
    if (eglDisplay != EGL14.EGL_NO_DISPLAY) {
        EGL14.eglTerminate(eglDisplay);
        eglDisplay = EGL14.EGL_NO_DISPLAY;
    }
    if (surface != null) {
        surface.release();
        surface = null;
    }
    if (surfaceTexture != null) {
        surfaceTexture.release();
        surfaceTexture = null;
    }
}
```

If the file already has scattered EGL teardown in `cleanupCapture`, prefer to keep
that path and only release things `cleanupCapture` does *not* touch. The intent is
to be idempotent: `releaseEglResources` must be safe to call twice.

## Consolidate `captureLock` semantics

The report calls out mixed locking (`captureLock.lock()/unlock()` plus
`synchronized (captureLock) { … }`). Use only one form in this PR — pick
`ReentrantLock` because we already need `tryLock` for the GL thread:

```diff
-synchronized (captureLock) {
+captureLock.lock();
+try {
     // … existing body …
-}
+} finally {
+    captureLock.unlock();
+}
```

Apply at `MainActivity.java:728` and `MainActivity.java:775` (the two
`synchronized (captureLock)` blocks). The `lock()`/`unlock()` call sites at
`633, 639, 662, 751` already use the right form.

## Diff summary

```text
app/src/main/java/org/fcast/android/sender/MainActivity.java
   + ~50 lines (onDestroy, onStop, releaseEglResources)
   ~ 4 lines  (synchronized → try/finally)
```

## Testing

| Test                                                                | How                                                                                              |
|---------------------------------------------------------------------|--------------------------------------------------------------------------------------------------|
| Activity finish() leaves no native references                       | `adb shell dumpsys meminfo org.fcast.android.sender` before and after. Native heap should drop. |
| Display listener stops firing after finish()                        | Rotate display while activity is gone — should not appear in logcat.                            |
| BroadcastReceiver stops firing after finish()                       | Trigger a manual `am broadcast` with `ACTION_MEDIA_PROJECTION_STARTED` — should be ignored.     |
| GL thread joins                                                     | `adb shell ps -T` post-finish — no `OpenGLThread` row.                                          |
| MediaProjection.Callback.onStop fires before our teardown            | Add a temporary `Log.d` in the callback; observe ordering.                                       |
| Slint headless UI tests                                              | `cargo test -p fcastsender --test ui_snapshots` — unaffected.                                  |
| Smoke: start capture, finish activity, start again                  | No black frames; no `EGL_BAD_DISPLAY`.                                                          |

## Rollback

Revert the file. The cleanup is purely additive; reverting cannot break the
existing happy path. If a regression appears specifically on `onDestroy`, comment
out steps 5 and 6 first (`glThread.quitSafely()` and `releaseEglResources()`) and
keep the receiver / display-listener unregistration.

## Follow-ups (not in this PR)

- Extract capture coordination out of `MainActivity` — **Step 04**.
- Move the activity to Kotlin so `lifecycleScope` can own the unregistration —
  **Step 08**.
