# vendor/pdfium

PDFree renders through [PDFium](https://pdfium.googlesource.com/pdfium/) — the
same engine Chrome uses — loaded **dynamically at runtime**. The native library
is *not* committed to git (it's ~7 MB per platform); it's fetched on demand.

## Get the library

```bash
scripts/fetch-pdfium.sh          # host platform
scripts/fetch-pdfium.sh linux-x64
scripts/fetch-pdfium.sh mac-arm64
```

This downloads a prebuilt binary from
[bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)
(Apache-2.0 / BSD-3-Clause) and drops the platform library here, e.g.
`vendor/pdfium/libpdfium.so`.

## How `pdfree-core` finds it

`pdfree_core::pdfium::bind()` searches, in order:

1. `$PDFIUM_DYNAMIC_LIB_PATH` — explicit path to the library file or its directory.
2. `vendor/pdfium/<platform-lib>` (this directory).
3. The system library search path.

So once this directory is populated, `cargo test` renders for real; without it,
the render tests skip with a notice and still pass.

See `docs/pdfium-bundling.md` for the per-platform shipping strategy.
