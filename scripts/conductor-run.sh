#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$PATH"

ROOT="${CONDUCTOR_WORKSPACE_PATH:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$ROOT"

PORT="${CONDUCTOR_PORT:-7643}"
BIND_ADDR="${JCODE_GATEWAY_BIND_ADDR:-0.0.0.0}"
PROVIDER="${JCODE_PROVIDER:-auto}"
BINARY="$ROOT/target/debug/jcode"
WORKSPACE_NAME="${CONDUCTOR_WORKSPACE_NAME:-$(basename "$ROOT")}"
RUNTIME_KEY="$(printf '%s' "$ROOT:$WORKSPACE_NAME" | cksum | awk '{print $1}')"
RUNTIME_DIR="${JCODE_RUNTIME_DIR:-${TMPDIR:-/tmp}/jcode-conductor-$RUNTIME_KEY}"

if [ ! -x "$BINARY" ]; then
  cargo build --bin jcode
fi

mkdir -p "$RUNTIME_DIR"

export JCODE_RUNTIME_DIR="$RUNTIME_DIR"
export JCODE_GATEWAY_ENABLED=1
export JCODE_GATEWAY_PORT="$PORT"
export JCODE_GATEWAY_BIND_ADDR="$BIND_ADDR"
export JCODE_DEBUG_CONTROL="${JCODE_DEBUG_CONTROL:-1}"

echo "Starting jcode gateway for workspace $WORKSPACE_NAME"
echo "Gateway: http://$BIND_ADDR:$PORT/health"
echo "Runtime dir: $JCODE_RUNTIME_DIR"
echo "Default branch: ${CONDUCTOR_DEFAULT_BRANCH:-unknown}"

exec "$BINARY" --provider "$PROVIDER" serve
