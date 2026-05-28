# Step 7 — Route A: standalone `GameTextInput` (escape hatch)

[← Previous: Step 6](06-runtime-sanity-checks.md) · [Index](README.md) · [Next: Step 8 →](08-non-goals-and-risks.md)

**Adopt this only if Route B (Steps 1–6) turns out to be insufficient** — typically when you need any of:

- IME composition regions (multi-key characters, CJK input)
- Spell-checking / completions surfaced by the IME
- Action-key handling beyond plain text
- Full-screen IME control in landscape

The research recommends standalone `GameTextInput` as the most robust no-`GameActivity` option. It’s an AndroidX library that comes as an AAR with Prefab/CMake integration for native linkage.

## Dependency changes

### `app/build.gradle`

Append to the `dependencies` block:

```groovy
dependencies {
    // existing entries unchanged
    implementation libs.material
    testImplementation libs.junit
    androidTestImplementation libs.ext.junit
    androidTestImplementation libs.espresso.core
    implementation "com.journeyapps:zxing-android-embedded:4.3.0"

    // Android 12 / 12L soft-keyboard fix via standalone GameTextInput.
    // 4.0.0 is the lowest version that ships the standalone-use fixes
    // (3.0.2 added the fix; 4.0.0 added further keyboard stability fixes).
    implementation "androidx.games:games-text-input:4.0.0"
}
```

Also enable Prefab so the AAR’s native artifacts are visible to CMake:

```groovy
android {
    // ... existing config ...

    buildFeatures {
        viewBinding true
        prefab true
    }
}
```

### Native CMake link

Wherever the `fcastsender` shared library target is defined:

```cmake
find_package(game-text-input REQUIRED CONFIG)
target_link_libraries(fcastsender PRIVATE game-text-input::game-text-input)
```

This gives the native side access to `GameTextInput_*` C functions so Rust can read/write text state without going through the slow Java path.

## Replacement editor view — `InputEnabledTextView.java`

`RustImeView` from [Step 3](03-rust-ime-view.md) is replaced by an editor that delegates to `com.google.androidgamesdk.gametextinput.InputConnection`:

```java
package org.fcast.android.sender;

import static android.view.inputmethod.EditorInfo.IME_ACTION_NONE;
import static android.view.inputmethod.EditorInfo.IME_FLAG_NO_FULLSCREEN;

import android.content.Context;
import android.util.AttributeSet;
import android.view.View;
import android.view.inputmethod.EditorInfo;

import androidx.core.graphics.Insets;

import com.google.androidgamesdk.gametextinput.GameTextInput;
import com.google.androidgamesdk.gametextinput.Listener;
import com.google.androidgamesdk.gametextinput.Settings;
import com.google.androidgamesdk.gametextinput.State;

public final class InputEnabledTextView extends View implements Listener {

    public com.google.androidgamesdk.gametextinput.InputConnection bridgeConnection;

    public InputEnabledTextView(Context context) {
        super(context);
        init();
    }

    public InputEnabledTextView(Context context, AttributeSet attrs) {
        super(context, attrs);
        init();
    }

    private void init() {
        setFocusable(true);
        setFocusableInTouchMode(true);
    }

    public void createBridge(int inputType) {
        EditorInfo info = new EditorInfo();
        info.inputType  = inputType;
        info.actionId   = IME_ACTION_NONE;
        info.imeOptions = IME_FLAG_NO_FULLSCREEN;

        bridgeConnection =
                new com.google.androidgamesdk.gametextinput.InputConnection(
                        getContext(),
                        this,
                        new Settings(info, true))
                        .setListener(this);
    }

    @Override
    public boolean onCheckIsTextEditor() {
        return true;
    }

    @Override
    public android.view.inputmethod.InputConnection onCreateInputConnection(
            EditorInfo outAttrs) {
        if (bridgeConnection == null || !bridgeConnection.getSoftKeyboardActive()) {
            return null;
        }
        GameTextInput.copyEditorInfo(bridgeConnection.getEditorInfo(), outAttrs);
        return bridgeConnection;
    }

    @Override
    public void stateChanged(State newState, boolean dismissed) {
        onTextStateNative(
                newState.text,
                newState.selectionStart, newState.selectionEnd,
                newState.composingRegionStart, newState.composingRegionEnd,
                dismissed);
    }

    @Override
    public void onEditorAction(int action) {
        onEditorActionNative(action);
    }

    @Override
    public void onImeInsetsChanged(Insets insets) {
        // Optional: forward insets to native if you want UI re-layout.
    }

    @Override
    public void onSoftwareKeyboardVisibilityChanged(boolean visible) {
        onImeVisibilityNative(visible);
    }

    private native void onTextStateNative(String text,
                                          int selStart, int selEnd,
                                          int compStart, int compEnd,
                                          boolean dismissed);
    private native void onEditorActionNative(int action);
    private native void onImeVisibilityNative(boolean visible);
}
```

