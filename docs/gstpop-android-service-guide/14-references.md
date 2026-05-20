# 14 · References

Pointers into the current codebase. Line numbers are at the time of
writing — re-grep if they drift.

## 14.1 Daemon hosting

| File | Lines | Why |
|---|---|---|
| `src/backend/gstpop/embedded.rs` | 11–62 | Current statics, `ensure_started`, the existing port-probe and bind logic. The whole file is rewritten by step 2. |
| `src/backend/gstpop/backend.rs` | 62–66 | Implicit `ensure_started` call in `probe`. Removed by step 6. |
| `src/backend/gstpop/backend.rs` | 156–164 | Smoke test that already accepts externally-managed daemons. Stays as-is. |
| `vendor/gstpop/src/server.rs` | — | Upstream `ServerHandle`. Read but do not modify. |
| `vendor/gstpop/src/dbus/*.rs` | — | Linux-only DBus surface, gated by `cfg(target_os = "linux")`. Unrelated to this work. |

## 14.2 Backend lifecycle / Slint glue

| File | Lines | Why |
|---|---|---|
| `src/backend/lifecycle.rs` | 31–85 | Apply/Save/Probe callback registration. Step 7 adds two more callbacks here. |
| `src/backend/lifecycle.rs` | 88–99 | `apply`. Step 5 rewrites it. |
| `src/backend/lifecycle.rs` | 80–87 | `autostart`. Step 5 rewrites it. |
| `src/backend/mod.rs` | 14–28 | `MediaBackend` trait. No changes — just relevant context. |
| `src/backend/migration_backend.rs` | 1–60 | The other backend. Unchanged. |
| `src/backend/persistence.rs` | 1–60 | `StoredBackendConfig` (serde struct). No changes — already has the fields the service needs. |
| `src/backend/kind.rs` | 1–26 | `BackendKind`. No changes. |

## 14.3 JNI entrypoints

| File | Lines | Why |
|---|---|---|
| `src/lib.rs` | 1750–1764 | Where `BackendLifecycle::new` and `register` are called, alongside the runtime build. Step 3.2 stashes the `HOST_RUNTIME` near here. |
| `src/lib.rs` | 2456–2483 | Pattern reference for `Java_org_fcast_android_sender_<Class>_<method>` exports. Step 3.1 adds three new ones modelled on this. |
| `src/lib.rs` | 591–610 | First inline `vm_as_ptr` / `activity_as_ptr` block. Step 5.2 hoists this. |
| `src/lib.rs` | 1146–1156 | Second inline block. Same. |
| `src/lib.rs` | 529–599 | `JavaVM::from_raw` and `JObject::from_raw` usage examples (resolve_android_files_dir, etc.). |
| `src/lib.rs` | 2449–2453 | `jstring_to_string` helper used by `nativeGraphCommand`; reused by the new JNI exports. |

## 14.4 Android side

| File | Lines | Why |
|---|---|---|
| `app/src/main/AndroidManifest.xml` | 1–43 | Permissions, existing service block, MainActivity declaration. Step 4.2 diffs this. |
| `app/src/main/java/org/fcast/android/sender/MainActivity.java` | 212–213 | `System.loadLibrary("gstreamer_android")` + `System.loadLibrary("fcastsender")` — covers the new JNI exports. |
| `app/src/main/java/org/fcast/android/sender/MainActivity.java` | 156–158 | Native method declaration pattern for the discovery listener. |
| `app/src/main/java/org/fcast/android/sender/MainActivity.java` | 868–880 | Existing `startForegroundService` call for `ScreenCaptureService`. Pattern reference for step 3.3 / 4.1. |
| `app/src/main/java/org/fcast/android/sender/MainActivity.java` | 1138–1150 | Native method declarations at the bottom of the class. The new bridge keeps these out of MainActivity. |
| `app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java` | 1–84 | Template for `GstPopService`. Same notification + START_STICKY shape. |
| `app/build.gradle` | 1–55 | Manifest namespace, abi filter, dependency list. No changes for this work. |

## 14.5 Slint UI

| File | Lines | Why |
|---|---|---|
| `ui/bridge.slint` | 50–60 | `MediaBackendKind` / `MediaBackendState` enums. Step 7.1 adds `starting`. |
| `ui/bridge.slint` | 196–376 | `Bridge` global. Step 7.2 adds two callbacks + two properties. |
| `ui/bridge.slint` | 289–296 | Existing media backend state surface (kind / state / status text / error text / url / api-key / pipeline-id). |
| `ui/bridge.slint` | 316–320 | Existing media-backend callback block. New callbacks go here. |
| `ui/pages/media_backend_page.slint` | 40–120 | The Media Backend page. Step 7.3 + 7.4 edit this file. |
| `ui/components/buttons.slint` | — | `PrimaryButton`, `DestructiveButton`, `TextButton` reused by step 7.4. |
| `ui/components/settings_rows.slint` | — | `SettingsSection`. Reused by step 7.4. |
| `ui/theme.slint` | — | Colour tokens. Step 7.3 reuses `Theme.success`, `Theme.warning`, `Theme.error-fg`. |

## 14.6 Build infrastructure

| File | Why |
|---|---|
| `.github/workflows/gstpop-smoke.yml` | Docker-based smoke test that already plays nice with externally-managed daemons. No changes. |
| `.github/workflows/android.yml` | APK build job. Step 10.7 adds the JNI symbol grep. |
| `.github/actions/android-ci-setup/action.yml` | GStreamer SDK download URL — was fixed in PR #8. Mentioned for completeness. |
| `ci/build-rust-android-lib.sh` | Builds the cdylib. No changes — the new symbols build automatically once `lib.rs` exports them. |
| `Dockerfile` | Dev container. No changes. |
| `flake.nix` | Nix dev shell. No changes. |

## 14.7 Upstream references (for context)

| Source | Why |
|---|---|
| `dabrain34/gstpop` daemon | Originating implementation. `vendor/gstpop` is a near-mirror with a couple of local adaptations (pre-bound listener in `server.rs`, Linux-only DBus). |
| Android docs: `ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC` | Justifies the choice in step 4.2. |
| Android docs: foreground service start restrictions (API 31+) | Constrains step 4.1's `startForeground` timing. |

---

End of guide. Start from [00-plan-review.md](./00-plan-review.md).
