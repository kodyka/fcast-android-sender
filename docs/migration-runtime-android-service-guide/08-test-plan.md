# 08 — Test plan

When the implementation PR lands, the following acceptance checks
verify it end-to-end. They mirror the gst-pop guide's
[`10-test-plan.md`](../gstpop-android-service-guide/10-test-plan.md).

## 8.1 Static checks

* **`cargo check --target aarch64-linux-android`** — compiles the
  three new JNI exports against the migration-runtime functions.
* **`cargo clippy --target aarch64-linux-android --all-features -- -D warnings`** —
  catches unused imports introduced when shuttling
  `migration_runtime_status_json` between files.
* **`./gradlew :app:compileDebugJavaWithJavac`** — verifies the two
  new Java files compile cleanly. The native method declarations are
  resolved at runtime, so compile success does **not** prove the JNI
  symbols line up — see §8.3.
* **`./gradlew assembleDebug`** — verifies the two new Java files and
  the manifest entry are picked up by the AGP merge step.
* **`pre-commit run --all-files`** — runs the four hooks defined in
  `.pre-commit-config.yaml`. The Slint snippets in step 6 are
  hook-clean by construction.

## 8.2 Manifest verification

```bash
./gradlew :app:processDebugManifest
cat app/build/intermediates/merged_manifests/debug/AndroidManifest.xml \
  | grep -A 4 MigrationRuntimeService
```

Expected:

```xml
<service
    android:name="org.fcast.android.sender.MigrationRuntimeService"
    android:exported="false"
    android:stopWithTask="false"
    android:foregroundServiceType="dataSync" />
```

If the `android:name` is missing the package prefix, the leading dot in
`.MigrationRuntimeService` (step 4) was dropped — restore it.

## 8.3 Native-symbol verification

After `./gradlew assembleDebug` succeeds:

```bash
unzip -p app/build/outputs/apk/debug/app-debug.apk lib/arm64-v8a/libfcastsender.so \
  > /tmp/libfcastsender.so
readelf -Ws /tmp/libfcastsender.so \
  | grep -E 'MigrationRuntimeServiceBridge_native(Start|Stop|Get)MigrationRuntime'
```

Expected three matching lines, one per export. If any are missing, the
`#[unsafe(no_mangle)]` / `extern "C"` attributes were dropped (see
[07-build-and-package.md §7.3](./07-build-and-package.md#73-required-rust-attributes)).

## 8.4 Runtime smoke test (no UI needed)

After installing the debug APK on a device:

```bash
adb shell am start-foreground-service \
  -n org.fcast.android.sender/.MigrationRuntimeService \
  -a org.fcast.android.sender.MIGRATION_RUNTIME_START
```

Then:

```bash
adb shell dumpsys activity services org.fcast.android.sender
```

Expected:

* `MigrationRuntimeService` is listed.
* `foreground=true` with `foregroundServiceType=dataSync`.
* Notification ID 3 is alive on the device (visible in the shade).

Stop it:

```bash
adb shell am start-foreground-service \
  -n org.fcast.android.sender/.MigrationRuntimeService \
  -a org.fcast.android.sender.MIGRATION_RUNTIME_STOP
```

After ~100 ms, `dumpsys activity services` should no longer list the
service.

## 8.5 UI smoke test (after step 6 is implemented)

1. Launch the app, open **Media Backend** panel.
2. Switch the backend to **Migration**. The new `SERVICE` section
   appears.
3. Initial state pill: grey, text "Migration runtime stopped".
4. Tap **Start service**. Within ~1 s the pill turns green and
   reads "Migration runtime running"; the notification appears.
5. Tap **Stop service**. Within ~1 s the pill returns to grey and the
   notification disappears.

## 8.6 Survival checks

* **Task removal.** Start the service, swipe the app from Recents.
  Because `android:stopWithTask="false"`, the service must
  **survive** task removal. Verify the notification is still present
  and `dumpsys activity services` still lists the service.
* **Process kill.** With the service running, `adb shell am
  kill org.fcast.android.sender`. The system kills the process →
  `onDestroy` fires → `nativeStop()` is invoked → next launch of the
  app sees a clean state (the foreground service is **not**
  auto-restarted because `onStartCommand` returns `START_NOT_STICKY`
  for null intents — see [03-android-service.md §3.4](./03-android-service.md#34-why-start_not_sticky-after-a-null-intent-restart)).
* **`stop` while already stopped.** Tap **Stop service** when the pill
  reads `stopped`. Must be a no-op (no crash, no notification flicker).
  This is guaranteed by `shutdown_graph_runtime` being idempotent
  (`runtime.rs:312-320`).

## 8.7 Cross-service interaction

The migration runtime and gst-pop services can run independently. With
both running:

* **Notification shade** shows two FCast notifications (IDs 2 and 3).
* **`dumpsys`** lists both services.
* Stopping one does not affect the other.

If you observe the migration notification being replaced by the gst-pop
notification or vice versa, you used the same `CHANNEL_ID` for both —
re-check the channel constants in
[03-android-service.md §3.1](./03-android-service.md#31-full-file-content)
and `GstPopService.java:27`.

## 8.8 Logging output to capture for the PR

```bash
adb logcat -s MigrationRuntimeService MigrationRuntimeServiceBridge \
              GstPopService GstPopServiceBridge \
  | tee migration-service-test.log
```

Run the [§8.4](#84-runtime-smoke-test-no-ui-needed) and
[§8.5](#85-ui-smoke-test-after-step-6-is-implemented) flows with logcat
recording, and attach the log to the implementation PR. The expected
sequence per start cycle is:

```
MigrationRuntimeService  D onStartCommand action=org.fcast.android.sender.MIGRATION_RUNTIME_START
MigrationRuntimeService  D nativeStart -> {"state":"running"}
MigrationRuntimeService  D onStartCommand action=org.fcast.android.sender.MIGRATION_RUNTIME_STOP
MigrationRuntimeService  D nativeStop -> {"state":"stopped"}
```
