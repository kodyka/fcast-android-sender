# 11 — Build-stack upgrade

**Priority:** Medium · **Effort:** High · **Estimated PR size:** 3 PRs of ~50 LOC each + bug-fix follow-ups.

## Goal

Bring the Gradle wrapper, Android Gradle Plugin, NDK, compileSdk/targetSdk, and
Kotlin baseline forward in controlled increments. Verify native GStreamer
compatibility at each step.

## Report finding

> "Official sources show AGP 9.2.0, Gradle 9.5.0, and current Kotlin 2.3.x
> releases, while the repo is still on compile/target SDK 34 and Gradle 8.9,
> with AGP pinned in the 8.7 generation."
>
> "Android 16 is API level 36 and changes behaviour around orientation/
> resizability on large screens, which is relevant because the app explicitly
> sets capture and activity orientation/resizability-related manifest flags."
>
> "AGP 9.0 also documents a default move to NDK r28c, which could interact badly
> with the project's current native scripts unless validated deliberately."

— `deep-research-report-3.md`, "Detailed findings" and "Testing, CI, risk and rollback".

## Pre-state on `main`

Verified versions:

| Tool                          | Current value                     | Source                                                    |
|-------------------------------|-----------------------------------|-----------------------------------------------------------|
| Gradle wrapper                | 8.9                               | `gradle/wrapper/gradle-wrapper.properties:4`              |
| Android Gradle Plugin (AGP)   | 8.7.0                             | `gradle/libs.versions.toml:2`                             |
| `compileSdk`                  | 34                                | `app/build.gradle:7`                                       |
| `targetSdk`                   | 34                                | `app/build.gradle:21`                                      |
| `minSdk`                      | 26                                | `app/build.gradle:20`                                      |
| NDK                           | r25c                              | `.github/actions/android-ci-setup` + `.gitlab-ci.yml`     |
| GStreamer Android             | 1.28.0                            | `.gitlab-ci.yml:24` & composite action                    |
| Kotlin plugin                 | not applied (Java shell today)    | `app/build.gradle` plugins block                          |
| Java toolchain (CI)           | 21                                | composite action                                          |

## Target state (recommended end state)

| Tool                          | Target                            | Notes                                                            |
|-------------------------------|-----------------------------------|------------------------------------------------------------------|
| Gradle wrapper                | 9.x (latest 9.5+)                 | Bumping the wrapper alone is safe; AGP can follow.               |
| AGP                           | 9.2.0                             | Major bump — see risks below.                                    |
| `compileSdk` / `targetSdk`    | 36 (Android 16)                   | Read the orientation/resizability behaviour-change notes.       |
| `minSdk`                      | 26 (unchanged)                    | Bumping minSdk is a product call; keep 26 for this step.        |
| NDK                           | r28c (AGP 9 default)              | Validate GStreamer Android first.                                |
| GStreamer Android             | 1.28.0 (unchanged this step)      | Track separately — this guide does not propose 1.28→1.30.        |
| Kotlin                        | 2.3.x                             | Required if step 08 has landed.                                  |
| Java toolchain                | 21                                | Already there.                                                   |

## Risk register

| Risk                                                                     | Mitigation                                                                                 |
|--------------------------------------------------------------------------|--------------------------------------------------------------------------------------------|
| GStreamer Android 1.28.0 + NDK r28c ABI breakage                         | Build `aarch64-linux-android` against r28c on a feature branch first. Keep r25c CI cache.   |
| Android 16 orientation/resizability behaviour change                     | Audit `AndroidManifest.xml` for `screenOrientation`, `resizeableActivity`, `configChanges`. Add a manual test plan for foldables / large screens. |
| AGP 9 namespace / manifest-merger stricter rules                         | Run `./gradlew :app:assembleDebug` with `-Pandroid.suppressUnsupportedCompileSdk=…` to surface warnings as errors during the upgrade PR. |
| Native-build script assumptions                                          | `ci/build-rust-android-lib.sh` and `ci/build-gstreamer-android-glue.sh` reference NDK paths — they must be re-validated under r28c. |
| Kotlin 2.3 vs existing Java code                                         | The Java code does not stop compiling, but pull `kotlin-stdlib` 2.3 + `kotlinx-coroutines` 1.9 together. |

## Upgrade recipe (sub-PRs)

### PR 11.1 — Gradle wrapper 8.9 → 9.x

