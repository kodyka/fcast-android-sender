# Step 2 — Install the IME bridge view (Android 12 / 12L only)

[← Previous: Step 1](01-manifest-check.md) · [Index](README.md) · [Next: Step 3 →](03-rust-ime-view.md)

**Goal:** add an invisible 1×1 editor view to `android.R.id.content` inside `MainActivity.onCreate()`, gated on Android 12 / 12L. This is the “focus target” that fixes the missing `InputConnection`.

## Where to add the call

Inside `MainActivity.onCreate()` (currently at `MainActivity.java:326`). The bridge must be installed **after** `super.onCreate()` returns, because `NativeActivity.onCreate()` installs its own content view and loads the native library; customization is safe only after that.

Recommended insertion point: at the very end of `onCreate()`, after the existing `POST_NOTIFICATIONS` permission block.

## Imports to add to `MainActivity.java`

```java
import android.view.Gravity;
import android.widget.FrameLayout;
```

`android.os.Build` and `android.view.View` are already in scope via `android.os.*` and `android.view.*` (see `MainActivity.java:27,30`).

## New field on `MainActivity`

Add alongside the other private fields (e.g. just below `displayManager`):

```java
// Android 12 / 12L only; null on every other Android version.
private RustImeView imeView;
```

`RustImeView` is the new editor class created in [Step 3](03-rust-ime-view.md). It lives in the same package (`org.fcast.android.sender`), so no import is needed.

## Patch fragment for `onCreate()`

Append this block to the end of the existing `onCreate()` body:

```java
@Override
protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);

    // ... existing GStreamer.init, Discoverer, projectionCallback,
    // mediaProjectionManager, glThread, LocalBroadcastManager,
    // displayManager, POST_NOTIFICATIONS permission block ...

    if (Build.VERSION.SDK_INT == Build.VERSION_CODES.S
            || Build.VERSION.SDK_INT == Build.VERSION_CODES.S_V2) {
        installAndroid12ImeBridge();
    }
}
```

## The helper method (full snippet)

Add this as a new method on `MainActivity`:

```java
/**
 * Android 12 / 12L only: install an invisible 1x1 editor View into
 * android.R.id.content so the framework has a valid InputConnection
 * target. Without this, the IME may appear but isAcceptingText() is
 * false and modern soft keyboards deliver no characters.
 */
private void installAndroid12ImeBridge() {
    FrameLayout root = findViewById(android.R.id.content);
    if (root == null) {
        Log.w(TAG, "installAndroid12ImeBridge: no android.R.id.content root");
        return;
    }

    imeView = new RustImeView(this);
    imeView.setAlpha(0f);                 // invisible
    imeView.setFocusable(true);
    imeView.setFocusableInTouchMode(true);

    // 1x1 at bottom-left so the view never overlaps the native render
    // surface that NativeActivity installed as the first child.
    FrameLayout.LayoutParams lp = new FrameLayout.LayoutParams(
            1, 1, Gravity.BOTTOM | Gravity.START);
    root.addView(imeView, lp);

    Log.i(TAG, "Android 12 IME bridge installed (SDK="
            + Build.VERSION.SDK_INT + ")");
}
```

## Why each invariant matters

- **Size `1×1`, `alpha = 0f`** — the view is non-visible and effectively non-interactive for the user, but it still participates in focus and the IME pipeline.
- **Child of `android.R.id.content`** — that is the `FrameLayout` `NativeActivity` already populated with its native render surface as the first child; adding a 1×1 sibling does not displace anything.
- **`focusable=true` *and* `focusableInTouchMode=true`** — otherwise `requestFocus()` in [Step 4](04-show-hide-from-native.md) is a silent no-op and the IME shows against the plain `NativeContentView` (back to the broken state).
- **Version gate `== S || == S_V2`** — equivalent to `SDK_INT in {31, 32}`. Other versions keep the legacy `ANativeActivity_showSoftInput()` path, so no untested behavior leaks into Android 11- or 13+.

## Why end-of-`onCreate()` and not earlier

`MainActivity.onCreate()` already runs `GStreamer.init(this)`, instantiates `Discoverer`, registers a `LocalBroadcastManager` receiver, and registers a `DisplayManager` listener. None of these touch `android.R.id.content`, so insertion order does not matter for correctness — but appending at the end keeps the existing flow untouched and the new code reviewable as a single trailing block.

## Verification

After this step alone (without Steps 3–5):

- On an Android 12 device the app should still launch identically. The new view is invisible and never focused yet, so it cannot change behavior on its own.
- `adb shell dumpsys window | grep -i ime` should still show the activity normally; no IME visibility changes happen yet.

If the app crashes on launch, the most likely cause is that `RustImeView` is referenced before [Step 3](03-rust-ime-view.md) created the class. Land Steps 2 and 3 together.

[← Previous: Step 1](01-manifest-check.md) · [Index](README.md) · [Next: Step 3 →](03-rust-ime-view.md)
