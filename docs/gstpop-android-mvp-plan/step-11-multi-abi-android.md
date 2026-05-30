# Step 11 — Multi-ABI Android packaging

**Phase:** 3 — Desktop & cross-platform
**Priority:** medium (only after Phase 1 is shipping)
**Depends on:** Step 6 (working arm64)
**Unblocks:** emulator development, 32-bit devices

## Goal

Extend the APK from arm64-only to additional ABIs without regressing the
existing arm64 build. **Add one ABI at a time**; the order matters because
each new ABI requires matching GStreamer Android libraries and Rust target.

## Recommended order

1. `x86_64-linux-android` — emulator development on Intel/AMD hosts
2. `arm64-v8a` — *already done in Step 6*, but verify the Apple Silicon
   emulator path
3. `armv7-linux-androideabi` — only if 32-bit ARM devices are required
4. `i686-linux-android` — almost never needed today; skip unless forced

## Prerequisites

```bash
rustup target add \
    x86_64-linux-android \
    armv7-linux-androideabi \
    i686-linux-android

# GStreamer must be available for each ABI under $GSTREAMER_ROOT_ANDROID.
# The official "android-universal" tarball includes all four; verify:
ls $GSTREAMER_ROOT_ANDROID
# expect: arm64  armv7  x86  x86_64  share
```

If your `GSTREAMER_ROOT_ANDROID` is single-ABI, swap to the universal build
*before* touching Gradle.

## Gradle changes

Edit `app/build.gradle`:

```groovy
android {
    defaultConfig {
        ndk {
            // Start narrow; add ABIs one at a time.
            abiFilters "arm64-v8a", "x86_64"
            // Later, if needed:
            //   abiFilters "arm64-v8a", "x86_64", "armeabi-v7a"
        }
    }

    // Split APKs per ABI to keep download size down on the Play Store.
    splits {
        abi {
            enable true
            reset()
            include "arm64-v8a", "x86_64"
            universalApk false
        }
    }
}
```

## Build commands

```bash
# Build each ABI's cdylib; cargo-ndk handles target setup.
cargo ndk \
    -t arm64-v8a \
    -t x86_64 \
    -o app/src/main/jniLibs \
    build -p android-sender --release

./gradlew :app:assembleRelease
```

Inspect the resulting APKs:

```bash
for apk in app/build/outputs/apk/release/*.apk; do
    echo "=== $apk ==="
    unzip -l "$apk" | grep '\.so$'
done
```

Expect one `.so` per ABI per APK split.

## Adding armv7 (only if required)

armv7 is the most painful ABI: smaller address space, distinct linker
quirks, and many GStreamer plugins are heavier there. Only add it if you
have real device coverage data forcing the issue.

```groovy
abiFilters "arm64-v8a", "x86_64", "armeabi-v7a"
splits.abi.include "arm64-v8a", "x86_64", "armeabi-v7a"
```

```bash
cargo ndk \
    -t arm64-v8a -t x86_64 -t armeabi-v7a \
    -o app/src/main/jniLibs build -p android-sender --release
```

If you hit `relocation R_ARM_THM_CALL out of range`, the cdylib is too
large; either split it or strip dead code with `-C link-arg=-Wl,--gc-sections`
and `RUSTFLAGS="-C codegen-units=1 -C opt-level=z"`.

## Device matrix to validate

| ABI | Validation device |
|---|---|
| `arm64-v8a` | Pixel/Galaxy physical device |
| `x86_64` | Android Studio emulator (system image: x86_64) |
| `armeabi-v7a` | Genymotion or a real 32-bit device |

Run the [Step 6 smoke test](./step-06-android-arm64-build.md) on each.

## Common failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `couldn't find libgstreamer.so` for new ABI | `GSTREAMER_ROOT_ANDROID` missing that ABI | Use universal tarball |
| `dlopen failed: cannot locate symbol "__aeabi_*"` on armv7 | Missing compiler builtins | Re-check `build.rs` clang_rt linker arg for armv7 (NDK r28c specifics) |
| Emulator launches but logcat shows `INVALID_ELF_CLASS` | Wrong ABI installed | `adb install --abi x86_64 ...` |
| APK size doubles | Forgot `splits.abi` | Enable per-ABI splits |

## Done when

- arm64 path from Step 6 still passes.
- A second ABI (`x86_64`) builds, installs, and runs the Step 6 smoke test
  inside an emulator.
- Per-ABI APK splits keep individual artifact size close to the original
  arm64-only APK.
