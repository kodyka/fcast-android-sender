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
    private native void onTextStateNative(String text,
                                          int selStart, int selEnd,
                                          int compStart, int compEnd);
    private native void onImeKeyNative(int keyCode, int unicodeChar);
    private native void onEditorActionNative(int actionCode);
}
