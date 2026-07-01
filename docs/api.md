# pdfree-core API (Phase 0 + 1 + 2 + 3)

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

## Signing: placing a visual signature image

```rust
use pdfree_core::signatures::{self, SignaturePlacement};

let signed: Vec<u8> = signatures::place_signature(
    &pdf_bytes,
    &signature_png,   // drawn, typed-and-rendered, or uploaded — the shell's choice
    SignaturePlacement { page: 0, x: 72.0, y: 450.0, width: 150.0, height: 60.0 },
)?;
```

This is the "basic e-sign" path from `CLAUDE.md`'s v1 spec: stamp an image
onto the page, no cryptography involved. `signatures::sign_with_certificate`
(PKCS#12 digital certificate signing) is deliberately `PdfError::NotImplemented`
— see `CLAUDE.md`'s Phase 2 entry for why.

## Annotations: highlight, underline, strikeout, sticky notes

```rust
use pdfree_core::annotations::{self, Annotation, AnnotationKind, Color};

let annotated: Vec<u8> = annotations::annotate(
    &pdf_bytes,
    &[
        Annotation {
            page: 0, kind: AnnotationKind::Highlight,
            x: 72.0, y: 600.0, width: 300.0, height: 20.0,
            color: None,                    // None = kind's default color
            note: Some("check this".into()),
        },
        Annotation {
            page: 0, kind: AnnotationKind::Note,
            x: 400.0, y: 700.0, width: 24.0, height: 24.0,
            color: None,
            note: Some("reviewer comment".into()),
        },
    ],
)?;

// Read every highlight/underline/strikeout/note back out, e.g. to render an
// annotation list UI or support deleting one.
let found = annotations::list(&annotated)?;
```

**Known gap**: highlight/underline/strikeout write correct, spec-compliant
`/QuadPoints`/`/Rect`/`/C` data — verified by `annotations::list` reading it
straight back — that most real-world viewers (Acrobat, Preview, browsers)
render correctly per the PDF spec's default-appearance-synthesis rule. But
`pdfium-render` 0.8.37 doesn't expose a way to attach an explicit appearance
stream (`/AP`) to those three annotation types, and `PDFium`'s own rendering
doesn't synthesize one either — so they won't show in `pdfree-core`'s own
render preview yet. `AnnotationKind::Note` is unaffected: `PDFium` synthesizes
a sticky-note icon appearance natively.

## Editing: font-preserving in-place text replacement

```rust
use pdfree_core::editor;

// "Detect font of clicked text": enumerate every text run with its font and
// bounds, or hit-test a specific point (both in PDF points).
let runs = editor::text_runs(&pdf_bytes)?;
let hit = editor::text_run_at_point(&pdf_bytes, 0, 80.0, 705.0)?; // Option<TextRun>

// Replace text in place. The matching text object's own content is mutated,
// not recreated, so its font carries over automatically — no font-matching
// heuristic involved.
let edited: Vec<u8> = editor::replace_text(&pdf_bytes, 0, "page one", "chapter one")?;
```

If a text run contains `find` more than once, every occurrence in that run is
replaced together — there's no character-offset-precise "replace just this
one instance" within a run yet. `replace_text` errors
(`PdfError::TextNotFound`) rather than silently no-opping if nothing on the
page matches.

## Pages: merge, split, rotate, extract, reorder

```rust
use pdfree_core::pages::{self, Rotation};

let merged: Vec<u8> = pages::merge(&[doc_a_bytes, doc_b_bytes])?;
let pieces: Vec<Vec<u8>> = pages::split(&pdf_bytes, &[(0, 2), (3, 5)])?; // inclusive 0-based ranges
let rotated: Vec<u8> = pages::rotate(&pdf_bytes, 0, Rotation::Clockwise90)?;

// extract() pulls the given 0-based pages, in exactly the order given, into
// a new document — which is also how reorder() is implemented: give it a
// full permutation of the document's page indices.
let extracted: Vec<u8> = pages::extract(&pdf_bytes, &[2, 0, 1])?;
let reordered: Vec<u8> = pages::reorder(&pdf_bytes, &[1, 0])?; // swap a 2-page doc
```

## Converting: text extraction and image → PDF

```rust
use pdfree_core::convert;

let text: String = convert::to_text(&pdf_bytes)?;           // every page, joined
let pdf: Vec<u8> = convert::from_image(&png_bytes, 96.0)?;  // dpi controls the resulting page size
```

`convert::to_docx`/`convert::from_docx` are `PdfError::NotImplemented` —
`PDFium` has no DOCX support, and faithful PDF↔DOCX conversion needs a
document layout engine this workspace doesn't have yet. See `CLAUDE.md`'s
open questions.

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
| `InvalidAnnotation(..)` | `annotations::annotate` given a non-positive/non-finite width/height |
| `InvalidSignaturePlacement(..)` | `signatures::place_signature` given a non-positive/non-finite width/height |
| `InvalidPageRange(..)` | `pages::merge`/`split`/`extract` given an empty or inverted range/list |
| `InvalidPageOrder(..)` | `pages::reorder` given a list that isn't exactly a permutation of the document's pages |
| `TextNotFound { page, find }` | `editor::replace_text` found no occurrence of `find` on `page` |
| `Io(..)` / `Image(..)` | Filesystem, PNG-encoding, or signature/image-decoding failure |
| `NotImplemented(name)` | `signatures::sign_with_certificate` or `convert::to_docx`/`from_docx` — the two capabilities deliberately deferred pending open decisions |

## PDFium binding

`pdfree_core::pdfium::bind()` loads PDFium, searching `$PDFIUM_DYNAMIC_LIB_PATH`,
then `vendor/pdfium/`, then the system path. See `docs/pdfium-bundling.md`.

**Implementation note**: never call `bind()` twice within one call chain —
two live `PDFium` bindings in the same process hangs (confirmed empirically
while building `pages::reorder`). Every public function binds exactly once;
`pages::extract`/`pages::reorder` share one binding through a private
`extract_with(&Pdfium, ...)` helper rather than one calling the other's
public entry point.

## Status

Phases 0–3 are complete. `pdfree-core`'s only remaining deliberate gaps are
`signatures::sign_with_certificate` (PKCS#12) and `convert::to_docx`/
`from_docx` — both `PdfError::NotImplemented` pending open decisions in
`CLAUDE.md`, not missing engineering. Phase 4 (platform shells) and Phases
5–7 (`pdfree-ai`) are next.
