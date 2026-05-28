# NativeActivity soft keyboard patch for Android 12 without GameActivity

If your app stays on `NativeActivity`, the fix is not simply “show the keyboard with a newer API”. The deeper problem is that the stock `NativeActivity` surface is **not a real text editor target**. The most reliable no-`GameActivity` patch is therefore to keep `NativeActivity`, subclass it in Java/Kotlin, and add a tiny focused editor view that exposes a valid `InputConnection`. Google now documents two viable ways to do that without `GameActivity`: an **official standalone `GameTextInput` integration**, and a **custom editor view built on `BaseInputConnection`**. citeturn33view0turn30view0turn5view0

## Where the failure really comes from

In current AOSP, `NativeActivity` creates an internal `NativeContentView` that is just a plain `View`, sets it as the content view, requests focus on it, and uses that view when it asks the IME to show. The NDK wrapper `ANativeActivity_showSoftInput()` is explicitly documented as calling `InputMethodManager.showSoftInput()` for the activity. In other words, stock `NativeActivity` does try to show the soft keyboard, but it does so for a plain `View`, not for a text editor view. citeturn31view0turn2view0

Android’s own IME docs explain why that is not enough. A real editor is expected to expose an `InputConnection` from `View.onCreateInputConnection()`, and Android’s custom-editor guidance says a custom text editor should return `true` from `onCheckIsTextEditor()`. `InputMethodManager.isAcceptingText()` also states that if the currently served view has **no input connection**, it can only handle **raw key events**. That matters because the `KeyEvent` docs are explicit: you should **not** rely on soft keyboards to generate key events, and the default software keyboard will never send key events to apps targeting Jelly Bean or later. Modern IMEs primarily talk through `InputConnection`, not through raw keycodes. citeturn15view0turn15view1turn15view2turn29view0turn5view0turn2view4

That is the key refinement to your research: on Android 12, the issue is not only that the keyboard might refuse to appear. Even if you make it appear, **text input still will not behave correctly** until the focused target is an editor with a valid `InputConnection`. This is an inference from Android’s documented editor contract plus the way `NativeActivity` is implemented. citeturn31view0turn15view0turn29view0turn2view4

## Why changing only the show-keyboard API is not enough

Android’s input-method visibility guide says that `InputMethodManager.showSoftInput()` can fail to make the keyboard visible during activity start because the view may not yet satisfy the conditions needed to connect to the IME. The same guide recommends `WindowInsetsControllerCompat.show(Type.ime())` as the reliable approach because it is scheduled after window focus is established, and its example explicitly calls `requestFocus()` before showing the IME. The platform `WindowInsetsController.show()` docs say that if the window does not yet control that inset type, the request will be applied once control is gained. citeturn15view3turn15view4turn15view5turn2view2

That is why a pure “replace `showSoftInput()` with `WindowInsetsController.show(ime())`” patch is only half a fix. It improves **IME visibility timing**, but it does **not** create the missing editor connection on its own. For `NativeActivity`, you need both pieces together: a proper editor view **and** a reliable IME-show call after focus. citeturn15view3turn15view5turn15view0turn29view0

One more detail matters on Android 12 and later: the same visibility guide warns that if you call `show(ime())` during startup, `SOFT_INPUT_STATE_HIDDEN` or `SOFT_INPUT_STATE_ALWAYS_HIDDEN` can suppress the keyboard unexpectedly. `NativeActivity` itself sets `SOFT_INPUT_STATE_UNSPECIFIED | SOFT_INPUT_ADJUST_RESIZE`, so if you changed `windowSoftInputMode` in the manifest, make sure you did not override that with a hidden state. citeturn15view6turn31view0

## The best official no-GameActivity fix

Google’s current documentation no longer forces an all-or-nothing choice between `NativeActivity` and `GameActivity`. The `GameTextInput` guide explicitly says the library can be used **as a standalone library**, not only through `GameActivity`, and describes it as a simpler alternative to implementing your own full-screen soft-keyboard integration. It supports showing and hiding the keyboard, edited text state, selection, composing regions, spell-checking, completions, and multi-key characters. The guide also shows an `InputEnabledTextView` Java class that exposes an `InputConnection` and forwards state changes to native code. citeturn33view0turn21view0

