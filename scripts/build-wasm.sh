#!/usr/bin/env bash
# Build the WASM bindings for the web app.
#
# Requires wasm-pack (cargo install wasm-pack) and PDFium's WASM build wired in
# per docs/pdfium-bundling.md (Phase 4).
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
wasm-pack build crates/pdfree-wasm \
  --target web \
  --out-dir ../../apps/web/src/wasm \
  --release
