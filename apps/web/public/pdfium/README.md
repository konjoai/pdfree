# PDFium WASM module (not bundled)

This directory needs two files before the web app can open a real PDF:

- `pdfium.js` — Emscripten-generated glue, exposes a global `PDFiumModule()`
  factory function.
- `pdfium.wasm` — the actual compiled PDFium binary.

They are **not committed to this repo** (same policy as `vendor/pdfium/` for
the native apps — see `docs/pdfium-bundling.md`): third-party binaries are
fetched, never checked in.

## Where to get them

`pdfium-render` (the Rust crate this project binds to) points at
[paulocoutinhox/pdfium-lib releases](https://github.com/paulocoutinhox/pdfium-lib/releases)
as the WASM build source. Download the `wasm` release asset for a recent
version, extract it, and place `pdfium.js` + `pdfium.wasm` directly in this
directory.

## How it's wired up

`src/lib/pdfium.ts` loads both modules and calls `pdfium-render`'s exported
`initialize_pdfium_render(pdfiumModule, rustModule, debug)` — see that
file's comments for the exact sequence pdfium-render's own example
(`examples/index.html` in the `pdfium-render` repo) uses.

**Not yet verified end-to-end** — this repo has no way to fetch or run a
real browser + real `pdfium.wasm` in the environment this integration was
written in. The Rust↔JS call shapes are confirmed correct (they come
directly from `pdfium-render`'s own source and documented example), but the
actual "open a PDF in a browser" path hasn't been click-tested. Treat this
as the next thing to verify once a real `pdfium.wasm` is available.
