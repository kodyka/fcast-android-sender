# 07 — Build / package gotchas

A short collection of things that have bitten similar JNI integrations
and would silently fail if missed.

## 7.1 No `build.gradle` changes

The two new files live in the existing `org.fcast.android.sender`
package; the cdylib (`libfcastsender.so`) picks up the three new JNI
exports because they are conditional on `cfg(target_os = "android")`
and share the existing crate build.

If the Android Gradle Plugin's `sourceSets` config in
`app/build.gradle` is restrictive enough to ignore one of the new
files, double-check that `src/main/java` is in the `java.srcDirs` list
(it is, on `main` — this is just a safety note).

## 7.2 Symbol matching

The JNI symbol must be byte-for-byte:

```
Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStartMigrationRuntimeHost
Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStopMigrationRuntimeHost
Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeGetMigrationRuntimeStatus
```

If you rename either side, rename both. The function name segment after
`Java_<package>_<class>_` is the Java method name with underscores in
the original method name doubled (`_` → `_1`). The three method names
here contain no underscores, so they map directly.

## 7.3 Required Rust attributes

`#[unsafe(no_mangle)]`, `#[allow(non_snake_case)]`, and
`pub extern "C" fn` are required exactly as shown in
[01-rust-jni-bridge.md §1.1](./01-rust-jni-bridge.md#11-three-new-exports).
These match the existing gst-pop exports at `src/lib.rs:2991-3037`.

Common ways to break this:

* Forgetting `#[unsafe(no_mangle)]` — the symbol gets a hash suffix
  and the JVM cannot find it at runtime (`UnsatisfiedLinkError`).
* Forgetting `extern "C"` — silent UB on parameter passing.
* Using `pub fn` without `extern "C"` — same as above.

## 7.4 ProGuard / R8

The repo's current debug build does not enable shrinking, so the new
classes survive minification by default. When the eventual release
build adds R8, add `-keep` rules:

```proguard
# app/proguard-rules.pro
-keep class org.fcast.android.sender.GstPopServiceBridge { *; }
-keep class org.fcast.android.sender.MigrationRuntimeServiceBridge { *; }
```

Both classes need the rule because the JNI-export-side symbol resolution
goes through `findClass` / `loadClass`, which R8 can't see through.

## 7.5 `cargo check` matrix

Make sure the three new exports compile on all build targets:

```bash
cargo check --target aarch64-linux-android      # main target
cargo check --target x86_64-linux-android       # emulator
cargo check                                     # host (must not pick up the new exports)
```

Because every export is `#[cfg(target_os = "android")]`, the host build
must not reference them. If you accidentally drop the cfg-gate, the
host build will fail with missing `migration::runtime::start_graph_runtime`
on a non-Android target — though as of writing `start_graph_runtime` is
not Android-gated, so the error will be at the JNI types
(`jni::JNIEnv` is not exposed on host).

## 7.6 NDK / linker quirks

The cdylib is built once per ABI; nothing about the new exports requires
a different linker setup than what `GstPopServiceBridge` already uses.
If you see "undefined symbol Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_*"
at runtime, the most common cause is that the cdylib was not rebuilt
since the Java file was added — invalidate the Gradle cache and run
`./gradlew assembleDebug --rerun-tasks`.

## 7.7 Tracing / log filtering

Both `MigrationRuntimeService` and `MigrationRuntimeServiceBridge` use
their class names as Logcat tags:

```
MigrationRuntimeService
MigrationRuntimeServiceBridge
```

When debugging, filter with:

```bash
adb logcat -s MigrationRuntimeService MigrationRuntimeServiceBridge \
              GstPopService GstPopServiceBridge
```

On the Rust side, the JNI exports do not emit `tracing::info!` events
today — if you want logs there, follow the convention in the existing
gst-pop exports (which also emit no logs and rely on the underlying
`embedded::start_embedded` to do its own tracing). Add `tracing::info!`
calls inside `migration::runtime::start_graph_runtime` /
`shutdown_graph_runtime` rather than in `lib.rs`.