That makes standalone `GameTextInput` the strongest official patch if you want to **stay on `NativeActivity`**. Android’s old NDK documentation also notes that it is valid to **subclass `NativeActivity`** and name that subclass in the manifest instead of `android.app.NativeActivity`. That is exactly what you need here: keep the native lifecycle, but insert a tiny Java/Kotlin editor bridge into the view hierarchy. citeturn34view0

The AndroidX release notes make this path even more attractive. They say version **3.0.2** fixed a bug that prevented `GameTextInput` from being used without `GameActivity`, and version **4.0.0** added further stability and keyboard fixes. As of the AndroidX release page, **4.0.0** is the latest stable release listed, while **4.3.0-alpha01** is newer but alpha. citeturn30view0

A typical dependency setup for the official standalone route looks like this:

```groovy
dependencies {
    implementation "androidx.games:games-text-input:4.0.0"
}
```

```cmake
find_package(game-text-input REQUIRED CONFIG)
target_link_libraries(your_native_target PRIVATE game-text-input::game-text-input)
```

Android’s `GameTextInput` guide also notes that the library ships as an AAR with Prefab/CMake integration, so this route is designed for native apps rather than being a Java-only workaround. citeturn33view0turn30view0

## Java patch for a NativeActivity subclass

The cleanest Java patch is to subclass `NativeActivity`, let `super.onCreate()` complete, then add a tiny editor bridge view into `android.R.id.content`. That matches the way `NativeActivity` is implemented internally: it installs its own content view and loads the native library during `onCreate()`, so the safest customisation point is **after** `super.onCreate()` returns. This section adapts Google’s standalone `GameTextInput` sample to that `NativeActivity` model, and adds `onCheckIsTextEditor()` because Android’s custom-editor docs say custom text editors should report themselves as text editors. citeturn31view0turn25view0turn25view1turn5view0turn34view0

**Manifest fragment**

```xml
<activity
    android:name=".NativeImeActivity"
    android:exported="true"
    android:windowSoftInputMode="stateUnspecified|adjustResize">

    <meta-data
        android:name="android.app.lib_name"
        android:value="main" />

    <meta-data
        android:name="android.app.func_name"
        android:value="ANativeActivity_onCreate" />
</activity>
```

**`InputEnabledTextView.java`**

```java
package com.example.nativeime;

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
        info.inputType = inputType;
        info.actionId = IME_ACTION_NONE;
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
                newState.selectionStart,
                newState.selectionEnd,
                newState.composingRegionStart,
                newState.composingRegionEnd,
                dismissed);
    }

    @Override
    public void onEditorAction(int action) {
        onEditorActionNative(action);
    }

    @Override
    public void onImeInsetsChanged(Insets insets) {
        // Optional: forward insets to native if you need UI re-layout.
    }

    @Override
    public void onSoftwareKeyboardVisibilityChanged(boolean visible) {
        onImeVisibilityNative(visible);
    }

    private native void onTextStateNative(
            String text,
            int selStart,
            int selEnd,
            int compStart,
            int compEnd,
            boolean dismissed);

    private native void onEditorActionNative(int action);
    private native void onImeVisibilityNative(boolean visible);
}
```

**`NativeImeActivity.java`**

```java
package com.example.nativeime;

import android.app.NativeActivity;
import android.os.Bundle;
import android.text.InputType;
import android.view.Gravity;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
import android.widget.FrameLayout;

import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;

public final class NativeImeActivity extends NativeActivity {
    private InputEnabledTextView imeView;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        FrameLayout root = findViewById(android.R.id.content);

        imeView = new InputEnabledTextView(this);
        imeView.setAlpha(0f);
        imeView.createBridge(InputType.TYPE_CLASS_TEXT);

        FrameLayout.LayoutParams lp =
                new FrameLayout.LayoutParams(1, 1, Gravity.BOTTOM | Gravity.START);
        root.addView(imeView, lp);

        setInputConnectionNative(imeView.bridgeConnection);
    }

    public void showImeFromNative(int inputType) {
        runOnUiThread(() -> {
            EditorInfo info = new EditorInfo();
            info.inputType = inputType;
            info.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN;
            imeView.bridgeConnection.setEditorInfo(info);

            imeView.requestFocus();
            getSystemService(InputMethodManager.class).restartInput(imeView);

            WindowInsetsControllerCompat controller =
                    WindowCompat.getInsetsController(getWindow(), imeView);
            if (controller != null) {
                controller.show(WindowInsetsCompat.Type.ime());
            }
        });
    }

    public void hideImeFromNative() {
        runOnUiThread(() -> {
            WindowInsetsControllerCompat controller =
                    WindowCompat.getInsetsController(getWindow(), imeView);
            if (controller != null) {
                controller.hide(WindowInsetsCompat.Type.ime());
            }
        });
    }

    public void restartImeFromNative() {
        runOnUiThread(() ->
                getSystemService(InputMethodManager.class).restartInput(imeView));
    }

    private native void setInputConnectionNative(
            com.google.androidgamesdk.gametextinput.InputConnection connection);
}
```

