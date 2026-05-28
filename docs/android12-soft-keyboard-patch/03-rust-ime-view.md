# Step 3 — `RustImeView.java`: the editor with `BaseInputConnection`

[← Previous: Step 2](02-install-ime-bridge.md) · [Index](README.md) · [Next: Step 4 →](04-show-hide-from-native.md)

**Goal:** create a new Java file `RustImeView.java` that is a focusable, invisible `View` declaring itself a text editor and exposing a real `InputConnection`. This is the single most important file in the patch — without it, the IME shows but never delivers characters.

**File path when implemented:** `app/src/main/java/org/fcast/android/sender/RustImeView.java`.

## Full snippet — `RustImeView.java`

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

/**
 * Invisible text-editor View used only on Android 12 / 12L to give the
 * NativeActivity a valid IME target. The framework requires:
 *   1. onCheckIsTextEditor() = true
 *   2. onCreateInputConnection() returning a non-null InputConnection
 *      backed by a real Editable
 *   3. updateSelection() calls whenever the model changes
 *
 * Without all three, InputMethodManager.isAcceptingText() stays false
 * and modern soft keyboards deliver no characters.
 */
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

    /** Called from MainActivity#showImeFromNative before requestFocus(). */
    public void setInputTypeValue(int v) {
        this.inputTypeValue = v;
    }

    @Override
    public boolean onCheckIsTextEditor() {
        return true;
    }

    @Override
    public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
        outAttrs.inputType = inputTypeValue;
        outAttrs.imeOptions =
                EditorInfo.IME_FLAG_NO_FULLSCREEN | EditorInfo.IME_ACTION_NONE;
        outAttrs.initialSelStart = Selection.getSelectionStart(editable);
        outAttrs.initialSelEnd   = Selection.getSelectionEnd(editable);

        return new BaseInputConnection(this, /*fullEditor=*/ true) {
            @Override
            public Editable getEditable() {
                return editable;
            }

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
            public boolean deleteSurroundingText(int beforeLength, int afterLength) {
                boolean ok = super.deleteSurroundingText(beforeLength, afterLength);
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

    /**
     * Called from MainActivity (typically from native code via JNI) when
     * the Rust side wants to overwrite the IME's idea of the text model.
     * After calling this, MainActivity should also call
     * InputMethodManager.restartInput(this) so the IME picks up the new
     * initial selection.
     */
    public void replaceState(String text, int selStart, int selEnd) {
        editable.replace(0, editable.length(), text);
        int len = editable.length();
        Selection.setSelection(
                editable,
                clamp(selStart, 0, len),
                clamp(selEnd,   0, len));
        publishState();
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(v, hi));
    }

    private void publishState() {
        int selStart  = Selection.getSelectionStart(editable);
        int selEnd    = Selection.getSelectionEnd(editable);
        int compStart = BaseInputConnection.getComposingSpanStart(editable);
        int compEnd   = BaseInputConnection.getComposingSpanEnd(editable);

        InputMethodManager imm =
                getContext().getSystemService(InputMethodManager.class);
        if (imm != null) {
            imm.updateSelection(this, selStart, selEnd, compStart, compEnd);
        }
        onTextStateNative(editable.toString(), selStart, selEnd, compStart, compEnd);
    }

    // -------- Native callbacks --------
    // Implement these on the Rust/C++ side. Until you do, declare empty
    // Java stubs (remove the `native` keyword and leave an empty body) so
    // UnsatisfiedLinkError does not block landing Steps 2–4.
    private native void onTextStateNative(String text,
                                          int selStart, int selEnd,
                                          int compStart, int compEnd);
    private native void onImeKeyNative(int keyCode, int unicodeChar);
    private native void onEditorActionNative(int actionCode);
}
```

## Stub-friendly variant (drop in until Rust side is ready)

If you want to land the visible-keyboard fix before wiring the native callbacks, replace the bottom three `native` declarations with:

```java
private void onTextStateNative(String text,
                               int selStart, int selEnd,
                               int compStart, int compEnd) {
    android.util.Log.d("RustImeView",
            "text=" + text + " sel=[" + selStart + "," + selEnd + "]"
            + " comp=[" + compStart + "," + compEnd + "]");
}

private void onImeKeyNative(int keyCode, int unicodeChar) {
    android.util.Log.d("RustImeView",
            "key=" + keyCode + " ch=" + unicodeChar);
}

private void onEditorActionNative(int actionCode) {
    android.util.Log.d("RustImeView", "editorAction=" + actionCode);
}
```

With these stubs the keyboard is visible and round-trips characters through `logcat`, but nothing reaches Rust yet. [Step 5](05-native-jni-wiring.md) covers the JNI wiring.

## Why each part is non-optional

These come directly from Android’s editor contract (see the research file in `../research-android12-activity-keyboard.md` for upstream doc references):

- **`onCheckIsTextEditor()` returning `true`** — declares this view as a text editor target. `InputMethodManager` uses this to decide whether to bind.
- **`BaseInputConnection` with a real `Editable`** — `isAcceptingText()` becomes `true`. Modern IMEs deliver text through `commitText` / `setComposingText` rather than raw `KeyEvent`s, so this is the path that actually carries characters.
- **`EditorInfo.initialSelStart` / `initialSelEnd`** — without these the IME does not know where the cursor is on first attach; some IMEs refuse to start composing.
- **`InputMethodManager.updateSelection()` on every change** — keeps the IME’s mirror of the editor coherent without expensive `restartInput()` round trips.
- **`IME_FLAG_NO_FULLSCREEN`** — prevents the IME from taking over the whole screen in landscape, which would hide the GStreamer render surface. The app is fundamentally a native render app, so this is what you want.
- **`finishComposingText()` overridden** — without forwarding state on commit-finalization, the Rust side will see stale composing-region indices.

## Verification

After Step 3 + Step 2 land together:

- `adb logcat | grep -E 'MainActivity|RustImeView'` on launch should show `Android 12 IME bridge installed` and no errors.
- Class loader should not throw — if it does, the most likely culprit is a typo in the package declaration; it **must** be `org.fcast.android.sender`.

[← Previous: Step 2](02-install-ime-bridge.md) · [Index](README.md) · [Next: Step 4 →](04-show-hide-from-native.md)
