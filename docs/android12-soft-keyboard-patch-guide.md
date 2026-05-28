# Android 12 soft-keyboard patch guide for `MainActivity` (`NativeActivity`)

**Status:** guide only — no code in this repository is modified by this document.
**Scope:** Android 12 (API 31, `Build.VERSION_CODES.S`) and Android 12L (API 32, `Build.VERSION_CODES.S_V2`).
**Target file when implemented:** `app/src/main/java/org/fcast/android/sender/MainActivity.java` (currently extends `android.app.NativeActivity`).
**Source:** distilled from `research-android12-activity-keyboard.md`. See that file for upstream documentation references.

This guide explains, step by step, how to add a working soft-keyboard path to the existing `NativeActivity`-based `MainActivity` so that text input works on Android 12 phones, without migrating to `GameActivity`. Other Android versions are left on the existing code path.

---

## 1. Why the keyboard is broken on Android 12 here

`MainActivity extends NativeActivity` (see `app/src/main/java/org/fcast/android/sender/MainActivity.java:204`). Stock `NativeActivity` installs an internal `NativeContentView` — a plain `View`, not a text editor — as its content view and focuses it.

That has two consequences on Android 12:

1. `ANativeActivity_showSoftInput()` (the NDK helper) calls `InputMethodManager.showSoftInput()` against that plain `View`. Even when the IME does appear, the focused target has **no `InputConnection`**, so `InputMethodManager.isAcceptingText()` is `false` and modern IMEs route text only through `InputConnection`, not raw key events. Default soft keyboards do not generate `KeyEvent`s on Jelly Bean+.
2. `showSoftInput()` itself is timing-sensitive during activity start; on Android 11+ the reliable API is `WindowInsetsController.show(Type.ime())`, scheduled after window focus is established.

So a complete Android-12 fix needs **both**:

- A real editor target with `onCheckIsTextEditor() = true` and a non-null `InputConnection`.
- `requestFocus()` on that editor, then `WindowInsetsController.show(ime())`.

Replacing only the show-IME API is not enough.

---

## 2. Strategy: version-gated overlay editor, no `GameActivity` migration

The patch keeps `MainActivity extends NativeActivity` and the existing native entry point (`android.app.lib_name = fcastsender`, see `AndroidManifest.xml:57-58`). It adds a tiny invisible editor `View` to `android.R.id.content` **only on Android 12 / 12L**, and routes IME show/hide through that view.

Why version-gate:

- Other versions (lower or higher) keep the current behavior — no regression risk in code paths that currently work.
- The bug pattern described in your research is most acute on Android 12’s window-focus timing; restricting the change to `S`/`S_V2` keeps blast radius minimal.

Two implementation routes; pick one:

| Route | Dependency | Pros | Cons |
|---|---|---|---|
| **A. Standalone `GameTextInput`** | `androidx.games:games-text-input:4.0.0` (+ Prefab/CMake link from native) | Handles composition, selection, IME insets, action keys; fewer edge cases | Adds AndroidX dependency; requires native CMake change if native code wants the state |
| **B. `BaseInputConnection` Kotlin/Java view** | none | Zero new dependencies | You own composition + selection + restartInput plumbing |

Recommendation for this repo: **Route B** for the first landing. The app already has plenty of native↔Java glue; we don’t need composition-aware features yet, and avoiding a new AndroidX dep keeps the build simple. Route A is documented below as an escape hatch if Route B turns out to need composing-text behavior.

---

## 3. Step-by-step plan

### Step 1 — Confirm the manifest is compatible

`app/src/main/AndroidManifest.xml:48` already declares:

```xml
android:windowSoftInputMode="adjustResize"
```

That is acceptable. Do **not** add `stateHidden` or `stateAlwaysHidden` — those override programmatic `show(ime())` during startup on Android 12. No manifest change is required for Route B.

If you adopt Route A and add a dedicated subclass activity, the manifest fragment would be:

```xml
<activity
    android:name=".MainActivity"
    android:configChanges="keyboardHidden|orientation|screenSize"
    android:windowSoftInputMode="stateUnspecified|adjustResize"
    android:resizeableActivity="true"
    android:exported="true">
    <!-- existing intent-filter and lib_name meta-data unchanged -->
</activity>
```

Note: `MainActivity` is already a `NativeActivity` subclass (`MainActivity.java:204`), so the “subclass instead of `android.app.NativeActivity`” requirement from the research is already satisfied. The `android.app.lib_name = fcastsender` metadata stays as-is.