This patch works because it replaces the stock “focused plain `View`” model with a **focused editor view that actually creates an `InputConnection`**, while still using the documented IME-show sequence of **request focus first, then show the IME**. The `IME_FLAG_NO_FULLSCREEN` part comes directly from Google’s sample because native full-screen apps often want to avoid the IME taking over the whole display in landscape. citeturn25view0turn25view1turn15view5

## Kotlin patch with no extra IME library

If you do not want the `games-text-input` dependency, the bare minimum is to implement your own custom editor view using `BaseInputConnection` and an `Editable`. Android’s docs support that model: `InputConnection` says an editor should start by implementing `View.onCreateInputConnection()`, `BaseInputConnection` says implementations should provide an `Editable`, and the custom-editor guide says the view should return `true` from `onCheckIsTextEditor()`. This route is more maintenance-heavy than standalone `GameTextInput`, but it can be enough if your Android side only needs to ferry text state into Rust. citeturn15view0turn15view2turn5view0turn33view0

```kotlin
package com.example.nativeime

import android.app.NativeActivity
import android.content.Context
import android.os.Build
import android.os.Bundle
import android.text.Editable
import android.text.InputType
import android.text.Selection
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.WindowInsets
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout

class NativeImeActivity : NativeActivity() {
    private lateinit var imeView: RustImeView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val root = findViewById<FrameLayout>(android.R.id.content)
        imeView = RustImeView(this).apply { alpha = 0f }

        root.addView(
            imeView,
            FrameLayout.LayoutParams(1, 1, Gravity.BOTTOM or Gravity.START)
        )
    }

    fun showImeFromNative(inputType: Int = InputType.TYPE_CLASS_TEXT) {
        runOnUiThread {
            imeView.inputTypeValue = inputType
            imeView.requestFocus()

            val imm = getSystemService(InputMethodManager::class.java)
            imm.restartInput(imeView)

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                imeView.windowInsetsController?.show(WindowInsets.Type.ime())
            } else {
                imm.showSoftInput(imeView, InputMethodManager.SHOW_IMPLICIT)
            }
        }
    }

    fun hideImeFromNative() {
        runOnUiThread {
            val imm = getSystemService(InputMethodManager::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                imeView.windowInsetsController?.hide(WindowInsets.Type.ime())
            } else {
                imm.hideSoftInputFromWindow(imeView.windowToken, 0)
            }
        }
    }

    fun replaceTextStateFromNative(text: String, selStart: Int, selEnd: Int) {
        runOnUiThread {
            imeView.replaceState(text, selStart, selEnd)
            getSystemService(InputMethodManager::class.java).restartInput(imeView)
        }
    }
}

class RustImeView(context: Context) : View(context) {
    var inputTypeValue: Int = InputType.TYPE_CLASS_TEXT
    private val editable: Editable = Editable.Factory.getInstance().newEditable("")

    init {
        isFocusable = true
        isFocusableInTouchMode = true
        Selection.setSelection(editable, 0)
    }

    override fun onCheckIsTextEditor(): Boolean = true

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
        outAttrs.inputType = inputTypeValue
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN or EditorInfo.IME_ACTION_NONE
        outAttrs.initialSelStart = Selection.getSelectionStart(editable)
        outAttrs.initialSelEnd = Selection.getSelectionEnd(editable)

        return object : BaseInputConnection(this@RustImeView, true) {
            override fun getEditable(): Editable = editable

            override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
                val ok = super.commitText(text, newCursorPosition)
                publishState()
                return ok
            }

            override fun setComposingText(text: CharSequence, newCursorPosition: Int): Boolean {
                val ok = super.setComposingText(text, newCursorPosition)
                publishState()
                return ok
            }

            override fun finishComposingText(): Boolean {
                val ok = super.finishComposingText()
                publishState()
                return ok
            }

            override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
                val ok = super.deleteSurroundingText(beforeLength, afterLength)
                publishState()
                return ok
            }

            override fun sendKeyEvent(event: KeyEvent): Boolean {
                if (event.action == KeyEvent.ACTION_DOWN) {
                    onImeKeyNative(event.keyCode, event.unicodeChar)
                }
                return super.sendKeyEvent(event)
            }

            override fun performEditorAction(actionCode: Int): Boolean {
                onEditorActionNative(actionCode)
                return true
            }
        }
    }

    fun replaceState(text: String, selStart: Int, selEnd: Int) {
        editable.replace(0, editable.length, text)
        Selection.setSelection(
            editable,
            selStart.coerceIn(0, editable.length),
            selEnd.coerceIn(0, editable.length)
        )
        publishState()
    }

    private fun publishState() {
        val selStart = Selection.getSelectionStart(editable)
        val selEnd = Selection.getSelectionEnd(editable)
        val compStart = BaseInputConnection.getComposingSpanStart(editable)
        val compEnd = BaseInputConnection.getComposingSpanEnd(editable)

        context.getSystemService(InputMethodManager::class.java).updateSelection(
            this,
            selStart,
            selEnd,
            compStart,
            compEnd
        )

        onTextStateNative(editable.toString(), selStart, selEnd, compStart, compEnd)
    }

    private external fun onTextStateNative(
        text: String,
        selStart: Int,
        selEnd: Int,
        compStart: Int,
        compEnd: Int
    )

    private external fun onImeKeyNative(keyCode: Int, unicodeChar: Int)
    private external fun onEditorActionNative(actionCode: Int)
}
```

