#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$PATH"

ROOT="${CONDUCTOR_WORKSPACE_PATH:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$ROOT"

runtime_dir="$ROOT/.context/conductor/runtime"
mkdir -p "$runtime_dir"

./scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode

export JCODE_RUNTIME_DIR="$runtime_dir"
export JCODE_SOCKET="$runtime_dir/jcode.sock"
export JCODE_DEBUG_SOCKET="$runtime_dir/jcode-debug.sock"

exec "$ROOT/target/selfdev/jcode" --no-update --debug-socket "$@"
