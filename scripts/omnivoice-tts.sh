#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/omnivoice-tts.sh [options] [text...]

Send text to the OmniVoice TTS service used by the Claude Code PAI voice hook.
Text can be passed as arguments or piped on stdin.

Options:
  --url URL          TTS endpoint. Default: $OMNIVOICE_URL or PAI OmniVoice URL
  --instruct TEXT   Voice instruction. Default: $OMNIVOICE_INSTRUCT or PAI voice
  --output PATH     Write WAV to PATH. Default: temporary file
  --timeout SECS    Request timeout. Default: $OMNIVOICE_TIMEOUT or 60
  --no-play         Do not play the WAV with afplay
  -h, --help        Show this help

Environment:
  OMNIVOICE_URL       Override the TTS endpoint
  OMNIVOICE_INSTRUCT  Override the voice instruction
  OMNIVOICE_TIMEOUT   Override timeout seconds
EOF
}

url="${OMNIVOICE_URL:-http://100.113.37.61:8766/tts}"
instruct="${OMNIVOICE_INSTRUCT:-female, british accent}"
timeout="${OMNIVOICE_TIMEOUT:-60}"
output=""
play=1
args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --url)
      [[ $# -ge 2 ]] || { echo "error: --url requires a value" >&2; exit 2; }
      url="$2"
      shift 2
      ;;
    --instruct)
      [[ $# -ge 2 ]] || { echo "error: --instruct requires a value" >&2; exit 2; }
      instruct="$2"
      shift 2
      ;;
    --output)
      [[ $# -ge 2 ]] || { echo "error: --output requires a value" >&2; exit 2; }
      output="$2"
      shift 2
      ;;
    --timeout)
      [[ $# -ge 2 ]] || { echo "error: --timeout requires a value" >&2; exit 2; }
      timeout="$2"
      shift 2
      ;;
    --no-play)
      play=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      args+=("$@")
      break
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      args+=("$1")
      shift
      ;;
  esac
done

if [[ ${#args[@]} -gt 0 ]]; then
  text="${args[*]}"
else
  text="$(cat)"
fi

if [[ -z "${text//[[:space:]]/}" ]]; then
  echo "error: no text provided" >&2
  usage >&2
  exit 2
fi

if ! [[ "$timeout" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
  echo "error: --timeout must be numeric" >&2
  exit 2
fi

if [[ -z "$output" ]]; then
  output="$(mktemp "${TMPDIR:-/tmp}/jcode-omnivoice.XXXXXX.wav")"
else
  mkdir -p "$(dirname "$output")"
fi

json_payload="$(
  TEXT="$text" INSTRUCT="$instruct" python3 - <<'PY'
import json
import os

print(json.dumps({
    "text": os.environ["TEXT"],
    "instruct": os.environ["INSTRUCT"],
}))
PY
)"

tmp_headers="$(mktemp "${TMPDIR:-/tmp}/jcode-omnivoice-headers.XXXXXX")"
status=0
http_code="$(
  curl -sS \
    --connect-timeout 5 \
    --max-time "$timeout" \
    -D "$tmp_headers" \
    -o "$output" \
    -w "%{http_code}" \
    -H "Content-Type: application/json" \
    --data "$json_payload" \
    "$url"
)" || status=$?

if [[ "$status" -ne 0 ]]; then
  rm -f "$tmp_headers"
  rm -f "$output"
  cat >&2 <<EOF
error: OmniVoice request failed (curl exit $status)
endpoint: $url

The JCode wrapper is installed, but the OmniVoice service is not reachable from this Mac.
Start or expose OmniVoice on the host that owns it, or set OMNIVOICE_URL to a reachable /tts endpoint.
EOF
  exit "$status"
fi

if [[ "$http_code" -lt 200 || "$http_code" -ge 300 ]]; then
  rm -f "$tmp_headers"
  rm -f "$output"
  cat >&2 <<EOF
error: OmniVoice returned HTTP $http_code
endpoint: $url
EOF
  exit 1
fi

if [[ ! -s "$output" ]]; then
  rm -f "$tmp_headers"
  echo "error: OmniVoice returned an empty audio file" >&2
  exit 1
fi

rm -f "$tmp_headers"
echo "$output"

if [[ "$play" -eq 1 ]]; then
  if command -v afplay >/dev/null 2>&1; then
    afplay "$output"
  else
    echo "warning: afplay not found; WAV was written but not played" >&2
  fi
fi
