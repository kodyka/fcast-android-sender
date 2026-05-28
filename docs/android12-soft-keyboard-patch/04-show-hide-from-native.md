# Step 4 — `showImeFromNative(int)` and `hideImeFromNative()` on `MainActivity`

[← Previous: Step 3](03-rust-ime-view.md) · [Index](README.md) · [Next: Step 5 →](05-native-jni-wiring.md)

**Goal:** add two methods on `MainActivity` that the Rust/C++ side will call to show or hide the IME. They run on the UI thread and use `WindowInsetsController.show(Type.ime())` — the reliable Android 11+ path — after focusing the editor view from [Step 3](03-rust-ime-view.md).

## Imports to add to `MainActivity.java`

```java
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.inputmethod.InputMethodManager;
import android.text.InputType;
```

`android.os.Build` is already in scope via `android.os.*` (`MainActivity.java:27`).

## Full snippet — append to `MainActivity`

```java
/**
 * Called from native code on Android 12 / 12L to bring up the IME.
 * On every other Android version this is a no-op; the native side
 * should fall back to ANativeActivity_showSoftInput() in that case
 * (see Step 5).
 *
 * @param inputType one of android.text.InputType.* (e.g. TYPE_CLASS_TEXT = 1)
 */
@SuppressWarnings("unused") // called from JNI
private void showImeFromNative(int inputType) {
    if (imeView == null) {
        // We are not on Android 12 / 12L, or the bridge failed to install.
        // Do nothing; native code chose the wrong path.
        Log.w(TAG, "showImeFromNative: imeView null (SDK="
                + Build.VERSION.SDK_INT + "), ignoring");
        return;
    }

    runOnUiThread(() -> {
        imeView.setInputTypeValue(inputType);

        // Order is load-bearing: requestFocus() MUST happen before
        // WindowInsetsController.show(ime()). Without focus on the
        // editor target, show() either no-ops or attaches to the
        // plain NativeContentView (the broken case).
        boolean focused = imeView.requestFocus();
        Log.d(TAG, "showImeFromNative: requestFocus=" + focused);

        InputMethodManager imm = getSystemService(InputMethodManager.class);
        if (imm != null) {
            // Force the IME to (re)build its mirror with the new
            // EditorInfo we just supplied via setInputTypeValue().
            imm.restartInput(imeView);
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            WindowInsetsController c = imeView.getWindowInsetsController();
            if (c != null) {
                c.show(WindowInsets.Type.ime());
            } else {
                Log.w(TAG, "showImeFromNative: no WindowInsetsController");
            }
        } else if (imm != null) {
            // Defensive: we should not be here because imeView only
            // exists on S/S_V2, but keep a sane fallback anyway.
            imm.showSoftInput(imeView, InputMethodManager.SHOW_IMPLICIT);
        }
    });
}

/** Called from native code on Android 12 / 12L to dismiss the IME. */
@SuppressWarnings("unused") // called from JNI
private void hideImeFromNative() {
    if (imeView == null) return;

    runOnUiThread(() -> {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            WindowInsetsController c = imeView.getWindowInsetsController();
            if (c != null) {
                c.hide(WindowInsets.Type.ime());
            }
        } else {
            InputMethodManager imm = getSystemService(InputMethodManager.class);
            if (imm != null) {
                imm.hideSoftInputFromWindow(imeView.getWindowToken(), 0);
            }
        }
    });
}

/**
 * Optional: called from native code when the Rust side has changed the
 * text model and wants the IME to resync. After this, the IME will
 * re-call onCreateInputConnection() on imeView.
 */
@SuppressWarnings("unused") // called from JNI
private void replaceImeTextFromNative(String text, int selStart, int selEnd) {
    if (imeView == null) return;
    runOnUiThread(() -> {
        imeView.replaceState(text, selStart, selEnd);
        InputMethodManager imm = getSystemService(InputMethodManager.class);
        if (imm != null) imm.restartInput(imeView);
    });
}
```

## Why `WindowInsetsController.show(ime())` and not `showSoftInput()`

`InputMethodManager.showSoftInput()` can be silently ignored if it is called before the window has IME control — exactly the race that hits Android 12 during activity start. `WindowInsetsController.show()` documents that if the window does not yet control the inset, the request is queued and applied once control is gained. That removes the race and is what the Android docs explicitly recommend on 11+.

## Why `restartInput()` after `setInputTypeValue()`

`onCreateInputConnection()` only runs when the IME (re-)attaches. If the input type changed since the last attach, the IME would otherwise keep using the old `EditorInfo`. `restartInput()` forces the framework to call `onCreateInputConnection()` again so the new `inputType` actually takes effect.

## Default `inputType` to pass

For free-form text input, pass `android.text.InputType.TYPE_CLASS_TEXT` (integer value `1`). Common alternatives:

| Use case | Constant | Int value |
|---|---|---|
| Plain text | `TYPE_CLASS_TEXT` | `1` |
| URI / URL | `TYPE_CLASS_TEXT \| TYPE_TEXT_VARIATION_URI` | `1 \| 16 = 17` |
| Numeric | `TYPE_CLASS_NUMBER` | `2` |
| Password | `TYPE_CLASS_TEXT \| TYPE_TEXT_VARIATION_PASSWORD` | `1 \| 128 = 129` |

The native side picks the value; the Java method does not need to know which use case it is.

## Why `runOnUiThread`

`showImeFromNative` is intended to be called from JNI, which may originate on any thread. `requestFocus()`, `WindowInsetsController.show()`, and `InputMethodManager.restartInput()` all require the UI thread — calling them off-thread on Android 12 either throws or silently no-ops.

## Verification

After this step, with Steps 2 and 3 in place but before Step 5:

- Temporarily call `showImeFromNative(InputType.TYPE_CLASS_TEXT)` from a debug button or from `onCreate()` after a short delay (`postDelayed` on the UI handler) to confirm the keyboard appears.
- `adb shell dumpsys input_method | grep -i 'mInputShown\|mServedView'` should show `mInputShown=true` and `mServedView` pointing at `RustImeView@…`.

If the keyboard does not appear, the most common causes are:

1. `imeView.requestFocus()` returned `false` — likely missing `focusableInTouchMode` (see [Step 2](02-install-ime-bridge.md)).
2. `WindowInsetsController` was `null` — the view is not yet attached to a window; happens if the call fires before `onAttachedToWindow`. Defer the test call until after the activity is fully resumed.

[← Previous: Step 3](03-rust-ime-view.md) · [Index](README.md) · [Next: Step 5 →](05-native-jni-wiring.md)
