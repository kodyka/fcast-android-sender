# Step 6 — Runtime sanity checks on an Android 12 device

[← Previous: Step 5](05-native-jni-wiring.md) · [Index](README.md) · [Next: Step 7 →](07-route-a-game-text-input.md)

**Goal:** prove on a real Android 12 (API 31) or Android 12L (API 32) phone that the patch produces (a) a visible IME, (b) `isAcceptingText() == true`, and (c) characters round-trip into your Rust handler.

A device emulator running API 31/32 is acceptable for the first two checks. The third is most meaningful on a real device with a real third-party IME (Gboard, SwiftKey).

## Check 1 — `isAcceptingText()` is `true`

Temporarily add this one-shot log inside `showImeFromNative()` (Step 4), immediately after `c.show(WindowInsets.Type.ime())`:

```java
imeView.postDelayed(() -> {
    InputMethodManager imm2 = getSystemService(InputMethodManager.class);
    Log.i(TAG, "isAcceptingText=" + (imm2 != null && imm2.isAcceptingText()));
}, 250);
```

Expected output in `adb logcat -s MainActivity`:

```
I MainActivity: showImeFromNative: requestFocus=true
I MainActivity: isAcceptingText=true
```

If you see `isAcceptingText=false`:

| Symptom | Most likely cause | Fix |
|---|---|---|
| `requestFocus=false` | `RustImeView` not focusable in touch mode | Re-check [Step 2](02-install-ime-bridge.md) — both `setFocusable(true)` and `setFocusableInTouchMode(true)` must be set. |
| `requestFocus=true`, `isAcceptingText=false` | `onCheckIsTextEditor()` not overridden, or `onCreateInputConnection()` returned `null` | Re-check [Step 3](03-rust-ime-view.md) — `onCheckIsTextEditor` must return `true`, and the inner `BaseInputConnection` must not be `null`. |
| Both `true` but no keyboard | `windowSoftInputMode` was changed to `stateHidden` | Revert manifest per [Step 1](01-manifest-check.md). |

## Check 2 — IME actually appears

Use `dumpsys` to see what the system thinks:

```sh
adb shell dumpsys input_method | grep -E 'mInputShown|mServedView|mCurClient'
```

Expected (lines may vary by Android 12 build):

```
  mInputShown=true
  mServedView=org.fcast.android.sender.RustImeView{... VFED.... ...}
  mCurClient=...
```

Key indicators:

- `mInputShown=true` — the system has shown the IME.
- `mServedView` is a `RustImeView` — the editor is the focused target. If it’s `NativeContentView`, the IME is attached to the broken view and `isAcceptingText` will be `false`.

You can also watch the IME window directly:

```sh
adb shell dumpsys window | grep -A1 InputMethod
```

## Check 3 — characters round-trip into Rust (or stub)

With the stub `RustImeView` from [Step 3](03-rust-ime-view.md):

```sh
adb logcat -s RustImeView
```

Then type, for example, `hello` on the IME. Expected:

```
D RustImeView: text=h sel=[1,1] comp=[-1,-1]
D RustImeView: text=he sel=[2,2] comp=[-1,-1]
D RustImeView: text=hel sel=[3,3] comp=[-1,-1]
...
```

(Some IMEs use composing regions: you may see non-`-1` `comp=[…]` until the user accepts a suggestion.)

If you see the keyboard but no `RustImeView` logs at all, the JNI native methods are unimplemented and throwing `UnsatisfiedLinkError`. Filter for it:

```sh
adb logcat *:E | grep -i UnsatisfiedLink
```

Switch to the stub variant in [Step 3](03-rust-ime-view.md) (no `native` keyword on the three callbacks) until the JNI side from [Step 5](05-native-jni-wiring.md) is in place.

## Check 4 — non-Android-12 devices are unchanged

On any Android 11 or Android 13+ device, the version gate keeps the legacy `ANativeActivity_showSoftInput()` path:

- Confirm `Log.i(TAG, "Android 12 IME bridge installed ...")` does **not** appear in logcat.
- Confirm the previous IME behavior is observed (whatever it was).

## Check 5 — `MediaProjection` / screen-capture path unaffected

Trigger the existing screen-capture flow once (Cast button → permission dialog → start capture). It should behave identically to `main`:

- `mediaProjectionManager.createScreenCaptureIntent()` opens normally.
- `ScreenCaptureService` starts.
- Native frames flow through `nativeProcessFrame`.

If anything regresses here, the editor view is competing for focus with the system UI dialog. Mitigation: do not call `showImeFromNative()` from inside the projection-result handler; only call it after `onActivityResult` has fully returned and the projection callback has fired.

## Quick teardown commands

After verification, remove the temporary `postDelayed` log from [Check 1](#check-1--isacceptingtext-is-true) so production logs stay clean.

[← Previous: Step 5](05-native-jni-wiring.md) · [Index](README.md) · [Next: Step 7 →](07-route-a-game-text-input.md)
