# Step 5 — Native JNI wiring

[← Previous: Step 4](04-show-hide-from-native.md) · [Index](README.md) · [Next: Step 6 →](06-runtime-sanity-checks.md)

**Goal:** route the “open soft keyboard” request from native code through `MainActivity#showImeFromNative(int)` on Android 12 / 12L, and keep `ANativeActivity_showSoftInput()` for every other Android version.

This step has two halves:

- **Outbound** (native → Java): replace the show/hide call sites.
- **Inbound** (Java → native): implement the three `native` methods declared on `RustImeView` so character/key/action events reach Rust.

The repo uses Rust as the primary native language (linked together with C glue into `libfcastsender.so`), so the C examples below illustrate the JNI shape — the Rust equivalents use `jni-rs` or `ndk` crates with the same method signatures.

## Outbound: replace `ANativeActivity_showSoftInput()` call sites

Conceptually, every existing call of the form:

```c
ANativeActivity_showSoftInput(activity, ANATIVEACTIVITY_SHOW_SOFT_INPUT_IMPLICIT);
```

becomes:

```c
#include <android/api-level.h>

static void fcast_show_soft_input(ANativeActivity* activity, int input_type) {
    int sdk = android_get_device_api_level();
    if (sdk == 31 /* Android 12, S */ || sdk == 32 /* Android 12L, S_V2 */) {
        JNIEnv* env = NULL;
        (*activity->vm)->AttachCurrentThread(activity->vm, &env, NULL);

        jclass cls = (*env)->GetObjectClass(env, activity->clazz);
        jmethodID mid = (*env)->GetMethodID(env, cls, "showImeFromNative", "(I)V");
        if (mid != NULL) {
            (*env)->CallVoidMethod(env, activity->clazz, mid, (jint) input_type);
        }
        if ((*env)->ExceptionCheck(env)) {
            (*env)->ExceptionClear(env);
        }
        (*env)->DeleteLocalRef(env, cls);
        // Do NOT DetachCurrentThread here if the thread is owned by the app loop.
    } else {
        ANativeActivity_showSoftInput(activity, ANATIVEACTIVITY_SHOW_SOFT_INPUT_IMPLICIT);
    }
}

static void fcast_hide_soft_input(ANativeActivity* activity) {
    int sdk = android_get_device_api_level();
    if (sdk == 31 || sdk == 32) {
        JNIEnv* env = NULL;
        (*activity->vm)->AttachCurrentThread(activity->vm, &env, NULL);
        jclass cls = (*env)->GetObjectClass(env, activity->clazz);
        jmethodID mid = (*env)->GetMethodID(env, cls, "hideImeFromNative", "()V");
        if (mid != NULL) {
            (*env)->CallVoidMethod(env, activity->clazz, mid);
        }
        if ((*env)->ExceptionCheck(env)) (*env)->ExceptionClear(env);
        (*env)->DeleteLocalRef(env, cls);
    } else {
        ANativeActivity_hideSoftInput(activity, ANATIVEACTIVITY_HIDE_SOFT_INPUT_IMPLICIT_ONLY);
    }
}
```

### Rust (`jni-rs`) equivalent

```rust
use jni::{JNIEnv, objects::JObject};

fn fcast_show_soft_input(env: &mut JNIEnv, activity: &JObject, input_type: i32) {
    let sdk = ndk_sys::android_get_device_api_level();
    if sdk == 31 || sdk == 32 {
        // Signature: void showImeFromNative(int)
        let _ = env.call_method(
            activity,
            "showImeFromNative",
            "(I)V",
            &[input_type.into()],
        );
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
    } else {
        unsafe {
            ndk_sys::ANativeActivity_showSoftInput(
                /* activity ptr */ activity_ptr,
                ndk_sys::ANATIVEACTIVITY_SHOW_SOFT_INPUT_IMPLICIT as u32,
            );
        }
    }
}
```

`input_type = 1` (i.e. `InputType.TYPE_CLASS_TEXT`) is the default for free-form text entry. See [Step 4](04-show-hide-from-native.md) for the constant table.

## Inbound: implement `RustImeView`’s native methods

`RustImeView` declares three `native` methods:

```java
private native void onTextStateNative(String text,
                                      int selStart, int selEnd,
                                      int compStart, int compEnd);
private native void onImeKeyNative(int keyCode, int unicodeChar);
private native void onEditorActionNative(int actionCode);
```

JNI symbol names follow the fully-qualified class:

```
Java_org_fcast_android_sender_RustImeView_onTextStateNative
Java_org_fcast_android_sender_RustImeView_onImeKeyNative
Java_org_fcast_android_sender_RustImeView_onEditorActionNative
```

