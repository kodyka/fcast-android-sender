# fcast-android-sender

Standalone Android sender app for the [FCast protocol](https://github.com/kodyka/fcast).

Extracted from `kodyka/fcast` at commit `63980e6736e65adbd15588d21903d0c02223c15c`
via MVP phase 10.

## Building

The Android build still expects the same toolchain components the monorepo used:

- Rust toolchain with Android targets
- Android SDK command-line tools
- Android NDK r25c
- GStreamer Android SDK 1.28.0
- Java 21

### Required environment

`build.rs` is a no-op on non-Android targets. On Android targets
(`aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`,
`i686-linux-android`), export:

| Variable | Purpose |
| --- | --- |
| `ANDROID_NDK_ROOT` or `ANDROID_NDK_HOME` | Path to Android NDK r25c |
| `GSTREAMER_ROOT_ANDROID` | Path to GStreamer Android SDK 1.28.0 |

For Gradle builds, also export:

| Variable | Purpose |
| --- | --- |
| `ANDROID_HOME` | Path to Android SDK |
| `ANDROID_SDK_ROOT` | Alias for `ANDROID_HOME` |

### Cargo

```console
$ cargo check --target aarch64-linux-android
$ cargo build --release --target aarch64-linux-android
```

### Gradle

```console
$ ./gradlew assembleDebug
$ ./gradlew installDebug
```

## Repository layout

- `Cargo.toml`, `build.rs`, `src/`: `android-sender` Rust crate
- `ui/`: Slint UI tree
- `ui/components/mcore/`, `ui/components/std/`: vendored Slint helpers copied from `kodyka/fcast`
- `app/`: Android shell, JNI glue, resources
- `ci/`: UI validation script
- `gradle/`, `gradlew*`, `build.gradle`, `settings.gradle`: Gradle project
- `docs/`: extraction and cross-repo maintenance docs

## SDK dependencies

The crate depends on three SDK crates that remain in the FCast monorepo:

- `fcast-protocol`
- `fcast-sender-sdk`
- `mcore`

They are pinned as Git dependencies to `kodyka/fcast` commit
`63980e6736e65adbd15588d21903d0c02223c15c`.

To bump the SDK pin, see [docs/cross-repo-sync.md](docs/cross-repo-sync.md).

## License

MIT
