# Step 8 — Non-goals and risks

[← Previous: Step 7](07-route-a-game-text-input.md) · [Index](README.md) · [Next: Step 9 →](09-checklist.md)

## What this guide deliberately does **not** change

- The native render path, EGL/GLES setup, `MediaProjection`, and the GStreamer bridge are untouched. All of the existing code in `MainActivity` from `setupGles()` through `cleanupCapture()` is preserved verbatim.
- `ScreenCaptureService`, `MigrationRuntimeService`, `GstPopService` are untouched.
- `MainActivity.onCreate()`’s existing ordering — `GStreamer.init`, `Discoverer`, `projectionCallback`, `mediaProjectionManager`, `glThread`, `LocalBroadcastManager`, `displayManager`, `POST_NOTIFICATIONS` permission — is preserved. The IME bridge is appended at the end of `onCreate()`.
- All non-Android-12 code paths keep the current `ANativeActivity_showSoftInput()` behavior. Android 11 and Android 13+ are not touched.
- No manifest, `app/build.gradle`, or `CMakeLists.txt` changes for **Route B**. (Route A needs all three — see [Step 7](07-route-a-game-text-input.md).)

## Risks

### Focus stealing

The 1×1 invisible `RustImeView` is focusable. If anything in the app calls `findFocus()` or relies on the `NativeContentView` being the focused view, that view may now answer instead.

**Mitigations:**

1. Only call `requestFocus()` inside `showImeFromNative()` (Step 4) — never on `onResume` or activity start.
2. After `hideImeFromNative()`, optionally return focus to the native render view:
   ```java
   FrameLayout root = findViewById(android.R.id.content);
   if (root != null && root.getChildCount() > 0) {
       root.getChildAt(0).requestFocus();  // NativeContentView is child 0
   }
   ```
3. If back-button handling regresses (`MainActivity.onKeyUp` / `dispatchKeyEvent` at lines 241–267), inspect `findFocus()` to confirm which view is receiving the event.

### Key event interception

`RustImeView`’s `BaseInputConnection.sendKeyEvent()` forwards key downs to native via `onImeKeyNative`. The existing `MainActivity` already has `KEYCODE_BACK` handling in `dispatchKeyEvent`, `onKeyDown`, `onKeyUp`. While the IME is focused, hardware back may route through `RustImeView` before reaching the activity. This is normal Android behavior, but if the back-handling code is sensitive to the exact dispatch path, audit it on Android 12.

### Configuration changes

`MainActivity` declares `configChanges="keyboardHidden|orientation|screenSize"` (manifest line 47), so the activity is **not** recreated on rotation. The `imeView` field survives and no state restoration is needed.

If you ever remove `configChanges`, you must:

- Re-install the bridge in the new `onCreate()` (already gated on `Build.VERSION_CODES.S`/`S_V2`, so this happens automatically).
- Persist any pending text state to `onSaveInstanceState`.

### `MediaProjection` coexistence

Showing the IME during an active screen capture is uncommon but legal. The IME window overlays the captured surface and is **included** in the projection (`MediaProjection` captures the display, including IME). If you want to hide the IME during capture, call `hideImeFromNative()` in the existing `mediaProjection.registerCallback(...)` start path.

### Other Android versions in the future

If field reports come in that Android 13+ also misbehaves with the legacy NDK path, broaden the gate in both `MainActivity.installAndroid12ImeBridge()` and the native `fcast_show_soft_input` ([Step 5](05-native-jni-wiring.md)) from `== S || == S_V2` to `>= S`. **Do not broaden speculatively** — the current `ANativeActivity_showSoftInput()` path is what the rest of the app has been running against on those versions, and changing it is a regression risk.

### Native callbacks (`RustImeView` natives)

The three `native` methods on `RustImeView` must be registered on the Rust/C side (either via the implicit `Java_…` symbols or via `RegisterNatives`). Until they are, the class will throw `UnsatisfiedLinkError` on first character input.

**Workaround for incremental landing:** use the [stub variant](03-rust-ime-view.md#stub-friendly-variant-drop-in-until-rust-side-is-ready) — drop the `native` keyword, add a `Log.d` body. This unblocks Steps 2–4 and [Step 6](06-runtime-sanity-checks.md) Check 1/2 without needing the JNI side ready. Replace with `native` once [Step 5](05-native-jni-wiring.md) is implemented.

### Selection drift

`BaseInputConnection.getEditable()` returns the same `Editable` instance forever. If multiple IME sessions reuse it, span indices can drift. The `replaceState()` helper in [Step 3](03-rust-ime-view.md) plus `restartInput()` in [Step 4](04-show-hide-from-native.md)’s `replaceImeTextFromNative()` resync the IME. Always go through `replaceImeTextFromNative` when the Rust side modifies the model; do not mutate `editable` from any other Java code.

### Memory

`RustImeView` holds one `Editable` and one `BaseInputConnection`. Both are negligible. The view is never destroyed during the activity lifetime (no rotation recreation), so no leak surface beyond the activity itself.

## Risks summary table

| Risk | Severity | Mitigation |
|---|---|---|
| Focus stealing | Medium | Focus only inside `showImeFromNative`; restore native view focus in `hideImeFromNative` if needed |
| Key dispatch path change | Low | Audit `dispatchKeyEvent` / `onKeyUp` on Android 12 |
| `MediaProjection` overlap | Low | Optionally hide IME during capture |
| Over-gating future versions | Low | Wait for field reports before broadening |
| Missing JNI registration | High at integration time | Use stub variant until Step 5 is landed |
| Selection drift | Low | Always resync via `replaceImeTextFromNative` |

[← Previous: Step 7](07-route-a-game-text-input.md) · [Index](README.md) · [Next: Step 9 →](09-checklist.md)
