# pdfree-core API (Phase 0)

The engine works on **bytes**, not file paths, so the identical code path runs on
native platforms and in the browser (where there is no filesystem). Convenience
constructors read files where a filesystem exists.

## Opening a document

```rust
use pdfree_core::{Document, RenderOptions};

// From a file...
let doc = Document::open("contract.pdf")?;
// ...or from bytes (browser, network, DB blob):
let doc = Document::from_bytes(bytes, None)?;          // None = no password
let doc = Document::open_with_password("locked.pdf", "hunter2")?;
```

## Inspecting

```rust
doc.page_count();          // u16
let m = doc.metadata();    // &Metadata { title, author, subject, creator, producer, page_count }
m.title.as_deref();        // Option<&str>
```

## Rendering a page

```rust
// 0-based page index. DPI: 72 = 1x, 150 = screen default, 300 = print.
let png: Vec<u8> = doc.render_page(0, &RenderOptions::with_dpi(150.0))?;
std::fs::write("page-1.png", png)?;
```

Free-function equivalents also exist:

```rust
let doc = pdfree_core::open_document("contract.pdf")?;
let png = pdfree_core::render_page(&doc, 0, &RenderOptions::default())?; // default 150 DPI
```

## Errors

All fallible calls return `pdfree_core::Result<T>` (`Err` is `PdfError`):

| Variant | When |
|---|---|
| `PdfiumUnavailable { searched, .. }` | PDFium library could not be located/loaded; lists every path tried |
| `Pdfium(..)` | PDFium reported an error opening/working with the document |
| `PageOutOfRange { index, count }` | Requested a page that doesn't exist |
| `InvalidRenderTarget(..)` | Non-positive DPI, or a render that would exceed the pixel-size guard |
| `Io(..)` / `Image(..)` | Filesystem or PNG-encoding failure |
| `NotImplemented(name)` | A later-phase module (`forms`, `signatures`, …) called before it's built |

## PDFium binding

`pdfree_core::pdfium::bind()` loads PDFium, searching `$PDFIUM_DYNAMIC_LIB_PATH`,
then `vendor/pdfium/`, then the system path. See `docs/pdfium-bundling.md`.

## Later phases

`forms`, `editor`, `signatures`, `annotations`, `pages`, and `convert` are present
as scaffolds and currently return `PdfError::NotImplemented`. Their signatures are
the intended shape; Phases 1–3 fill in the bodies.
