#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

: "${GSTREAMER_ROOT_ANDROID:?GSTREAMER_ROOT_ANDROID must be set}"

MODE="${1:-debug}"
TARGET="aarch64-linux-android"
OUT_DIR="$ROOT/app/src/main/jniLibs"

export PKG_CONFIG_PATH="$GSTREAMER_ROOT_ANDROID/arm64/lib/pkgconfig"

cd "$ROOT"

args=(
  --target "$TARGET"
  -o "$OUT_DIR"
  build
  --package android-sender
)

if [[ "$MODE" == "release" ]]; then
  args+=(--release)
elif [[ "$MODE" != "debug" ]]; then
  echo "unsupported build mode: $MODE" >&2
  exit 2
fi

cargo ndk "${args[@]}"
