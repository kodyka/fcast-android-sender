# Step 0 — Overview: why the keyboard is broken on Android 12 here

[← Index](README.md) · [Next: Step 1 — Manifest check →](01-manifest-check.md)

## The setup in this repo

`MainActivity extends NativeActivity` — see `app/src/main/java/org/fcast/android/sender/MainActivity.java:204`. The native library is loaded in a static initializer:

```java
static {
    System.loadLibrary("gstreamer_android");
    System.loadLibrary("fcastsender");
}
```

The manifest binds that library to the activity via `android.app.lib_name`:

```xml
<!-- app/src/main/AndroidManifest.xml:56-58 -->
<meta-data
    android:name="android.app.lib_name"
    android:value="fcastsender" />
```

So `ANativeActivity_onCreate` runs inside `fcastsender`, and any “open keyboard” request from native code today goes through `ANativeActivity_showSoftInput()`.

## Why that fails on Android 12

Stock `NativeActivity` installs an internal `NativeContentView` — a plain `View`, **not a text editor** — as its content view and focuses it. That has two consequences on Android 12:

1. `ANativeActivity_showSoftInput()` calls `InputMethodManager.showSoftInput()` against that plain `View`. Even when the IME appears, the focused target has **no `InputConnection`**, so `InputMethodManager.isAcceptingText()` returns `false`. Modern IMEs route text only through `InputConnection`, not raw key events — default soft keyboards do not generate `KeyEvent`s on Jelly Bean+.
2. `showSoftInput()` is timing-sensitive during activity start. On Android 11+ the reliable API is `WindowInsetsController.show(Type.ime())`, which defers until the window has IME control. Android 12’s window-focus race makes this especially visible.

A complete fix needs **both**:

- A real editor target with `onCheckIsTextEditor() = true` and a non-null `InputConnection`.
- `requestFocus()` on that editor, then `WindowInsetsController.show(ime())`.

Replacing only the show-IME API is not enough.

## Strategy

Keep `MainActivity extends NativeActivity`. Keep the native entry point. Add a tiny invisible editor `View` to `android.R.id.content` **only on Android 12 / 12L**, and route IME show/hide through that view.

Version-gating reasons:

- Other Android versions keep their current code path — zero regression risk where things already work.
- The bug pattern in the research is most acute on Android 12’s window-focus timing; restricting the change to `S` / `S_V2` keeps blast radius minimal.

Two implementation routes:

| Route | Dependency | Pros | Cons |
|---|---|---|---|
| **B (recommended first)** | none | Zero new dependencies | You own composition + selection + `restartInput` plumbing |
| **A (escape hatch)** | `androidx.games:games-text-input:4.0.0` + Prefab/CMake link | Composition, selection, IME insets, action keys handled for you | Adds AndroidX dependency; needs native CMake change |

Steps 1–6 implement Route B. Step 7 documents Route A.

[← Index](README.md) · [Next: Step 1 — Manifest check →](01-manifest-check.md)
