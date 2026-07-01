# PDFium Binary Bundling Strategy

PDFree renders through [PDFium](https://pdfium.googlesource.com/pdfium/), the
engine Chrome uses. `pdfree-core` links it **dynamically at runtime** via
`pdfium-render` (which uses `libloading`), so there is no build-time native
dependency — the Rust crates compile without PDFium present, and the library is
supplied at run time.

## Source of binaries

We use the community-maintained
[**bblanchon/pdfium-binaries**](https://github.com/bblanchon/pdfium-binaries)
prebuilt releases rather than building PDFium from source. Reasons:

- Building PDFium from source pulls Google's `depot_tools` + gn/ninja — a
  multi-GB, slow, fragile toolchain we don't want in CI.
- The prebuilt binaries track upstream PDFium closely, are stripped, and cover
  every target we ship.
- License is compatible (Apache-2.0 / BSD-3-Clause).

`scripts/fetch-pdfium.sh` downloads the right archive and drops the platform
library into `vendor/pdfium/`. The binary is **git-ignored** — never committed.

## Runtime discovery (`pdfree_core::pdfium::bind`)

Searched in order, first success wins:

1. `$PDFIUM_DYNAMIC_LIB_PATH` — explicit path to the library file or its directory.
2. `vendor/pdfium/<platform-lib>` — the fetched, bundled copy.
3. The system library search path.

## Per-platform shipping

| Target | Library file | How it ships in the app |
|---|---|---|
| **macOS** (arm64 + x64) | `libpdfium.dylib` | Bundled inside the `.app` at `Contents/Frameworks/`; `@rpath` resolves it. Ship a universal (lipo'd) dylib. |
| **iOS** | `PDFium.xcframework` / static | Static-link or embed an `xcframework`. App Store forbids dynamic loading of arbitrary dylibs, so prefer static linking here (`pdfium-render` `static` feature) or an xcframework. |
| **Windows** | `pdfium.dll` | Placed next to the `.exe` (or in the Tauri resource dir). |
| **Linux** | `libpdfium.so` | Shipped in the AppImage / `.deb` alongside the binary; `LD_LIBRARY_PATH` / `rpath` or `$PDFIUM_DYNAMIC_LIB_PATH` points at it. |
| **Web (WASM)** | PDFium WASM module | Different path entirely: `pdfium-render`'s `wasm` feature expects a PDFium WASM build loaded from JS. `bblanchon` publishes a `pdfium-wasm` bundle. Wired in Phase 4. |

## Notes for later phases

- **iOS static linking** is the one meaningful deviation from "load at runtime";
  plan for the `static` feature and an `xcframework` build in Phase 4.
- **WASM** needs the PDFium WASM artifact plus glue; `apps/web` loads it before
  instantiating the `pdfree-wasm` module. Also Phase 4.
- Pin a specific PDFium release tag in `fetch-pdfium.sh` before shipping so
  builds are reproducible (currently tracks `latest`).
