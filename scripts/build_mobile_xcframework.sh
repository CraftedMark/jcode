#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE="jcode-mobile-ffi"
HEADER_DIR="$ROOT/crates/jcode-mobile-ffi/include"
OUT_DIR="$ROOT/target/mobile-ios"
GENERATED_HEADER_DIR="$OUT_DIR/include"
XCFRAMEWORK="$OUT_DIR/JCodeMobileCore.xcframework"
RUSTUP_CARGO="${RUSTUP_CARGO:-$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo}"
RUSTUP_RUSTC="${RUSTUP_RUSTC:-$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc}"

if [[ ! -x "$RUSTUP_CARGO" || ! -x "$RUSTUP_RUSTC" ]]; then
  echo "rustup stable toolchain not found. Run: rustup toolchain install stable" >&2
  exit 1
fi

rustup target add aarch64-apple-ios aarch64-apple-ios-sim >/dev/null

cd "$ROOT"
env RUSTC="$RUSTUP_RUSTC" "$RUSTUP_CARGO" build -p "$CRATE" --target aarch64-apple-ios --release
env RUSTC="$RUSTUP_RUSTC" "$RUSTUP_CARGO" build -p "$CRATE" --target aarch64-apple-ios-sim --release

rm -rf "$XCFRAMEWORK"
mkdir -p "$OUT_DIR"
rm -rf "$GENERATED_HEADER_DIR"
mkdir -p "$GENERATED_HEADER_DIR"
cp "$HEADER_DIR/jcode_mobile_ffi.h" "$GENERATED_HEADER_DIR/"
cat >"$GENERATED_HEADER_DIR/module.modulemap" <<'MODULEMAP'
module JCodeMobileCore {
  umbrella header "jcode_mobile_ffi.h"
  export *
}
MODULEMAP

xcodebuild -create-xcframework \
  -library "$ROOT/target/aarch64-apple-ios/release/libjcode_mobile_ffi.a" \
  -headers "$GENERATED_HEADER_DIR" \
  -library "$ROOT/target/aarch64-apple-ios-sim/release/libjcode_mobile_ffi.a" \
  -headers "$GENERATED_HEADER_DIR" \
  -output "$XCFRAMEWORK"

echo "$XCFRAMEWORK"
