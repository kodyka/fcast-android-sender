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

for _ in {1..30}; do
    if curl -s --max-time 1 http://127.0.0.1:9000 >/dev/null 2>&1; then
        break
    fi
    sleep 0.2
done

nix --offline develop .#default -c cargo test backend::gstpop -- --include-ignored

nix --offline develop .#android -c bash ci/build-gstreamer-android-glue.sh
nix --offline develop .#android -c cargo build -p android-sender --target aarch64-linux-android
