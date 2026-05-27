# 04 — AndroidManifest.xml diff

Insert one new `<service>` block immediately after the existing
`GstPopService` declaration (after current line 30).

## 4.1 Diff

```diff
         <service
             android:name=".GstPopService"
             android:exported="false"
             android:stopWithTask="false"
             android:foregroundServiceType="dataSync" />

+        <service
+            android:name=".MigrationRuntimeService"
+            android:exported="false"
+            android:stopWithTask="false"
+            android:foregroundServiceType="dataSync" />
+

         <activity
             android:name="com.journeyapps.barcodescanner.CaptureActivity"
```

## 4.2 Attribute rationale

| Attribute | Value | Why |
|-----------|-------|-----|
| `android:name` | `.MigrationRuntimeService` | Relative class name — resolved against the manifest `package`. |
| `android:exported` | `false` | The service has no IPC interface (`onBind` returns null). Other apps cannot start it. |
| `android:stopWithTask` | `false` | Matches `GstPopService`. The service must outlive task removal so a paused app coming back to foreground sees an already-warm runtime. (Contrast: `ScreenCaptureService` uses `stopWithTask="true"` because screen capture loses its `MediaProjection` on task removal anyway.) |
| `android:foregroundServiceType` | `"dataSync"` | Matches `GstPopService`. The migration runtime moves protocol frames between in-process pipelines and is best characterised as a long-running data-sync workload. |

## 4.3 No new permissions

The two permissions that cover this combination are already declared
on lines 5 and 7 of the current manifest:

| Permission | Declared at | Covers |
|------------|-------------|--------|
| `android.permission.FOREGROUND_SERVICE` | `AndroidManifest.xml:5` | `startForegroundService(...)` for any foregroundServiceType. |
| `android.permission.FOREGROUND_SERVICE_DATA_SYNC` | `AndroidManifest.xml:7` | The specific `dataSync` type. Required on API 34+. |

If you ever change `foregroundServiceType` away from `"dataSync"`, you
must add the matching `FOREGROUND_SERVICE_*` permission.

## 4.4 Service ordering inside `<application>`

Insert the new `<service>` right after `GstPopService` (current line 30)
and before the first `<activity>` (current line 33). Service ordering
inside `<application>` is not semantically significant, but keeping
related services next to each other simplifies diff review and
maintenance.

## 4.5 Verifying the manifest

After implementing, the merged manifest can be inspected with:

```bash
./gradlew :app:processDebugManifest
cat app/build/intermediates/merged_manifests/debug/AndroidManifest.xml \
  | grep -A 4 MigrationRuntimeService
```

Expected output mirrors the `<service>` element you added, with the
`android:` namespace expanded.
