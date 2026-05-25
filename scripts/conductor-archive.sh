#!/usr/bin/env bash
set -euo pipefail

ROOT="${CONDUCTOR_WORKSPACE_PATH:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$ROOT"

rm -rf .context/conductor target