## Updated `MainActivity` snippets

Replace [Step 2](02-install-ime-bridge.md)’s installer with the GameTextInput variant:

```java
private InputEnabledTextView imeView;

private void installAndroid12ImeBridge() {
    FrameLayout root = findViewById(android.R.id.content);
    if (root == null) return;

    imeView = new InputEnabledTextView(this);
    imeView.setAlpha(0f);
    imeView.createBridge(android.text.InputType.TYPE_CLASS_TEXT);

    FrameLayout.LayoutParams lp = new FrameLayout.LayoutParams(
            1, 1, Gravity.BOTTOM | Gravity.START);
    root.addView(imeView, lp);

    // Hand the bridge connection to native code if it wants direct access.
    setInputConnectionNative(imeView.bridgeConnection);
}

private native void setInputConnectionNative(
        com.google.androidgamesdk.gametextinput.InputConnection connection);
```

Replace [Step 4](04-show-hide-from-native.md)’s `showImeFromNative` body so it updates the bridge’s `EditorInfo` before showing:

```java
private void showImeFromNative(int inputType) {
    if (imeView == null) return;
    runOnUiThread(() -> {
        EditorInfo info = new EditorInfo();
        info.inputType  = inputType;
        info.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN;
        imeView.bridgeConnection.setEditorInfo(info);

        imeView.requestFocus();
        getSystemService(InputMethodManager.class).restartInput(imeView);

        WindowInsetsControllerCompat c =
                WindowCompat.getInsetsController(getWindow(), imeView);
        if (c != null) c.show(WindowInsetsCompat.Type.ime());
    });
}
```

Imports for the `Compat` helpers:

```java
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;
```

## Which version to pin

- **`4.0.0`** — recommended. Stable; ships fixes for use without `GameActivity` (introduced in `3.0.2`) plus additional keyboard stability fixes.
- **`4.3.0-alpha01`** — newer but alpha; avoid for production unless you specifically need a fix landed there.
- Anything **< `3.0.2`** — known broken when used without `GameActivity`; do not use.

## What carries over from Route B

- The version gate on `Build.VERSION_CODES.S` / `S_V2` from [Step 2](02-install-ime-bridge.md).
- The post-`super.onCreate()` insertion point.
- `MainActivity`’s class identity (`extends NativeActivity`) and the `android.app.lib_name` metadata.
- The JNI symbol convention from [Step 5](05-native-jni-wiring.md) — just adjust the class name segment (`InputEnabledTextView` instead of `RustImeView`).

## What you give up by adopting Route A

- A new transitive dependency on AndroidX games-text-input and Prefab build configuration.
- A slightly larger APK (≈100 KB native + Java).
- Build complexity: CMake must find the AAR’s Prefab package. If your build does not currently invoke CMake (Rust-only NDK builds via `cargo-ndk` or similar), you must add a thin CMakeLists.txt that links the prefab module and exposes it to Rust via `bindgen`.

If none of those are blockers, Route A is the lowest-edge-case option.

[← Previous: Step 6](06-runtime-sanity-checks.md) · [Index](README.md) · [Next: Step 8 →](08-non-goals-and-risks.md)