### Step 2 — Add an invisible editor view, gated on Android 12 / 12L

Inside `MainActivity.onCreate()` (currently at `MainActivity.java:326`), **after** `super.onCreate(savedInstanceState)` (the research is explicit: customize only after `super.onCreate()` returns, because `NativeActivity.onCreate()` installs the content view and loads the native library), append the editor only on Android 12:

```java
private RustImeView imeView; // Android 12 / 12L only; null on other versions

@Override
protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);
    // ... existing GStreamer.init, Discoverer, etc. ...

    if (Build.VERSION.SDK_INT == Build.VERSION_CODES.S
            || Build.VERSION.SDK_INT == Build.VERSION_CODES.S_V2) {
        installAndroid12ImeBridge();
    }
}

private void installAndroid12ImeBridge() {
    FrameLayout root = findViewById(android.R.id.content);
    imeView = new RustImeView(this);
    imeView.setAlpha(0f);                // invisible
    imeView.setFocusable(true);
    imeView.setFocusableInTouchMode(true);

    FrameLayout.LayoutParams lp =
            new FrameLayout.LayoutParams(1, 1, Gravity.BOTTOM | Gravity.START);
    root.addView(imeView, lp);
}
```

Key invariants:

- Size `1x1`, alpha `0` — not visible, not interactive for the user.
- Must be a child of `android.R.id.content`, so it sits inside the same `FrameLayout` that `NativeActivity` already installed (so it does not displace the native render surface).
- `focusable=true` and `focusableInTouchMode=true` are required, otherwise `requestFocus()` is a no-op when the IME tries to open.

### Step 3 — Implement `RustImeView` (the editor)

The whole point is `onCheckIsTextEditor() = true` plus a real `InputConnection` backed by an `Editable`. Place this as a new file `app/src/main/java/org/fcast/android/sender/RustImeView.java`:

```java
package org.fcast.android.sender;

import android.content.Context;
import android.text.Editable;
import android.text.InputType;
import android.text.Selection;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;

public final class RustImeView extends View {
    private int inputTypeValue = InputType.TYPE_CLASS_TEXT;
    private final Editable editable =
            Editable.Factory.getInstance().newEditable("");

    public RustImeView(Context context) {
        super(context);
        setFocusable(true);
        setFocusableInTouchMode(true);
        Selection.setSelection(editable, 0);
    }

    public void setInputTypeValue(int v) { inputTypeValue = v; }

    @Override
    public boolean onCheckIsTextEditor() { return true; }

    @Override
    public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
        outAttrs.inputType   = inputTypeValue;
        outAttrs.imeOptions  = EditorInfo.IME_FLAG_NO_FULLSCREEN
                             | EditorInfo.IME_ACTION_NONE;
        outAttrs.initialSelStart = Selection.getSelectionStart(editable);
        outAttrs.initialSelEnd   = Selection.getSelectionEnd(editable);

        return new BaseInputConnection(this, true) {
            @Override public Editable getEditable() { return editable; }

            @Override
            public boolean commitText(CharSequence text, int newCursorPosition) {
                boolean ok = super.commitText(text, newCursorPosition);
                publishState();
                return ok;
            }

            @Override
            public boolean setComposingText(CharSequence text, int newCursorPosition) {
                boolean ok = super.setComposingText(text, newCursorPosition);
                publishState();
                return ok;
            }

            @Override
            public boolean finishComposingText() {
                boolean ok = super.finishComposingText();
                publishState();
                return ok;
            }

            @Override
            public boolean deleteSurroundingText(int before, int after) {
                boolean ok = super.deleteSurroundingText(before, after);
                publishState();
                return ok;
            }

            @Override
            public boolean sendKeyEvent(KeyEvent event) {
                if (event.getAction() == KeyEvent.ACTION_DOWN) {
                    onImeKeyNative(event.getKeyCode(), event.getUnicodeChar());
                }
                return super.sendKeyEvent(event);
            }

            @Override
            public boolean performEditorAction(int actionCode) {
                onEditorActionNative(actionCode);
                return true;
            }
        };
    }

    public void replaceState(String text, int selStart, int selEnd) {
        editable.replace(0, editable.length(), text);
        int len = editable.length();
        Selection.setSelection(editable,
                Math.max(0, Math.min(selStart, len)),
                Math.max(0, Math.min(selEnd, len)));
        publishState();
    }

    private void publishState() {
        int selStart = Selection.getSelectionStart(editable);
        int selEnd   = Selection.getSelectionEnd(editable);
        int compStart = BaseInputConnection.getComposingSpanStart(editable);
        int compEnd   = BaseInputConnection.getComposingSpanEnd(editable);

        InputMethodManager imm =
                getContext().getSystemService(InputMethodManager.class);
        if (imm != null) {
            imm.updateSelection(this, selStart, selEnd, compStart, compEnd);
        }
        onTextStateNative(editable.toString(), selStart, selEnd, compStart, compEnd);
    }

    // Implement these in Rust JNI (or stub them initially and just log)
    private native void onTextStateNative(String text,
                                          int selStart, int selEnd,
                                          int compStart, int compEnd);
    private native void onImeKeyNative(int keyCode, int unicodeChar);
    private native void onEditorActionNative(int actionCode);
}
```

