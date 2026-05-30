# Step 6 — Android arm64 build validation

**Phase:** 1 — Android MVP
**Priority:** highest
**Depends on:** Steps 1–5
**Unblocks:** end of Phase 1

## Goal

Confirm Steps 1–5 do not break the existing arm64 packaging path. No
build-system refactor, no new ABIs. Build the cdylib with `cargo-ndk`,
package the APK with Gradle, install on a device, and run a smoke test.

## Prerequisites

```bash
rustup target add aarch64-linux-android

# cargo-ndk auto-detects the NDK in the Android Studio default location;
# override only if needed.
cargo install cargo-ndk

export ANDROID_NDK_HOME="$HOME/Library/Android/sdk/ndk/<version>"
export GSTREAMER_ROOT_ANDROID="$HOME/gstreamer-1.0-android-universal-1.28.0"
```

Verify the existing root `build.rs` still recognises the host. If it complains
about Apple Silicon, that is **out of scope for this step** — file a
follow-up; do not regress build.rs as part of MVP.

## Build matrix

```bash
# 1. Confirm the runtime crate compiles for arm64 with the Phase-1 features.
cargo ndk -t arm64-v8a build \
    -p gstpop-runtime \
    --features "typed-client media-tools" \
    --release

# 2. Build the app cdylib that ships in the APK.
cargo ndk -t arm64-v8a -o app/src/main/jniLibs build \
    -p android-sender \
    --release

# 3. Package.
./gradlew :app:assembleDebug
```

If any of the three steps fails, do not proceed to device testing — fix the
break first and re-run Step 5's integration tests on the host.

## Sanity check the APK contains the right `.so`

```bash
unzip -l app/build/outputs/apk/debug/app-debug.apk | grep '\.so$'
# expect:  lib/arm64-v8a/libandroid_sender.so  (or whatever the cdylib name is)
# expect NO lib/x86_64/...  (until Step 11)
```

## Device smoke test

```bash
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb logcat -c
adb shell am start -n org.fcast.sender/.MainActivity   # adjust to actual launcher activity
adb logcat | grep -iE 'gstpop|embedded|rust'
```

Look for:

- `Embedded gst-pop running on 127.0.0.1:<port>` (info-level)
- No `Embedded gst-pop bind failed` lines
- After tapping play in the UI: typed-client RPC traces (if your client logs
  at `tracing::debug` or above)

## Manual UI checklist

On the device:

1. App launches without crash.
2. Within ~1s, embedded server is reported `Running` (via whatever in-app
   debug surface you have, or logcat).
3. Trigger `create_pipeline` from the UI (existing playback flow).
4. Trigger `play`, then `pause`, then `stop`.
5. Background and foreground the app; `start_embedded` second call should be
   a no-op (Running, externally_owned=false, same port).
6. Force-stop the app; relaunch; the port is reusable.

## What to defer (explicitly)

| Item | Defer to |
|---|---|
| `x86_64-linux-android` emulator | [Step 11](./step-11-multi-abi-android.md) |
| `armv7-linux-androideabi` | [Step 11](./step-11-multi-abi-android.md) |
| NDK r28c linker fixes (already merged on `refactor`) | already done |
| JNI bridge in `gstpop-runtime` | [Step 7](./step-07-jni-bridge.md) |
| Apple Silicon host build.rs fixes | Out of Phase 1 |

## Done when

- arm64 release build of `gstpop-runtime` succeeds with `typed-client` and
  `media-tools` features.
- `:app:assembleDebug` produces an APK containing only `lib/arm64-v8a/*.so`.
- Device smoke test: app launches, embedded server reaches `Running`, a
  single `create_pipeline` → `play` → `stop` cycle completes without ANR or
  native crash.
- Integration tests from Step 5 still pass on the host after any incidental
  fixes from this step.
