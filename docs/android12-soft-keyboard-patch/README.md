# Android 12 soft-keyboard patch — split guide

**Status:** guide only — no code in this repository is modified by these documents.
**Scope:** Android 12 (API 31, `Build.VERSION_CODES.S`) and Android 12L (API 32, `Build.VERSION_CODES.S_V2`).
**Target file when implemented:** `app/src/main/java/org/fcast/android/sender/MainActivity.java` (currently extends `android.app.NativeActivity`, see `MainActivity.java:204`).
**Source:** distilled from `../research-android12-activity-keyboard.md`. See that file for upstream documentation references.
**Companion single-file version:** `../android12-soft-keyboard-patch-guide.md`.

The original guide was split into one file per step so each step can be reviewed and landed independently.

## Read in this order

| # | File | Purpose |
|---|---|---|
| 0 | [00-overview.md](00-overview.md) | Why the keyboard is broken on Android 12 here, and the overall strategy. |
| 1 | [01-manifest-check.md](01-manifest-check.md) | Confirm `AndroidManifest.xml` is compatible. No edits required. |
| 2 | [02-install-ime-bridge.md](02-install-ime-bridge.md) | Add the version-gated `installAndroid12ImeBridge()` call inside `MainActivity.onCreate()`. |
| 3 | [03-rust-ime-view.md](03-rust-ime-view.md) | New file: `RustImeView.java` — the actual editor with `BaseInputConnection`. |
| 4 | [04-show-hide-from-native.md](04-show-hide-from-native.md) | `showImeFromNative(int)` / `hideImeFromNative()` on `MainActivity`. |
| 5 | [05-native-jni-wiring.md](05-native-jni-wiring.md) | Route the native “open keyboard” request through the new Java methods on API 31 / 32 only. |
| 6 | [06-runtime-sanity-checks.md](06-runtime-sanity-checks.md) | Two checks on a real Android 12 device to verify the patch works. |
| 7 | [07-route-a-game-text-input.md](07-route-a-game-text-input.md) | Escape hatch: standalone `GameTextInput` (only if Route B is insufficient). |
| 8 | [08-non-goals-and-risks.md](08-non-goals-and-risks.md) | What this guide deliberately does **not** change; known risks and follow-ups. |
| 9 | [09-checklist.md](09-checklist.md) | Final landing checklist. |

## Route choice in one sentence

Land **Route B** first (no new dependencies, `BaseInputConnection` editor view); only escalate to **Route A** (`androidx.games:games-text-input:4.0.0`) if you later need composition regions, completions, or full-screen IME control.
