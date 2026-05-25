#!/usr/bin/env bash
set -euo pipefail

ROOT="${CONDUCTOR_WORKSPACE_PATH:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$ROOT"

WORKSPACE_NAME="${CONDUCTOR_WORKSPACE_NAME:-$(basename "$ROOT")}"
RUNTIME_KEY="$(printf '%s' "$ROOT:$WORKSPACE_NAME" | cksum | awk '{print $1}')"
RUNTIME_DIR="${JCODE_RUNTIME_DIR:-${TMPDIR:-/tmp}/jcode-conductor-$RUNTIME_KEY}"

rm -rf "$RUNTIME_DIR"

rm -rf \
  .context/DerivedData \
  ios/.build \
  ios/JCodeMobile.xcodeproj \
  target/mobile-ios
