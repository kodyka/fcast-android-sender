# 09 — CI / CD consolidation

**Priority:** Medium · **Effort:** Medium · **Estimated PR size:** ~150 LOC across YAML files.

## Goal

Consolidate the release pipeline onto GitHub Actions, add SDK / NDK / GStreamer
toolchain caching, add release signing, and freeze (but keep) the GitLab pipeline
as a documented fallback.

## Report finding

> "GitHub Actions builds an arm64 debug APK artifact, while the GitLab pipeline
> still manually assembles an unsigned release APK, which points to CI/CD
> duplication and a release process that is not yet fully consolidated."
>
> "The Android CI setup action downloads SDK, NDK, and GStreamer toolchains
> directly and I did not find caching in that action. That means slower builds,
> duplicate pipeline logic, and a release process that still looks transitional."

— `deep-research-report-3.md`, "Testing, CI, risk and rollback".

## Pre-state on `main`

Verified files on `main`:

| File                                                  | Purpose                                                                                |
|-------------------------------------------------------|----------------------------------------------------------------------------------------|
| `.github/workflows/android-release-apk.yml`           | Debug APK build + upload. No release-signing path.                                     |
| `.github/workflows/gstpop-smoke.yml`                  | `cargo test backend::gstpop -- --include-ignored --test-threads=1`.                    |
| `.github/workflows/slint-viewer-smoke.yml`            | Slint viewer compilation smoke.                                                        |
| `.github/workflows/ui-lint.yml`                       | `ci/ui-validate.sh --no-build`.                                                        |
| `.gitlab-ci.yml`                                      | `assembleRelease` → `app-release-unsigned.apk` artifact, manual trigger.                |
| `.github/actions/android-ci-setup/` (composite action)| Installs SDK + NDK r25c + GStreamer 1.28.0. No cache.                                  |
| `scripts/smoke-gstpop.sh`                             | Docker-based gst-pop smoke harness.                                                    |

The verified test-thread gate is real:

```
.github/workflows/gstpop-smoke.yml:101:    # --test-threads=1 required: two ignored tests share process-global atomics
.github/workflows/gstpop-smoke.yml:103:    run: cargo test backend::gstpop -- --include-ignored --test-threads=1
```

The GitLab pipeline produces `app-release-unsigned.apk` and stores it as an
artifact for 30 days. There is no signing step anywhere in the pipeline.

## Target state

```
.github/
├── actions/
│   └── android-ci-setup/             ← gains aggressive caching
└── workflows/
    ├── android-debug-apk.yml         ← renamed from android-release-apk.yml
    ├── android-release-apk.yml       ← NEW: signed release pipeline
    ├── gstpop-smoke.yml              ← unchanged
    ├── slint-viewer-smoke.yml        ← unchanged
    └── ui-lint.yml                   ← unchanged

.gitlab-ci.yml                        ← frozen with a banner comment, kept for rollback
```

## Add caching to the composite action

The composite at `.github/actions/android-ci-setup/action.yml` downloads SDK
command-line tools, NDK r25c, and GStreamer 1.28.0. Add hash-keyed caches for
each download. Use `actions/cache@v4` with content-addressed keys:

```yaml
# .github/actions/android-ci-setup/action.yml   (addition near the top of `runs:`)

- name: Cache Android SDK command-line tools
  uses: actions/cache@v4
  with:
    path: ${{ runner.tool_cache }}/android-sdk
    key: android-sdk-${{ runner.os }}-cmdline-${{ inputs.sdk-tools-version }}
    restore-keys: |
      android-sdk-${{ runner.os }}-cmdline-

- name: Cache Android NDK r25c
  uses: actions/cache@v4
  with:
    path: ${{ runner.tool_cache }}/android-ndk
    key: android-ndk-${{ runner.os }}-r25c

- name: Cache GStreamer Android 1.28.0
  uses: actions/cache@v4
  with:
    path: ${{ runner.tool_cache }}/gstreamer-android
    key: gstreamer-android-${{ runner.os }}-1.28.0

- name: Cache cargo registry & target
  uses: actions/cache@v4
  with:
    path: |
      ~/.cargo/registry
      ~/.cargo/git
      target
    key: cargo-${{ runner.os }}-${{ inputs.rust-target }}-${{ hashFiles('**/Cargo.lock') }}
    restore-keys: |
      cargo-${{ runner.os }}-${{ inputs.rust-target }}-
```

Then change each `curl … && unzip …` step to short-circuit when the cache
directory is non-empty:

```yaml
- name: Download Android NDK r25c (only on cache miss)
  shell: bash
  run: |
    if [[ -d "${{ runner.tool_cache }}/android-ndk/android-ndk-r25c" ]]; then
      echo "NDK already cached"; exit 0
    fi
    # …existing download/extract steps…
```

A maintained CI image is the next-best alternative if cache hit-rates are low
in practice. Build it from the same Dockerfile that GitLab already uses, then
have the GitHub workflows `runs-on: ubuntu-24.04` with `container:` set.

## Rename the debug pipeline

`.github/workflows/android-release-apk.yml` currently builds a *debug* APK
despite its name. Rename it:

```diff
- name: Android APK Release
+ name: Android Debug APK
```

…and update any badges / docs that reference the old name.

## Add a real release pipeline

`.github/workflows/android-release-apk.yml` (NEW, replacing nothing):