The important parts here are not arbitrary. Android’s docs say editor authors should keep `EditorInfo.initialSelStart` and `initialSelEnd` current when `onCreateInputConnection()` is called, and they should call `InputMethodManager.updateSelection()` whenever the cursor moves. When your native side changes the text model substantially, calling `restartInput()` is the correct way to force the IME to rebuild its state. citeturn29view0turn26search1

## Integration checklist

Use this as the short list that decides whether the patch is actually complete.

- **Subclass `NativeActivity` in the manifest rather than using `android.app.NativeActivity` directly.** The NDK docs say subclassing is allowed, and your subclass must still keep the `android.app.lib_name` metadata so `NativeActivity` can load your native library. citeturn34view0turn31view0

- **Do not replace the problem view with another plain `View`.** The patch only works if the target view behaves as a text editor: it should be focusable, return `true` from `onCheckIsTextEditor()`, and provide a non-null `InputConnection`. citeturn5view0turn15view0turn15view2

- **Show the IME only after focusing the editor view.** Android’s own example does `requestFocus()` first and then shows `Type.ime()`, and the visibility docs recommend the WindowInsets-based path because `showSoftInput()` can be ignored during startup timing races. citeturn15view3turn15view5turn2view2

- **Check your `windowSoftInputMode`.** Avoid `stateHidden` and `alwaysHidden` if you expect the keyboard to appear programmatically during startup or soon after focus. citeturn15view6

- **Use `InputMethodManager.isAcceptingText()` as a quick sanity check.** Android’s reference says that if this is `false`, the served view has no input connection and can only handle raw key events. In that state, soft-keyboard input is still fundamentally broken for modern IMEs. citeturn29view0

- **Prefer standalone `GameTextInput` if you want the least fragile result.** Google documents it as a supported standalone path, provides sample Java code for an `InputEnabledTextView`, and the AndroidX release notes specifically call out fixes for use without `GameActivity` and broader keyboard stability. citeturn33view0turn25view0turn30view0

The bottom line is straightforward: **staying on `NativeActivity` is viable**, but only if you stop relying on `NativeActivity`’s stock focused view for text entry. Add a real editor bridge, focus it, and then show the IME. If you want the patch with the fewest edge cases, use **standalone `GameTextInput` inside a `NativeActivity` subclass**. If you want zero extra dependencies, the **`BaseInputConnection` Kotlin bridge** above is the smallest workable SDK-only patch. citeturn33view0turn30view0turn5view0turn15view0