Why each part exists (these come straight from Android’s editor contract — see research §“Why changing only the show-keyboard API is not enough” and the custom-editor docs cited there):

- `onCheckIsTextEditor()` — declares this view as a text editor, so the framework treats it as a valid IME target.
- `BaseInputConnection` with a non-`null` `Editable` — `isAcceptingText()` becomes `true`; modern IMEs deliver text.
- `initialSelStart` / `initialSelEnd` set in `EditorInfo` — required so the IME knows the starting selection.
- `updateSelection()` whenever text changes — keeps the IME’s view of the model coherent without a full `restartInput()` round trip.

### Step 4 — Show and hide the IME from the existing call sites

The current `MainActivity` does not yet expose a “show keyboard” method to native code. Add two methods, called by JNI when the Rust side needs text entry:

```java
// Called from native code (only meaningful on Android 12 / 12L)
private void showImeFromNative(int inputType) {
    if (imeView == null) {
        // Other Android versions: fall back to the legacy NDK path
        // (ANativeActivity_showSoftInput from C++) — do nothing here.
        return;
    }
    runOnUiThread(() -> {
        imeView.setInputTypeValue(inputType);
        imeView.requestFocus();

        InputMethodManager imm = getSystemService(InputMethodManager.class);
        if (imm != null) imm.restartInput(imeView);

        // WindowInsetsController is the reliable show-IME path on 11+.
        // It defers until the window has IME control, which is exactly
        // what fixes the Android 12 timing race.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            WindowInsetsController c = imeView.getWindowInsetsController();
            if (c != null) c.show(WindowInsets.Type.ime());
        } else {
            imm.showSoftInput(imeView, InputMethodManager.SHOW_IMPLICIT);
        }
    });
}

private void hideImeFromNative() {
    if (imeView == null) return;
    runOnUiThread(() -> {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            WindowInsetsController c = imeView.getWindowInsetsController();
            if (c != null) c.hide(WindowInsets.Type.ime());
        } else {
            InputMethodManager imm = getSystemService(InputMethodManager.class);
            if (imm != null) imm.hideSoftInputFromWindow(imeView.getWindowToken(), 0);
        }
    });
}
```

Ordering matters: `requestFocus()` **before** `show(ime())`. Without focus on a real editor target, `show()` either no-ops or shows against the plain `NativeContentView` (i.e., back to the broken case).

### Step 5 — Wire the JNI calls

On the Rust/C++ side that drives `MainActivity`, replace the existing keyboard request — wherever it currently uses `ANativeActivity_showSoftInput()` — with a JNI call to `showImeFromNative(int)` on the `MainActivity` instance, **only on Android 12 / 12L**. Pseudocode:

```c
// In the native bridge that today calls ANativeActivity_showSoftInput(activity, 0):
int sdk = android_get_device_api_level();
if (sdk == 31 /* S */ || sdk == 32 /* S_V2 */) {
    // Call MainActivity#showImeFromNative(int inputType) via JNI.
    // inputType: pass android.text.InputType.TYPE_CLASS_TEXT == 1 unless
    // the call site needs e.g. TYPE_TEXT_VARIATION_URI.
    call_void_method(env, activity_obj, "showImeFromNative", "(I)V", /*TYPE_CLASS_TEXT*/ 1);
} else {
    ANativeActivity_showSoftInput(activity, 0);
}
```

For receiving text from the IME back into Rust, implement the three `native` methods declared in `RustImeView` (`onTextStateNative`, `onImeKeyNative`, `onEditorActionNative`). Until the Rust side is ready, stub them as empty methods on the Java side and the keyboard will still appear correctly — you just won’t see text events yet.

### Step 6 — Sanity-check at runtime

