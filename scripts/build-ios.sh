#!/usr/bin/env bash
# Build pdfree-ffi as an XCFramework for iOS device + simulator, and
# generate Swift bindings via UniFFI — the iOS equivalent of
# scripts/build-macos.sh. iOS needs a static-lib XCFramework rather than a
# loose dylib: unlike a macOS dev build (which can dlopen an unsigned
# absolute-path dylib), iOS restricts loading anything not embedded and
# signed inside the app bundle, so the artifact shape is different even
# though the Rust source and UniFFI interface are identical.
#
# Requires a rustup-managed toolchain with the iOS targets — this repo's
# other native crates build fine with a plain Homebrew/system Rust install,
# but cross-compiling to iOS (like wasm32) needs rustup's target management:
#   brew install rustup
#   rustup toolchain install stable --profile minimal
#   rustup target add aarch64-apple-ios aarch64-apple-ios-sim
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

RUSTUP_TOOLCHAIN_BIN="${RUSTUP_TOOLCHAIN_BIN:-$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin}"
if [ ! -x "$RUSTUP_TOOLCHAIN_BIN/cargo" ]; then
  echo "error: rustup-managed cargo not found at $RUSTUP_TOOLCHAIN_BIN" >&2
  echo "  install with: brew install rustup && rustup toolchain install stable --profile minimal && rustup target add aarch64-apple-ios aarch64-apple-ios-sim" >&2
  exit 1
fi

# PATH order matters: cargo resolves the rustc it invokes for build
# scripts/proc-macros via PATH, not via its own binary location (see the
# equivalent note in build-wasm.sh).
export PATH="$RUSTUP_TOOLCHAIN_BIN:$PATH"
CARGO="$RUSTUP_TOOLCHAIN_BIN/cargo"

"$CARGO" build --release -p pdfree-ffi --target aarch64-apple-ios
"$CARGO" build --release -p pdfree-ffi --target aarch64-apple-ios-sim

DEVICE_LIB="target/aarch64-apple-ios/release/libpdfree_ffi.a"
SIM_LIB="target/aarch64-apple-ios-sim/release/libpdfree_ffi.a"

mkdir -p apps/ios/Sources/Bridge
"$CARGO" run --release -p pdfree-ffi --bin uniffi-bindgen -- generate \
  --library target/aarch64-apple-ios/release/libpdfree_ffi.dylib \
  --language swift \
  --out-dir apps/ios/Sources/Bridge/ 2>/dev/null || {
  # The device target only produces a staticlib (see pdfree-ffi's crate-type
  # list — cdylib is real but iOS toolchains sometimes skip emitting it for
  # a static-only target triple); fall back to bindgen-ing from the
  # already-built macOS dylib, which exports the identical UniFFI interface
  # (same crate, same #[uniffi::export] surface) — only the target platform
  # differs, and bindgen reads interface metadata, not platform-specific code.
  echo "No iOS dylib found for bindgen introspection; using the macOS dylib instead (same interface)."
  "$CARGO" build --release -p pdfree-ffi --target aarch64-apple-darwin
  "$CARGO" run --release -p pdfree-ffi --bin uniffi-bindgen -- generate \
    --library target/aarch64-apple-darwin/release/libpdfree_ffi.dylib \
    --language swift \
    --out-dir apps/ios/Sources/Bridge/
}

# Xcode wants the generated C header in its own directory for the
# XCFramework's Headers/ slice.
mkdir -p target/ios-headers
cp apps/ios/Sources/Bridge/pdfree_ffiFFI.h target/ios-headers/
cp apps/ios/Sources/Bridge/pdfree_ffiFFI.modulemap target/ios-headers/module.modulemap

rm -rf target/PdfreeFFI.xcframework
xcodebuild -create-xcframework \
  -library "$DEVICE_LIB" -headers target/ios-headers \
  -library "$SIM_LIB" -headers target/ios-headers \
  -output target/PdfreeFFI.xcframework

rm -rf apps/ios/Frameworks/PdfreeFFI.xcframework
mkdir -p apps/ios/Frameworks
cp -R target/PdfreeFFI.xcframework apps/ios/Frameworks/

echo "Built apps/ios/Frameworks/PdfreeFFI.xcframework and generated Swift bindings in apps/ios/Sources/Bridge/"
