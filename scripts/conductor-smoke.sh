#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$PATH"

ROOT="${CONDUCTOR_WORKSPACE_PATH:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$ROOT"

echo "==> validating Conductor settings TOML"
python3 - <<'PY'
import tomllib
from pathlib import Path

path = Path(".conductor/settings.toml")
with path.open("rb") as handle:
    data = tomllib.load(handle)

assert data["$schema"] == "https://conductor.build/schemas/settings.repo.schema.json"
scripts = data["scripts"]
for key in ("setup", "run", "archive"):
    assert scripts[key].startswith("./scripts/conductor-"), key
assert scripts["run_mode"] == "nonconcurrent"
PY

echo "==> checking Conductor script syntax"
bash -n scripts/conductor-setup.sh scripts/conductor-run.sh scripts/conductor-archive.sh

echo "==> checking for unresolved conflict markers"
if git grep -nE '^(<<<<<<<|=======|>>>>>>>)' -- \
    ':!Cargo.lock' \
    ':!graphify-out' \
    ':!.codegraph' \
    ':!.context'; then
  echo "unresolved conflict marker found" >&2
  exit 1
fi

echo "==> running mobile core tests"
cargo test -p jcode-mobile-core

echo "==> running Swift harness tests"
swift test --package-path ios

echo "conductor smoke OK"
