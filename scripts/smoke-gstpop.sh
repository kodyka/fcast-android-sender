#!/usr/bin/env bash
set -euo pipefail

if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required for scripts/smoke-gstpop.sh" >&2
    exit 127
fi

DAEMON_ID=$(docker run --rm -d \
    -p 127.0.0.1:9000:9000 \
    --name gst-pop-smoke-$$ \
    ghcr.io/dabrain34/gstpop:latest)

cleanup() {
    docker stop "$DAEMON_ID" >/dev/null || true
}
trap cleanup EXIT

# gst-pop is a WebSocket-only server and refuses plain HTTP probes, so use a
# raw TCP connect instead of curl to detect readiness.
for _ in {1..50}; do
    if (exec 3<>/dev/tcp/127.0.0.1/9000) 2>/dev/null; then
        exec 3<&- 3>&-
        break
    fi
    sleep 0.2
done

nix --offline develop .#default -c cargo test backend::gstpop -- --include-ignored

nix --offline develop .#android -c bash ci/build-gstreamer-android-glue.sh
nix --offline develop .#android -c cargo build -p android-sender --target aarch64-linux-android
