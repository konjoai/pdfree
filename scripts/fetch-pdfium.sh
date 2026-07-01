#!/usr/bin/env bash
# Fetch a prebuilt PDFium shared library into vendor/pdfium/.
#
# Usage:
#   scripts/fetch-pdfium.sh [target]
#
# target is one of the bblanchon/pdfium-binaries names, e.g.
#   linux-x64 (default on Linux), linux-arm64,
#   mac-x64, mac-arm64 (default on Apple Silicon),
#   win-x64, win-arm64
#
# The library is loaded dynamically at runtime by pdfree-core; it is never
# committed to git (see vendor/pdfium/README.md).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$REPO_ROOT/vendor/pdfium"
RELEASE_BASE="https://github.com/bblanchon/pdfium-binaries/releases/latest/download"

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux)  os=linux ;;
    Darwin) os=mac ;;
    MINGW*|MSYS*|CYGWIN*) os=win ;;
    *) echo "unsupported OS: $os" >&2; exit 1 ;;
  esac
  case "$arch" in
    x86_64|amd64) arch=x64 ;;
    arm64|aarch64) arch=arm64 ;;
    *) echo "unsupported arch: $arch" >&2; exit 1 ;;
  esac
  echo "${os}-${arch}"
}

TARGET="${1:-$(detect_target)}"
URL="$RELEASE_BASE/pdfium-${TARGET}.tgz"

echo "Fetching PDFium for '$TARGET'..."
mkdir -p "$DEST"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -sSL --fail -o "$TMP/pdfium.tgz" "$URL"
tar -xzf "$TMP/pdfium.tgz" -C "$TMP"

# The archive contains lib/<platform-library>. Copy it flat into vendor/pdfium.
found=""
for f in "$TMP"/lib/libpdfium.so "$TMP"/lib/libpdfium.dylib "$TMP"/bin/pdfium.dll; do
  if [ -f "$f" ]; then
    cp "$f" "$DEST/"
    found="$(basename "$f")"
  fi
done

if [ -z "$found" ]; then
  echo "error: no PDFium library found in archive" >&2
  exit 1
fi

echo "Installed $found -> $DEST/$found"
