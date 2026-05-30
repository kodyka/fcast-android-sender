#!/usr/bin/env bash
set -euo pipefail

APK="${1:-app/build/outputs/apk/debug/app-debug.apk}"
PACKAGE="org.fcast.android.sender"
ACTIVITY="${PACKAGE}/.MainActivity"
LIB="${2:-target/aarch64-linux-android/debug/libfcastsender.so}"

if [[ ! -f "$APK" ]]; then
    echo "APK not found: $APK" >&2
    echo "Build it first with: ./gradlew --no-daemon :app:assembleDebug" >&2
    exit 1
fi

if [[ ! -f "$LIB" ]]; then
    echo "Native library not found: $LIB" >&2
    echo "Build it first with: cargo build --target=aarch64-linux-android -p android-sender" >&2
    exit 1
fi

adb install -r "$APK"
adb shell am start -n "$ACTIVITY"
sleep 5
adb shell am force-stop "$PACKAGE"

symbols="$(nm -D --defined-only "$LIB" | grep -c Java_org_fcast_android_sender)"
if [[ "$symbols" != "15" ]]; then
    echo "Expected 15 Java JNI symbols, found $symbols" >&2
    exit 1
fi

echo "OK"
