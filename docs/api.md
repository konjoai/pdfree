# pdfree-core API (Phase 0 + 1)

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

## Forms: reading and filling `AcroForm` fields

```rust
use pdfree_core::forms::{self, FieldKind, FillValue};

// Enumerate every interactive field with its kind and current value.
let fields = forms::fields(&pdf_bytes)?;
for f in &fields {
    println!("{:?} {} = {:?}", f.kind, f.name, f.value);
}

// Fill by field name. Every name must exist and must pair with a FillValue
// its kind accepts, or the call errors instead of silently dropping it.
let filled: Vec<u8> = forms::fill(
    &pdf_bytes,
    &[
        ("topmostSubform[0].Page1[0].f1_01[0]".to_string(), FillValue::Text("Wesley".into())),
        ("topmostSubform[0].Page1[0].c1_1[0]".to_string(), FillValue::Checkbox(true)),
    ],
)?;
```

`FillValue` is scoped to what `pdfium-render` 0.8 actually exposes a setter
for: `Text(String)` and `Checkbox(bool)`. Dropdowns, list boxes, radio button
groups, and signature fields are readable via `forms::fields` (their
`FieldKind` and current value come back fine) but not writable through
`forms::fill` — that call returns `PdfError::UnsupportedFieldFill { name, kind }`
rather than silently no-opping. See `CLAUDE.md`'s Phase 1 entry for why.

## Overlaying text on a non-interactive PDF

For a PDF with no `AcroForm` at all — a plain scanned form, a flat template —
stamp text directly onto the page instead:

```rust
use pdfree_core::forms::{self, TextOverlay};

let stamped = forms::overlay_text(
    &pdf_bytes,
    &[TextOverlay {
        page: 0,
        x: 72.0,          // PDF points from the left edge
        y: 700.0,         // PDF points from the bottom edge
        text: "Jane Doe".to_string(),
        font_size: 12.0,
    }],
)?;
```

## Errors

All fallible calls return `pdfree_core::Result<T>` (`Err` is `PdfError`):

| Variant | When |
|---|---|
| `PdfiumUnavailable { searched, .. }` | PDFium library could not be located/loaded; lists every path tried |
| `Pdfium(..)` | PDFium reported an error opening/working with the document |
| `PageOutOfRange { index, count }` | Requested a page that doesn't exist |
| `InvalidRenderTarget(..)` | Non-positive DPI, or a render that would exceed the pixel-size guard |
| `UnknownFormField(name)` | `forms::fill` was asked to fill a name not present in the document |
| `UnsupportedFieldFill { name, kind }` | `forms::fill` was asked to fill a field with a value kind it can't accept (wrong value type, or a dropdown/list-box/radio/signature field) |
| `InvalidOverlay(..)` | `forms::overlay_text` given a non-positive/non-finite `font_size` |
| `Io(..)` / `Image(..)` | Filesystem or PNG-encoding failure |
| `NotImplemented(name)` | A later-phase module (`editor`, `signatures`, …) called before it's built |

## PDFium binding

`pdfree_core::pdfium::bind()` loads PDFium, searching `$PDFIUM_DYNAMIC_LIB_PATH`,
then `vendor/pdfium/`, then the system path. See `docs/pdfium-bundling.md`.

## Later phases

`editor`, `signatures`, `annotations`, `pages`, and `convert` are present as
scaffolds and currently return `PdfError::NotImplemented`. Their signatures are
the intended shape; Phases 2–3 fill in the bodies.
