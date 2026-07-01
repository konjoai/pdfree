# PDFree

📄 **A truly free PDF editor with AI features.** No watermarks. No limits. No catch.

PDFree is the honest alternative to "free" PDF tools that hit you with
watermarks, task caps, paywalls, or silent uploads. It runs everywhere — macOS,
iOS, Web, Windows, Linux — from one Rust engine, and processes your documents
**locally by default**.

> Status: **Phase 0 complete.** The Rust workspace is scaffolded and the PDFium
> integration is proven end-to-end (open a PDF, render pages to PNG). See
> [`CLAUDE.md`](CLAUDE.md) for the full plan.

## Architecture

One Rust core (`pdfree-core`), many thin platform shells. Rendering goes through
[PDFium](https://pdfium.googlesource.com/pdfium/) — the engine Chrome uses. See
[`docs/architecture.md`](docs/architecture.md).

```
crates/pdfree-core   PDF engine (parse, render, edit, forms, sign, convert)
crates/pdfree-ai     provider-agnostic AI layer (local-first, cloud-opt-in)
crates/pdfree-ffi    UniFFI bridge → Swift/Kotlin
crates/pdfree-wasm   wasm-bindgen bridge → browser
```

## Quickstart

```bash
# 1. Fetch the PDFium runtime library (into vendor/pdfium/, git-ignored)
scripts/fetch-pdfium.sh

# 2. Build and test the whole workspace
scripts/build-all.sh
# or: cargo test --workspace
```

Minimal usage:

```rust
use pdfree_core::{Document, RenderOptions};

let doc = Document::open("contract.pdf")?;
println!("{} pages", doc.page_count());
let png = doc.render_page(0, &RenderOptions::with_dpi(150.0))?;
std::fs::write("page-1.png", png)?;
```

See [`docs/api.md`](docs/api.md) for the full `pdfree-core` API.

## Docs

- [`CLAUDE.md`](CLAUDE.md) — project plan, phases, and decisions
- [`docs/architecture.md`](docs/architecture.md) — engine + shells design
- [`docs/ai-design.md`](docs/ai-design.md) — local-first AI plan
- [`docs/api.md`](docs/api.md) — `pdfree-core` API reference
- [`docs/pdfium-bundling.md`](docs/pdfium-bundling.md) — PDFium per-platform strategy

## License

[BUSL-1.1](LICENSE) — free for personal and commercial non-SaaS use.
