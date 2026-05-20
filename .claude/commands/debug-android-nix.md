# Debug Android App with nix develop + ADB

Reference guide for building, deploying, and debugging the fcast-android-sender
app on a connected Android device using the Nix dev shell.

---

## 1. Enter the nix dev shell

All Android tooling (cargo-ndk, adb, ANDROID_HOME, ANDROID_NDK_ROOT,
GSTREAMER_ROOT_ANDROID) is provided by the `android` devShell:

```bash
nix develop .#android
```

To run a one-shot command without entering an interactive shell:

```bash
nix develop .#android -c <command>
nix develop .#android -c bash -c '<cmd1>; <cmd2>'   # multi-command
```

---

## 2. Build + deploy (full)

```bash
nix develop .#android -c bash scripts/build-deploy.sh
```

This compiles the Rust library (all ABI targets), assembles the APK, and
installs it via adb.

---

## 3. Install only (skip Rust rebuild)

```bash
nix develop .#android -c bash -c \
  'adb install -r app/build/outputs/apk/debug/app-debug.apk'
```

If `adb install -r` silently succeeds but the package doesn't appear in
`adb shell pm list packages | grep fcast`, do a fresh install:

```bash
nix develop .#android -c bash -c \
  'adb uninstall org.fcast.android.sender; \
   adb install app/build/outputs/apk/debug/app-debug.apk'
```

---

## 4. Launch / restart the app

```bash
nix develop .#android -c bash -c \
  'adb shell am start -n "org.fcast.android.sender/.MainActivity"'
```

Force-stop before relaunch (clears in-memory state):

```bash
nix develop .#android -c bash -c \
  'adb shell am force-stop org.fcast.android.sender && \
   adb shell am start -n "org.fcast.android.sender/.MainActivity"'
```

---

## 5. View logs

Stream Rust + app logs (Ctrl-C to stop):

```bash
nix develop .#android -c bash -c \
  'adb logcat -s fcastsender RustStdoutStderr'
```

With timestamps:

```bash
nix develop .#android -c bash -c \
  'adb logcat -v time -s fcastsender RustStdoutStderr'
```

Clear logcat buffer before a fresh run:

```bash
nix develop .#android -c bash -c 'adb logcat -c'
```

---

## 6. Take screenshots

```bash
nix develop .#android -c bash -c \
  'adb shell screencap -p /sdcard/screen.png && \
   adb pull /sdcard/screen.png /tmp/screen.png'
```

---

## 7. Simulate taps and navigation

`adb shell input tap` uses **physical pixels**. Device: 1440×2960 px.

### Coordinate system

| Region               | Physical y                       |
|----------------------|----------------------------------|
| Status bar top       | 0                                |
| Safe-area top        | 171 px                           |
| App header bottom    | 171 + 196 = 367 px               |
| First content row    | 367 + 42 (padding) ≈ 409 px      |

Scale: 1 Slint logical px ≈ 3.5 physical px (560 dpi device).  
Row height: 48 logical × 3.5 = **168 physical px**.  
Section spacing: 8 logical × 3.5 = **28 physical px**.

### Control bar (bottom, y ≈ 2606 px physical)

| Button          | x (physical) |
|-----------------|--------------|
| Settings        | 168          |
| Quick actions   | 504          |
| Cast / record   | 720          |
| Mixer           | 936          |
| Camera          | 1272         |

Tap example:

```bash
nix develop .#android -c bash -c 'adb shell input tap 168 2606'
```

### Settings page rows (no scroll needed, y from top)

| Row                  | Physical y |
|----------------------|------------|
| Audio                | 1578       |
| Camera               | 1750       |
| Bitrate presets      | 1922       |
| Network              | 2094       |
| Recording            | 2266       |
| Quick actions        | 2527       |
| H.264 encoder test   | 2786       |

Rows below H.264 require a scroll first:

```bash
# Scroll down ~600 px
nix develop .#android -c bash -c \
  'adb shell input swipe 720 1800 720 1200 400'
# Then tap target row
nix develop .#android -c bash -c 'adb shell input tap 720 <y>'
```

---

## 8. Check package installation

```bash
nix develop .#android -c bash -c \
  'adb shell pm list packages | grep fcast'
```

---

## 9. Common failure modes

| Symptom | Fix |
|---|---|
| `adb: device not found` | Plug in device; enable USB debugging; `adb devices` to confirm |
| `adb install` succeeds but package missing | `adb uninstall …` then fresh `adb install` (no `-r`) |
| `am start` fails — activity not found | Uninstall + fresh install resolves stale manifest cache |
| Slint UI gaps between settings rows | Ensure `alignment: start` on `VerticalLayout` inside every `ScrollView` |
| Build fails outside nix shell | Run all `cargo`/`gradle` commands via `nix develop .#android -c …` |
