#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$PATH"

ROOT="${CONDUCTOR_WORKSPACE_PATH:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$ROOT"

command -v cargo >/dev/null 2>&1 || {
  echo "cargo is required to set up jcode" >&2
  exit 1
}

mkdir -p .context/conductor/runtime

./scripts/dev_cargo.sh --print-setup
cargo fetch --manifest-path "$ROOT/Cargo.toml"