Two quick checks to verify the patch is working on a real Android 12 device:

1. Add a one-shot log immediately after `show(ime())`:

   ```java
   InputMethodManager imm = getSystemService(InputMethodManager.class);
   Log.i(TAG, "ime.isAcceptingText=" + (imm != null && imm.isAcceptingText()));
   ```

   It should print `true`. If `false`, the focused view has no `InputConnection` — re-check that `imeView.requestFocus()` ran before `show(ime())` and that the view is attached to the window.

2. Type a character in the keyboard. `onTextStateNative` should fire with the new text. If the keyboard shows but characters do not arrive, it is almost always because `getEditable()` returned `null` or `onCheckIsTextEditor()` was not overridden.

---

## 4. Route A: standalone `GameTextInput` (only if Route B is insufficient)

Adopt this only if you later need IME composition regions, spell-checking, completions, or full-screen IME control. It is also the most robust option per the research.

`app/build.gradle` dependency addition:

```groovy
dependencies {
    // existing entries unchanged
    implementation "androidx.games:games-text-input:4.0.0"
}
```

Native CMake addition (where `fcastsender`’s native target is defined):

```cmake
find_package(game-text-input REQUIRED CONFIG)
target_link_libraries(fcastsender PRIVATE game-text-input::game-text-input)
```

Then replace `RustImeView` with an `InputEnabledTextView` that delegates to `com.google.androidgamesdk.gametextinput.InputConnection`. The full snippet is in `research-android12-activity-keyboard.md` (`InputEnabledTextView.java` and `NativeImeActivity.java`). The same Step 2 / Step 4 gating on `Build.VERSION_CODES.S` / `S_V2` applies; nothing about the manifest, `MainActivity` class identity, or the native entry point needs to change.

Pin to `4.0.0`: the research notes that `3.0.2` first fixed standalone-use bugs and `4.0.0` adds further IME stability fixes; `4.3.0-alpha01` exists but is alpha.

---

## 5. What this guide deliberately does **not** change

- The native render path, EGL/GLES setup, `MediaProjection`, and the GStreamer bridge are untouched.
- `ScreenCaptureService`, `MigrationRuntimeService`, `GstPopService` are untouched.
- `MainActivity.onCreate()`’s existing ordering (`GStreamer.init`, `Discoverer`, `mediaProjectionManager`, `glThread`, `LocalBroadcastManager`, `displayManager`, `POST_NOTIFICATIONS` permission) is preserved — the IME bridge is appended at the end of `onCreate()`.
- All non-Android-12 code paths keep the current `ANativeActivity_showSoftInput()` behavior.

---

## 6. Risks and follow-ups

- **Focus stealing.** The 1×1 invisible `RustImeView` is focusable. If anything in the app calls `findFocus()` or relies on the `NativeContentView` being focused, that view may now answer instead. Mitigation: only focus the editor inside `showImeFromNative()`, and consider calling `nativeContentView.requestFocus()` after `hideImeFromNative()` if back-button or key event handling regresses.
- **Configuration changes.** `MainActivity` declares `configChanges="keyboardHidden|orientation|screenSize"` (`AndroidManifest.xml:47`), so the activity is not recreated. The `imeView` field survives; no extra state restoration is needed.
- **Other Android versions.** If reports come in that Android 13+ also misbehaves with the legacy NDK path, broaden the gate from `== S/S_V2` to `>= S`. Do not broaden it speculatively — the current `ANativeActivity_showSoftInput()` path is what the rest of the app has been running against.
- **Native callbacks.** The three `native` methods on `RustImeView` must be registered on the Rust side (either via `RegisterNatives` or `Java_org_fcast_android_sender_RustImeView_*` symbols). Stub them as empty Java methods first to land the visible-keyboard fix independently.

---

## 7. Implementation checklist

When you actually land the patch, this is the smallest viable set of changes:

- [ ] Add `RustImeView.java` (Step 3).
- [ ] Add `imeView` field + `installAndroid12ImeBridge()` to `MainActivity` and call it from `onCreate()` gated on `S` / `S_V2` (Step 2).
- [ ] Add `showImeFromNative(int)` and `hideImeFromNative()` to `MainActivity` (Step 4).
- [ ] In native code, route the “open keyboard” request through `showImeFromNative` only when `android_get_device_api_level() == 31 || == 32` (Step 5).
- [ ] Smoke-test `isAcceptingText()` and a character round-trip on an Android 12 phone (Step 6).
- [ ] No manifest, dependency, or CMake changes required for Route B.