```bash
./gradlew wrapper --gradle-version 9.5.0 --distribution-type bin
```

Commit `gradle/wrapper/gradle-wrapper.properties` and `gradle/wrapper/gradle-wrapper.jar`.

Smoke-build:

```bash
./gradlew --no-daemon :app:assembleDebug
```

Bump only after CI is green. Do not touch AGP yet.

### PR 11.2 — AGP 8.7 → 8.13 (stay in the 8.x line)

```diff
 # gradle/libs.versions.toml
- agp = "8.7.0"
+ agp = "8.13.0"
```

8.13 is the latest 8.x and is the natural intermediate before 9.x. Run the
debug build and the headless Slint tests.

### PR 11.3 — `compileSdk` 34 → 35

```diff
 # app/build.gradle
- compileSdk 34
+ compileSdk 35
```

Keep `targetSdk` at 34. This catches Android 15 (API 35) deprecation warnings
without changing runtime behaviour. Required for AGP 9.

### PR 11.4 — AGP 8.13 → 9.0.x

```diff
- agp = "8.13.0"
+ agp = "9.0.1"
```

AGP 9 changes the default NDK to r28c. Either:

- Update CI to install r28c (`.github/actions/android-ci-setup` + `.gitlab-ci.yml`).
- **Or** override the NDK version in `app/build.gradle`:

```diff
 android {
+    ndkVersion "25.2.9519653"   // pin r25c through this PR
 }
```

Pin to r25c if GStreamer Android 1.28.0 fails to link under r28c. Track the
NDK move as its own follow-up.

### PR 11.5 — `targetSdk` 34 → 36 (Android 16)

```diff
- targetSdk 34
+ targetSdk 36
- compileSdk 35
+ compileSdk 36
```

Re-audit the manifest for the orientation/resizability behaviour change:

```bash
rg -n 'screenOrientation|resizeableActivity|configChanges|foregroundServiceType' app/src/main/AndroidManifest.xml
```

For each match, decide:

- Is the value still appropriate on Android 16?
- Does the activity need `android:configChanges="screenSize|orientation|screenLayout|smallestScreenSize"` to avoid a re-create on orientation flip?
- Are foreground-service types declared with the granular flags required from API 34+?

### PR 11.6 — Kotlin baseline

If step 08 has landed:

```diff
 # gradle/libs.versions.toml
+ kotlin = "2.3.0"

 [plugins]
+ kotlin-android = { id = "org.jetbrains.kotlin.android", version.ref = "kotlin" }
```

```diff
 # app/build.gradle
 plugins {
     alias(libs.plugins.android.application)
+    alias(libs.plugins.kotlin.android)
 }
```

### PR 11.7 — NDK r25c → r28c

Switch CI installs and the optional `ndkVersion` pin:

```diff
- ANDROID_NDK_ROOT="$THIRDPARTY_DEPS_PATH/android-ndk-r25c"
+ ANDROID_NDK_ROOT="$THIRDPARTY_DEPS_PATH/android-ndk-r28c"
```

Validate the GStreamer link line. If it fails, hold this PR until the GStreamer
side ships a known-good 1.28.x or 1.30 release.

## Testing (per sub-PR)

| Check                                                            | Command                                                                  |
|------------------------------------------------------------------|--------------------------------------------------------------------------|
| Debug APK builds                                                 | `./gradlew --no-daemon :app:assembleDebug`                               |
| Release APK builds (PR 11.5 onwards)                             | `./gradlew --no-daemon :app:assembleRelease` with signing skipped.       |
| Headless Slint UI tests                                          | `cargo test -p fcastsender --test ui_snapshots`                          |
| Rust tests                                                       | `cargo test -p fcastsender`                                              |
| Native lib links                                                 | `nm -D --defined-only target/aarch64-linux-android/debug/libfcastsender.so | head` |
| Lint (must not regress)                                          | `./gradlew :app:lint`                                                    |
| Smoke on a real device                                           | Manual: launch + cast + stop.                                            |

## Rollback

Each sub-PR is independently revertable. The riskiest is **PR 11.4** (AGP 9). If
a regression appears only after 11.4, revert that PR and stay on AGP 8.13 +
Gradle 9 — that combination is supported and gives you most of the build-perf
wins without the AGP 9 risks.

## Follow-ups (not in this PR)

- GStreamer Android 1.28 → 1.30 (if released and validated).
- Performance pass — **Step 12**.
- Rollout / soak plan — **Step 13**.
