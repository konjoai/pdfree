# PDFree Architecture

## One engine, many shells

```
┌─────────────────────────────────────────────────────┐
│                   Platform Shells                    │
│  macOS/iOS (SwiftUI)  │  Web (React+WASM)  │ Tauri  │
│         ↕ UniFFI       │   ↕ wasm-bindgen   │   ↕    │
├─────────────────────────────────────────────────────┤
│              Rust Core Engine (pdfree-core)          │
│  parse │ render │ edit │ form-fill │ sign │ convert  │
├─────────────────────────────────────────────────────┤
│           PDFium (Google) via pdfium-render          │
└─────────────────────────────────────────────────────┘
```

All PDF logic lives once, in `pdfree-core`. Every platform is a thin shell over
it. There is no per-platform PDF code — only per-platform UI.

## Crates

| Crate | Role | Status |
|---|---|---|
| `pdfree-core` | The engine. Pure Rust, no platform code. Works on bytes. | Phase 0: `document` + `renderer` live; other modules scaffolded |
| `pdfree-ai` | Provider-agnostic AI/ML layer. Local-first, cloud-opt-in. | Scaffolded (Phases 5–7) |
| `pdfree-ffi` | UniFFI wrapper → Swift/Kotlin. Proc-macro mode (`#[uniffi::export]`, no `.udl`) — the interface is derived from `src/lib.rs` and covers the full Phase 0–3 `pdfree-core` surface. | Phase 4: codegen wired, macOS app scaffolded (`apps/macos/`) |
| `pdfree-wasm` | wasm-bindgen wrapper → JS/React. | Thin wrapper live; wasm32 build in Phase 4 |

## Key design decisions

### Bytes, not paths
`pdfree-core` never assumes a filesystem. `Document` owns the PDF bytes;
operations bind PDFium and work from memory. This is what lets the *same* code
render on macOS and in a browser tab. File-path constructors are thin conveniences
layered on top.

### PDFium loaded at runtime
`pdfium-render` binds PDFium dynamically (via `libloading`), so:
- The Rust crates compile with no native dependency present.
- Each platform ships the PDFium binary its own way (see `docs/pdfium-bundling.md`).
- Discovery is explicit and debuggable: `$PDFIUM_DYNAMIC_LIB_PATH` → `vendor/pdfium/`
  → system. Failures list every path tried.

### Errors are typed and honest
`PdfError` distinguishes "library missing" from "bad page" from "not implemented".
Later-phase modules return `NotImplemented(name)` rather than silently no-op'ing.

### No watermarks, no limits, no telemetry
There is deliberately no usage counter, no feature gate, no network call in the
core. That's the product.

## Data flow: render a page

```
UI (Swift/JS)
   → pdfree-ffi / pdfree-wasm wrapper
      → Document::render_page(index, RenderOptions{dpi})
         → pdfium::bind()            (locate + load libpdfium)
         → load_pdf_from_byte_slice  (parse in memory)
         → page.render_with_config   (rasterize at dpi/72 scale)
         → encode PNG                (image crate)
      ← Vec<u8> PNG bytes
   ← Uint8Array / Data
```

## Build & test

```bash
scripts/fetch-pdfium.sh     # get the runtime library into vendor/pdfium/
scripts/build-all.sh        # cargo build + test the whole workspace
```

Integration tests (`crates/pdfree-core/tests/render.rs`) open a real 2-page PDF
fixture and assert exact rendered pixel dimensions. Without PDFium bundled they
skip with a notice, so a bare checkout still builds green.
