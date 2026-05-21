#!/usr/bin/env bash
# check-slint-viewer.sh — Verify the installed slint-viewer matches the
# Slint version pinned in Cargo.toml.
#
# Usage:
#   bash scripts/check-slint-viewer.sh
#
# If slint-viewer is not on PATH, the script prints the nix-shell command
# that installs a compatible version without polluting the global environment.

set -euo pipefail

CARGO_TOML="$(git rev-parse --show-toplevel)/Cargo.toml"

pinned=$(awk -F'"' '/^slint = / { print $2 }' "$CARGO_TOML" | head -1)
if [ -z "$pinned" ]; then
    echo "ERROR: could not parse slint version from Cargo.toml" >&2
    exit 1
fi

if ! command -v slint-viewer &>/dev/null; then
    echo "slint-viewer not found."
    echo ""
    echo "Run it without installing via nix-shell:"
    echo "  nix-shell -p slint-viewer --run \"slint-viewer ui/main.slint --auto-reload\""
    echo ""
    echo "Or install it permanently (must match pinned Slint $pinned):"
    echo "  cargo install slint-viewer --version =$pinned --force"
    exit 1
fi

have=$(slint-viewer --version 2>&1 | awk '{ print $NF }' | head -1)

if [ "$pinned" != "$have" ]; then
    echo "ERROR: slint-viewer version mismatch." >&2
    echo "  Cargo.toml pins Slint $pinned" >&2
    echo "  slint-viewer reports  $have" >&2
    echo "" >&2
    echo "Fix with nix-shell (no install needed):" >&2
    echo "  nix-shell -p slint-viewer --run \"slint-viewer ui/main.slint\"" >&2
    echo "" >&2
    echo "Or reinstall the matching version:" >&2
    echo "  cargo install slint-viewer --version =$pinned --force" >&2
    exit 1
fi

echo "OK: slint-viewer $have matches pinned Slint $pinned"
