#!/usr/bin/env bash
# Build the macOS pdfree-ffi dylib (universal if both Apple targets are
# installed, aarch64-only otherwise — Apple Silicon first per CLAUDE.md) and
# generate Swift bindings via UniFFI's library-mode bindgen.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

cargo build --release -p pdfree-ffi --target aarch64-apple-darwin
AARCH64_DYLIB="target/aarch64-apple-darwin/release/libpdfree_ffi.dylib"

if rustc --print target-list | grep -q '^x86_64-apple-darwin$' \
  && [ -d "$(rustc --print sysroot)/lib/rustlib/x86_64-apple-darwin" ]; then
  cargo build --release -p pdfree-ffi --target x86_64-apple-darwin
  lipo -create -output target/libpdfree_ffi.dylib \
    "$AARCH64_DYLIB" \
    target/x86_64-apple-darwin/release/libpdfree_ffi.dylib
  echo "Built universal target/libpdfree_ffi.dylib (aarch64 + x86_64)"
else
  echo "x86_64-apple-darwin target not installed (rustup target add x86_64-apple-darwin" \
    "to add Intel Mac support) — building aarch64-only for now."
  cp "$AARCH64_DYLIB" target/libpdfree_ffi.dylib
  echo "Built aarch64-only target/libpdfree_ffi.dylib"
fi

mkdir -p apps/macos/Sources/Bridge
cargo run --release -p pdfree-ffi --bin uniffi-bindgen -- generate \
  --library target/libpdfree_ffi.dylib \
  --language swift \
  --out-dir apps/macos/Sources/Bridge/
echo "Generated Swift bindings in apps/macos/Sources/Bridge/"
