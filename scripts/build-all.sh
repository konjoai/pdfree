#!/usr/bin/env bash
# Build and test the whole Rust workspace.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
if [ ! -e vendor/pdfium/libpdfium.so ] && [ ! -e vendor/pdfium/libpdfium.dylib ]; then
  echo "PDFium not found; fetching..."
  scripts/fetch-pdfium.sh
fi
cargo build --workspace --release
cargo test --workspace
