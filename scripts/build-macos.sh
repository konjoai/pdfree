#!/usr/bin/env bash
# Build the universal macOS dylib and generate Swift bindings.
#
# Requires the aarch64/x86_64 Apple targets and uniffi-bindgen (Phase 4).
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
cargo build --release -p pdfree-ffi --target aarch64-apple-darwin
cargo build --release -p pdfree-ffi --target x86_64-apple-darwin
lipo -create -output target/libpdfree.dylib \
  target/aarch64-apple-darwin/release/libpdfree_ffi.dylib \
  target/x86_64-apple-darwin/release/libpdfree_ffi.dylib
echo "Built target/libpdfree.dylib"
echo "Next (Phase 4): uniffi-bindgen generate crates/pdfree-ffi/src/pdfree.udl \\"
echo "  --language swift --out-dir apps/macos/Sources/Bridge/"
