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

## Pages: merge, split, rotate, extract, reorder, Bates numbering

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

`pages::bates_number` (Phase 4 quick win) stamps a sequential
`<prefix><zero-padded number><suffix>` onto every page — the legal/discovery
convention — reusing the same stamped-text-object primitive as
`forms::overlay_text`, just looped per page with a computed string:

```rust
use pdfree_core::pages::{self, BatesOptions, StampCorner};

let stamped: Vec<u8> = pages::bates_number(&pdf_bytes, &BatesOptions {
    prefix: "ACME-".to_string(),
    suffix: String::new(),
    start: 1,
    digits: 6,                       // "ACME-000001", "ACME-000002", ...
    corner: StampCorner::BottomRight,
    margin: 24.0,                    // PDF points from the page edge
    font_size: 9.0,
})?;
```

A right-aligned corner (`TopRight`/`BottomRight`) measures each page's
stamped label after placing it (via the text object's own rendered bounds)
and shifts it left by that exact width, rather than estimating text width —
so the stamp's right edge always lands precisely at `margin`.

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

## Boxes: detecting every fillable box on a page (Phase 4 add-on)

Scanned or flattened forms often draw each fillable box as a stroked
rectangle, or a table's cell borders as ruled lines — sometimes a fully
closed rectangle, sometimes just side dividers with no top or bottom rule at
all (very common: a labeled blank gets dividers but the box would be
visually redundant with the row above/below) — instead of a real `AcroForm`
field. `boxes::boxes_on_page` looks at the page's vector graphics (not text,
not form fields) to reconstruct every such box at once, meant to be called
as a page loads so a shell can highlight every fillable area up front:

```rust
use pdfree_core::boxes;

for found in boxes::boxes_on_page(&pdf_bytes, 0)? {
    // found.x/y/width/height are in PDF points — hand them straight to
    // forms::overlay_text as the place to stamp typed text.
}
```

`boxes::box_at_point(&pdf_bytes, page, x, y)` is a convenience wrapper for a
single point-driven lookup (e.g. a manual click that isn't inside any
already-scanned box) — it's `boxes_on_page` filtered down to the smallest
box enclosing that point.

Detection runs three tiers, each skipping anything that duplicates a box a
higher tier already found:

1. **Closed cells** — four rulings (or ruling-clusters, since one visual
   line is often drawn as several abutting strokes) that together bound a
   rectangle. Most reliable: every side is confirmed by an actual line.
2. **Open cells** — a pair of adjacent vertical dividers that both meet the
   same horizontal ruling but have nothing closing the far side (the
   "labeled blank with side dividers, no box" case above).
3. **Lone rectangles** — a single stroked rectangle path not part of any
   grid (standalone checkboxes, signature boxes).

Two implementation details worth knowing if this ever needs revisiting:

- `pdfium-render`'s `PdfPagePathObjectSegments` yields each segment's
  **untransformed, raw** coordinates — real-world PDFs routinely place a
  path via a non-identity object matrix (translation at minimum), so this
  code always applies `path.matrix()` via `.segments().transform(matrix)`
  before reading points. Skipping that step silently reads every line's
  position wrong (confirmed against a real IRS 1040: reading raw segments
  found only small unrelated boxes and missed the entire ruled-line grid).
- When pairing "adjacent" dividers to form a cell, adjacency must be judged
  only among the dividers relevant to the row/column in question (their
  spans must actually reach that row's y-range, or touch that ruling's y).
  Pairing by raw x-order across the *whole page* pairs dividers from
  unrelated rows whenever their x positions happen to interleave — a real
  bug hit while building this, not a hypothetical.
Returns `None` if neither strategy finds an enclosing box — the caller
decides the fallback (a fixed-size overlay, typically).

## Search: in-document text search (Phase 4 quick win — "⌘F")

```rust
use pdfree_core::search;

// Every text run containing "invoice", across the whole document.
let hits = search::find_text(&pdf_bytes, "invoice", false)?; // false = case-insensitive
for hit in &hits {
    // hit.x/y/width/height are the containing run's bounding box, in PDF
    // points — enough to draw a highlight rect and jump to hit.page.
    println!("page {}: {:?} ({}x)", hit.page, hit.text, hit.occurrences);
}
```

Reuses `editor::text_runs` rather than a second text-walking pass. **Known
scope boundary**: a match's bounds are the whole containing run's bounding
box, not a tight box around just the matched substring — same
character-offset-precision boundary `editor::replace_text` already
documents. `SearchMatch::occurrences` reports how many times the query
appears within that run, so a shell can show a count instead of pretending
there was only one hit. An empty query returns an empty list rather than
every run.

## Bookmarks: document outline (Phase 4 quick win)

```rust
use pdfree_core::bookmarks;

// A tree, not a flat list — walk `children` to render a nested outline panel.
let outline = bookmarks::outline(&pdf_bytes)?;
for top_level in &outline {
    println!("{} -> page {:?}", top_level.title, top_level.page);
}
```

Wraps `pdfium-render`'s already-bound `PdfBookmarks`/`PdfBookmark` read API;
`pdfree-core` doesn't add any new `PDFium` capability here, just a plain,
`Send`-able tree a shell can render without touching `PDFium` types. Most
PDFs have no outline at all — that's `Ok(vec![])`, not an error. A bookmark
whose destination `PDFium` can't resolve to a page reports `page: None`
rather than being dropped, so the shell can still show its title. Depth and
total-node traversal are capped (mirroring the `MAX_EDGE_PIXELS` guard in
`renderer.rs`) against a pathological or cyclic bookmark tree.

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
| `InvalidOverlay(..)` | `forms::overlay_text` or `pages::bates_number` given a non-positive/non-finite `font_size` |
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

Phases 0–3 are complete, plus two Phase 4 add-ons: `boxes::box_at_point`
(driven by the macOS app's double-click-to-fill-a-box feature) and four
viewer/pages quick wins from the 2026-07-03 feature research pass —
`search::find_text`, `bookmarks::outline`, and `pages::bates_number` here,
plus `pdfree_ai::confidence::ground_check` (see `docs/ai-design.md`).
`pdfree-core`'s only remaining deliberate gaps are
`signatures::sign_with_certificate` (PKCS#12) and
`convert::to_docx`/`from_docx` — both `PdfError::NotImplemented` pending
open decisions in `CLAUDE.md`, not missing engineering. The rest of Phase 4
(platform shells — these four quick wins aren't wired into any shell's UI
yet) and Phases 5–7 (`pdfree-ai`) are next.