```yaml
name: Android Release APK

on:
  release:
    types: [published]
  workflow_dispatch:
    inputs:
      version-name:
        description: "Version name (e.g. 1.4.0)"
        required: true
      version-code:
        description: "Version code (integer)"
        required: true

permissions:
  contents: write

jobs:
  build-release:
    runs-on: ubuntu-24.04
    env:
      CARGO_TERM_COLOR: always
    steps:
      - uses: actions/checkout@v4

      - name: Set up Android CI toolchain
        uses: ./.github/actions/android-ci-setup
        with:
          rust-target: aarch64-linux-android

      - name: Build GStreamer Android JNI glue
        run: bash ci/build-gstreamer-android-glue.sh

      - name: Build arm64 Rust Android library (release)
        run: bash ci/build-rust-android-lib.sh release

      - name: Decode signing keystore
        env:
          KEYSTORE_BASE64: ${{ secrets.FCAST_KEYSTORE_BASE64 }}
        run: echo "$KEYSTORE_BASE64" | base64 -d > $RUNNER_TEMP/release.jks

      - name: Build signed release APK
        env:
          FCAST_KEYSTORE_PATH:     ${{ runner.temp }}/release.jks
          FCAST_KEYSTORE_PASSWORD: ${{ secrets.FCAST_KEYSTORE_PASSWORD }}
          FCAST_KEY_ALIAS:         ${{ secrets.FCAST_KEY_ALIAS }}
          FCAST_KEY_PASSWORD:      ${{ secrets.FCAST_KEY_PASSWORD }}
          FCAST_VERSION_NAME:      ${{ inputs.version-name || github.event.release.tag_name }}
          FCAST_VERSION_CODE:      ${{ inputs.version-code }}
        run: |
          ./gradlew --no-daemon assembleRelease \
            -PversionName=$FCAST_VERSION_NAME \
            -PversionCode=$FCAST_VERSION_CODE

      - name: Upload signed APK to release
        if: github.event_name == 'release'
        uses: softprops/action-gh-release@v2
        with:
          files: app/build/outputs/apk/release/app-release.apk
```

`app/build.gradle` must declare a `signingConfig` that reads
`FCAST_KEYSTORE_*` from environment variables. That diff is small and lives in
the same PR.

```diff
 android {
     signingConfigs {
+        release {
+            storeFile     file(System.getenv("FCAST_KEYSTORE_PATH") ?: "release.jks")
+            storePassword System.getenv("FCAST_KEYSTORE_PASSWORD") ?: ""
+            keyAlias      System.getenv("FCAST_KEY_ALIAS")         ?: ""
+            keyPassword   System.getenv("FCAST_KEY_PASSWORD")      ?: ""
+        }
     }
     buildTypes {
         release {
+            signingConfig signingConfigs.release
             minifyEnabled false
             proguardFiles getDefaultProguardFile('proguard-android-optimize.txt'), 'proguard-rules.pro'
         }
     }
 }
```

## Freeze the GitLab pipeline (do not delete)

Add a banner comment to `.gitlab-ci.yml` and keep `when: manual`:

```diff
+# DEPRECATED: kept for rollback until two consecutive GitHub-driven releases
+# ship successfully. Do not extend. See docs/refactor-implementation-guide/09-ci-cd-consolidation.md.
+
 buildAndroidSenderDockerContainer:
```

Per the report's rollback table:

> "Keep GitLab pipeline frozen but available until two successful GitHub release
> runs."

## Required GitHub secrets

| Secret                         | Source                                  |
|--------------------------------|-----------------------------------------|
| `FCAST_KEYSTORE_BASE64`        | `base64 -w0 release.jks`.               |
| `FCAST_KEYSTORE_PASSWORD`      | Keystore password.                      |
| `FCAST_KEY_ALIAS`              | Alias name in the keystore.             |
| `FCAST_KEY_PASSWORD`           | Per-key password.                       |

Store via `gh secret set FCAST_KEYSTORE_BASE64 < <(base64 -w0 release.jks)` etc.

## Testing

| Test                                                              | How                                                                      |
|-------------------------------------------------------------------|--------------------------------------------------------------------------|
| Debug pipeline still green                                         | Push to the PR branch — `android-debug-apk.yml` runs.                    |
| Cache hits on a re-run                                             | Re-run the workflow; tool-cache and cargo-cache steps report `Cache restored`. |
| Release pipeline (dry-run)                                         | `workflow_dispatch` with a `0.0.0-test` version-name; confirm signed APK artifact attaches. |
| GitLab pipeline still triggers manually                            | One manual run on `main` after the freeze comment lands.                 |
| Signed APK installs                                                | `adb install -r app-release.apk` on a clean device.                      |
| `apksigner verify` passes                                          | `apksigner verify --verbose app-release.apk`.                            |

## Rollback

- Revert the `android-release-apk.yml` file. Debug pipeline is untouched.
- Revert the caching block in the composite action; pipelines re-download
  toolchains as before.
- Remove the freeze banner from `.gitlab-ci.yml`.
- Keystore secrets stay in GitHub; deleting them is optional.

## Follow-ups (not in this PR)

- Tests follow — **Step 10**.
- Toolchain upgrade — **Step 11**.
- Move the test-thread gate to the Rust side (eliminate global test state) —
  noted in step 07 sub-PR 7.5.