### Option A: implicit symbol export (matches `MainActivity` style)

The existing code uses implicit JNI symbol export (e.g. `nativeBackPressed` at `MainActivity.java:1154` has no explicit `RegisterNatives` call), so the same convention works here. C signatures:

```c
JNIEXPORT void JNICALL
Java_org_fcast_android_sender_RustImeView_onTextStateNative(
        JNIEnv* env, jobject thiz,
        jstring text, jint selStart, jint selEnd, jint compStart, jint compEnd) {
    const char* utf = (*env)->GetStringUTFChars(env, text, NULL);
    // forward to Rust: rust_ime_on_text_state(utf, selStart, selEnd, compStart, compEnd);
    (*env)->ReleaseStringUTFChars(env, text, utf);
}

JNIEXPORT void JNICALL
Java_org_fcast_android_sender_RustImeView_onImeKeyNative(
        JNIEnv* env, jobject thiz, jint keyCode, jint unicodeChar) {
    // forward to Rust: rust_ime_on_key(keyCode, unicodeChar);
}

JNIEXPORT void JNICALL
Java_org_fcast_android_sender_RustImeView_onEditorActionNative(
        JNIEnv* env, jobject thiz, jint actionCode) {
    // forward to Rust: rust_ime_on_editor_action(actionCode);
}
```

### Option B: explicit `RegisterNatives`

If you prefer registering the methods explicitly (faster lookup, name-mangling-proof), call this once from `JNI_OnLoad` or from `MainActivity` static init:

```c
static JNINativeMethod kRustImeViewMethods[] = {
    {"onTextStateNative",     "(Ljava/lang/String;IIII)V", (void*) &on_text_state},
    {"onImeKeyNative",        "(II)V",                       (void*) &on_ime_key},
    {"onEditorActionNative",  "(I)V",                        (void*) &on_editor_action},
};

void register_rust_ime_view(JNIEnv* env) {
    jclass cls = (*env)->FindClass(env, "org/fcast/android/sender/RustImeView");
    (*env)->RegisterNatives(env, cls,
        kRustImeViewMethods,
        sizeof(kRustImeViewMethods) / sizeof(kRustImeViewMethods[0]));
    (*env)->DeleteLocalRef(env, cls);
}
```

### Rust equivalent (`jni-rs`, implicit export)

```rust
use jni::{JNIEnv, objects::{JClass, JString}, sys::jint};

#[no_mangle]
pub extern "system" fn Java_org_fcast_android_sender_RustImeView_onTextStateNative<'a>(
    mut env: JNIEnv<'a>,
    _this: JClass<'a>,
    text: JString<'a>,
    sel_start: jint,
    sel_end:   jint,
    comp_start: jint,
    comp_end:   jint,
) {
    let text: String = env.get_string(&text).map(Into::into).unwrap_or_default();
    crate::ime::on_text_state(&text, sel_start, sel_end, comp_start, comp_end);
}

#[no_mangle]
pub extern "system" fn Java_org_fcast_android_sender_RustImeView_onImeKeyNative<'a>(
    _env: JNIEnv<'a>, _this: JClass<'a>, key_code: jint, unicode_char: jint,
) {
    crate::ime::on_key(key_code, unicode_char);
}

#[no_mangle]
pub extern "system" fn Java_org_fcast_android_sender_RustImeView_onEditorActionNative<'a>(
    _env: JNIEnv<'a>, _this: JClass<'a>, action_code: jint,
) {
    crate::ime::on_editor_action(action_code);
}
```

## What about non-Android-12 versions?

They must continue to use `ANativeActivity_showSoftInput()` / `ANativeActivity_hideSoftInput()`. The version gate (`sdk == 31 || sdk == 32`) in `fcast_show_soft_input` is the only place that branches; the rest of the native code path is unchanged.

If field reports later show Android 13+ has the same problem, broaden the gate to `sdk >= 31` — but only after that signal arrives. Do not broaden speculatively.

## Verification

- On an Android 12 device, the outbound call should be observable in logcat: the `Log.d(TAG, "showImeFromNative: requestFocus=true")` from [Step 4](04-show-hide-from-native.md) confirms the JNI call landed.
- On Android 11 or 13+, `ANativeActivity_showSoftInput()` still runs and behavior is unchanged from main.
- `adb logcat -s RustImeView` after typing should show text events (with the [Step 3](03-rust-ime-view.md) stubs) or you should observe the Rust handler being hit (once the native side is wired).

[← Previous: Step 4](04-show-hide-from-native.md) · [Index](README.md) · [Next: Step 6 →](06-runtime-sanity-checks.md)
