# Step 1 — Confirm `AndroidManifest.xml` is compatible

[← Previous: Step 0](00-overview.md) · [Index](README.md) · [Next: Step 2 →](02-install-ime-bridge.md)

**Goal:** verify the existing manifest does not block the patch. **No edits are required for Route B.**

## What the current manifest says

```xml
<!-- app/src/main/AndroidManifest.xml:45-50 -->
<activity
    android:name=".MainActivity"
    android:configChanges="keyboardHidden|orientation|screenSize"
    android:windowSoftInputMode="adjustResize"
    android:resizeableActivity="true"
    android:exported="true">
    <intent-filter>
        <action android:name="android.intent.action.MAIN" />
        <category android:name="android.intent.category.LAUNCHER" />
    </intent-filter>

    <meta-data
        android:name="android.app.lib_name"
        android:value="fcastsender" />
</activity>
```

## Why this is already correct

- **`windowSoftInputMode="adjustResize"`** — acceptable. The research warns specifically against `stateHidden` and `stateAlwaysHidden`, which override programmatic `show(ime())` during startup on Android 12. Those are not set here.
- **`android.app.lib_name = fcastsender`** — preserved. `NativeActivity` will still load `libfcastsender.so` and call `ANativeActivity_onCreate`.
- **`MainActivity extends NativeActivity`** (`MainActivity.java:204`) — already a subclass. The research notes that subclassing `NativeActivity` (rather than naming `android.app.NativeActivity` directly in the manifest) is the supported customization point. ✅
- **`configChanges="keyboardHidden|orientation|screenSize"`** — the activity is not recreated on those changes, so the editor view installed in Step 2 survives rotation. No state restoration is needed.

## What would require a change (and does not apply here)

If, for any reason, `windowSoftInputMode` were ever changed to include `stateHidden` or `stateAlwaysHidden`, the IME would refuse to appear during startup even with `WindowInsetsController.show(ime())`. Keep it at `adjustResize` or `stateUnspecified|adjustResize`.

If you adopt **Route A** ([Step 7](07-route-a-game-text-input.md)) and want the most explicit form, the equivalent manifest fragment is:

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

`stateUnspecified` is the default and matches what stock `NativeActivity` itself sets, so this is a no-op in practice.

## Verification

Open `app/src/main/AndroidManifest.xml` and confirm:

- [ ] `android:windowSoftInputMode` does **not** contain `stateHidden` or `stateAlwaysHidden`.
- [ ] The `android.app.lib_name` meta-data is still `fcastsender`.
- [ ] `MainActivity` is referenced as `.MainActivity` (the project subclass), not `android.app.NativeActivity`.

Result expected: all three pass. Step 1 introduces no file changes.

[← Previous: Step 0](00-overview.md) · [Index](README.md) · [Next: Step 2 →](02-install-ime-bridge.md)
