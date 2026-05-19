#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

: "${ANDROID_NDK_ROOT:?ANDROID_NDK_ROOT must be set}"
: "${GSTREAMER_ROOT_ANDROID:?GSTREAMER_ROOT_ANDROID must be set}"

mkdir -p "$ROOT/target"

export BUILD_SYSTEM="$ANDROID_NDK_ROOT/build/core"
export GSTREAMER_JAVA_SRC_DIR="$ROOT/app/src/main/java"
export NDK_PROJECT_PATH="$ROOT/app"
export GSTREAMER_NDK_BUILD_PATH="$GSTREAMER_ROOT_ANDROID/share/gst-android/ndk-build"

cd "$ROOT/target"
make -f "$ANDROID_NDK_ROOT/build/core/build-local.mk"
