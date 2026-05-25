#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

ROOT="${CONDUCTOR_WORKSPACE_PATH:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$ROOT"

cargo fetch
cargo build --bin jcode
cargo check -p jcode-mobile-core -p jcode-mobile-sim

if [ -d ios ]; then
  if command -v xcodegen >/dev/null 2>&1; then
    (cd ios && xcodegen generate)
  fi

  swift package resolve --package-path ios
fi
