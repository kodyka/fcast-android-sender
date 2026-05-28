# Step 9 — Final landing checklist

[← Previous: Step 8](08-non-goals-and-risks.md) · [Index](README.md)

The smallest viable set of changes to land Route B and verify it.

## Code changes

- [ ] **New file** `app/src/main/java/org/fcast/android/sender/RustImeView.java` — full snippet from [Step 3](03-rust-ime-view.md). If the JNI side is not yet ready, use the stub variant (no `native` keyword, `Log.d` bodies).
- [ ] **Add field** `private RustImeView imeView;` to `MainActivity` — [Step 2](02-install-ime-bridge.md).
- [ ] **Add imports** to `MainActivity.java`: `android.view.Gravity`, `android.widget.FrameLayout`, `android.view.WindowInsets`, `android.view.WindowInsetsController`, `android.view.inputmethod.InputMethodManager`, `android.text.InputType`.
- [ ] **Append to `onCreate()`** (`MainActivity.java:326`+) the version-gated `installAndroid12ImeBridge()` call — [Step 2](02-install-ime-bridge.md).
- [ ] **Add `installAndroid12ImeBridge()`** method to `MainActivity` — [Step 2](02-install-ime-bridge.md).
- [ ] **Add `showImeFromNative(int)`** method to `MainActivity` — [Step 4](04-show-hide-from-native.md).
- [ ] **Add `hideImeFromNative()`** method to `MainActivity` — [Step 4](04-show-hide-from-native.md).
- [ ] **Add `replaceImeTextFromNative(String, int, int)`** method to `MainActivity` (optional, only if Rust needs to overwrite text) — [Step 4](04-show-hide-from-native.md).
- [ ] **Native code** — route every existing `ANativeActivity_showSoftInput()` / `ANativeActivity_hideSoftInput()` call through `fcast_show_soft_input` / `fcast_hide_soft_input`, gated on `android_get_device_api_level() == 31 || == 32` — [Step 5](05-native-jni-wiring.md).
- [ ] **Native code** — implement the three JNI exports for `RustImeView` (`onTextStateNative`, `onImeKeyNative`, `onEditorActionNative`) — [Step 5](05-native-jni-wiring.md).

## Files NOT changed

- [ ] `app/src/main/AndroidManifest.xml` — verified compatible in [Step 1](01-manifest-check.md).
- [ ] `app/build.gradle` — no new dependencies for Route B.
- [ ] Any CMake / NDK build files — no native build-system changes for Route B.
- [ ] `ScreenCaptureService.java`, `MigrationRuntimeService.java`, `GstPopService.java` — untouched.
- [ ] Any existing JNI symbols on `MainActivity` (`nativeProcessFrame`, `nativeGraphCommand`, `nativeCaptureStarted`, etc.) — untouched.

## Verification on hardware

- [ ] Android 12 (API 31) device or emulator: `installAndroid12ImeBridge` log line appears at startup.
- [ ] On `showImeFromNative` call: `requestFocus=true` and `isAcceptingText=true` logs — [Step 6](06-runtime-sanity-checks.md) Check 1.
- [ ] `adb shell dumpsys input_method` shows `mInputShown=true` and `mServedView=…RustImeView…` — [Step 6](06-runtime-sanity-checks.md) Check 2.
- [ ] Typing in the IME produces `RustImeView` log lines (stub) or reaches the Rust handler (real impl) — [Step 6](06-runtime-sanity-checks.md) Check 3.
- [ ] Android 11 device: no `Android 12 IME bridge installed` log; legacy behavior unchanged — [Step 6](06-runtime-sanity-checks.md) Check 4.
- [ ] Android 13+ device: same as Android 11 — version gate keeps legacy path.
- [ ] Screen capture still works after the patch — [Step 6](06-runtime-sanity-checks.md) Check 5.

## Pre-merge cleanup

- [ ] Remove the temporary `postDelayed` `isAcceptingText` log from `showImeFromNative` — [Step 6](06-runtime-sanity-checks.md).
- [ ] Remove any debug call to `showImeFromNative` from `onCreate` (if you added one for local testing).
- [ ] If you used the stub variant of `RustImeView`, either flip the three methods to `native` (and confirm JNI registration), or document in the PR that JNI wiring is a follow-up.

## Rollback plan

If Android 12 reports come in worse than before:

1. Comment out the `if (Build.VERSION.SDK_INT == … S || == … S_V2)` branch in `MainActivity.onCreate()`. This leaves the editor classes in place but never installs the bridge.
2. Revert the native gate so `ANativeActivity_showSoftInput()` runs unconditionally.

These two changes restore exactly the pre-patch behavior without removing any code.

## If Route B is insufficient

Escalate to **Route A** ([Step 7](07-route-a-game-text-input.md)): add `androidx.games:games-text-input:4.0.0`, enable Prefab, replace `RustImeView` with `InputEnabledTextView`, and update CMake. The version gate, `MainActivity` class identity, manifest, and JNI symbol convention all carry over.

[← Previous: Step 8](08-non-goals-and-risks.md) · [Index](README.md)
