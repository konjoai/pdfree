# CLAUDE.md — PDFree Project Handoff

> Konjo AI · Lead Architect: Wes (konjoai)
> Status: **Phase 0 complete** — Rust workspace scaffolded, PDFium integration proven.
> Project name: **PDFree** (PDF + Free)

---


## Project Overview

**PDFree** — a truly free, no-watermark, no-limit PDF application that runs everywhere:

- macOS desktop (native, Apple Silicon first)
- iOS
- Web (browser-based)
- Cross-platform desktop: Windows + Linux

**Why:** Every existing "free" PDF tool is fake-free — watermarks, task limits, paywalls, or privacy risks. PDFree is the honest alternative: no watermarks, no caps, no paywall, local-first privacy.

**Fits the Konjo AI portfolio** alongside Squish (inference), Squash (compliance), Vectro (embeddings), Kohaku (episodic memory), and lopi (orchestration).

**Positioning tagline options:**
- "PDFree — actually free."
- "The last PDF tool you'll ever download."
- "No watermarks. No limits. No catch."

---

## Architecture Decision Record

### Core Strategy: Rust Engine + Platform Shells

```
┌─────────────────────────────────────────────────────┐
│                   Platform Shells                    │
│  macOS/iOS (SwiftUI)  │  Web (React+WASM)  │ Tauri  │
│         ↕ FFI/UniFFI  │     ↕ wasm-bindgen │   ↕    │
├─────────────────────────────────────────────────────┤
│              Rust Core Engine (pdfree-core)          │
│  PDF parse │ render │ edit │ form fill │ sign │ OCR  │
├─────────────────────────────────────────────────────┤
│              PDFium (Google) via pdfium-render        │
│         The same engine Chrome uses. Battle-tested.  │
└─────────────────────────────────────────────────────┘
```

See `docs/architecture.md` for the full detail and `docs/api.md` for the current
`pdfree-core` API surface.

### Why Rust Core
- Single source of truth for all PDF logic
- Compiles to native (macOS/Linux/Windows) AND WASM (web)
- No licensing surprises; full control
- Fits existing Konjo AI Rust expertise (lopi, Vectro)

### Why PDFium
- Google's open-source PDF engine (Apache 2.0)
- Used in Chrome/Chromium — most battle-tested free PDF renderer on Earth
- `pdfium-render` crate provides safe Rust bindings; loaded dynamically at runtime
- Handles the hardest parts: complex forms, XFA, fonts, rendering fidelity

### Why SwiftUI for macOS/iOS
- Native look and feel; macOS-first quality
- UniFFI bridges Rust → Swift cleanly (interface frozen in `crates/pdfree-ffi/src/pdfree.udl`)
- Leverage PDFKit for rendering preview layer on Apple platforms

### Why Tauri for Windows/Linux
- Rust backend (same core) + web frontend (React)
- Far lighter than Electron (~3MB vs ~150MB)
- Ships the web frontend as the UI; no duplicate UI code

---

## Monorepo Structure

```
pdfree/                          ← repo root
├── CLAUDE.md                    ← this file
├── Cargo.toml                   ← workspace root
├── crates/
│   ├── pdfree-core/             ← PDF engine (Rust)   [Phase 0: document + renderer live]
│   ├── pdfree-ai/               ← AI/ML layer (Rust)  [scaffolded, Phases 5–7]
│   ├── pdfree-ffi/              ← UniFFI bindings → Swift/Kotlin
│   └── pdfree-wasm/             ← wasm-bindgen → browser
├── apps/                        ← platform shells (Phase 4): macos, ios, web, desktop
├── scripts/                     ← fetch-pdfium.sh, build-all.sh, build-wasm.sh, build-macos.sh
├── vendor/pdfium/               ← runtime PDFium library (fetched, gitignored)
└── docs/                        ← architecture.md, ai-design.md, api.md, pdfium-bundling.md
```

`pdfree-core` module map (module → phase it lands in):
`document.rs` + `renderer.rs` (Phase 0 ✅), `forms.rs` (Phase 1 ✅ — text/checkbox
fill + text overlay; dropdown/list-box *writing* deferred, see Phase 1 below),
`signatures.rs` + `annotations.rs` (Phase 2 ✅ — visual signature placement,
markup/note annotations; PKCS#12 crypto signing deferred, see Phase 2 below),
`editor.rs` + `pages.rs` + `convert.rs` (Phase 3 ✅ — font-preserving text
replace, merge/split/rotate/extract/reorder, text extraction + image→PDF;
PDF↔DOCX deferred, see Phase 3 below), `boxes.rs` (Phase 4 add-on ✅ —
`boxes_on_page` reconstructs every fillable box on a page from vector
graphics alone (closed cells, "open" cells with dividers but no top/bottom
rule, and lone rectangles — see `docs/api.md` for the tier breakdown);
`box_at_point` is a point-driven convenience wrapper over it. Powers the
macOS app's scan-on-load box highlighting; not in the original phase plan,
added alongside that UI work), `fields.rs` (label-aware fillable-field
detection — the accurate list a shell should highlight: every `AcroForm`
widget plus every *labeled* detected box, in one document parse. Replaces
raw `boxes_on_page` as the macOS overlay source so decorative/unlabeled
rectangles stop being highlighted and real `AcroForm` fields with no drawn
box stop being missed — see `docs/api.md`'s "Fields" section and the
2026-07-12 note in Phase 4 below), `search.rs` + `bookmarks.rs` (Phase 4
quick wins ✅ engine-side — `find_text` in-document search over
`editor::text_runs`, `outline` wrapping `pdfium-render`'s already-bound
bookmark tree; see Phase 4 below — neither is wired into a shell's UI yet).
Phase 4 is otherwise platform shells, Phases 5–7 add `pdfree-ai` (except
`confidence.rs`, a quick win pulled forward and already implemented — see
`docs/ai-design.md`).

---

## v1 Feature Spec

### Must-Have (v1.0)

| Feature | Engine Layer | Notes |
|---|---|---|
| Open + render PDF | `pdfree-core/renderer.rs` | Via PDFium; smooth scroll |
| Fill AcroForms | `pdfree-core/forms.rs` | Text fields, checkboxes, dropdowns |
| Fill non-interactive PDFs | `pdfree-core/forms.rs` | Overlay text boxes |
| Sign documents | `pdfree-core/signatures.rs` | Draw, type, or image upload |
| Edit existing text | `pdfree-core/editor.rs` | Font detection + matching |
| Annotate (highlight, underline, notes) | `pdfree-core/annotations.rs` | Standard PDF annotations |
| Merge PDFs | `pdfree-core/pages.rs` | N files → 1 |
| Split PDFs | `pdfree-core/pages.rs` | By page range or bookmarks |
| Convert to/from formats | `pdfree-core/convert.rs` | PDF↔Word/image/text |
| Save / export | `pdfree-core/document.rs` | Preserve original layout |

### Out of scope for v1
- Real-time collaboration
- Cloud storage sync
- Legally binding e-signature workflow (DocuSign-style audit trail)

---

## Core UX Principles (Wes, 2026-07-01)

These come directly from years of frustration with existing "free" PDF tools.
Treat them as hard requirements, not nice-to-haves:

1. **Default view = whole page visible.** On document load, and on every
   resize, default zoom must fit the entire page height+width in the viewport
   — regardless of screen size or window size. Never open zoomed in. (Bug
   found in current macOS app build — fix before anything else in Phase 4.)
2. **Zero manual text-box placement for forms.** On document load, scan the
   *entire* document for every fillable field (AcroForm fields **and**
   vector-drawn boxes/cells via `boxes.rs`) and pre-render an input affordance
   for all of them immediately. The user should never need to double-click to
   manually place a box — that's a fallback only, not the primary flow. If a
   field looks fillable to a human, the software must have already found it.
3. **Signature/initials fields are special-cased.** Any detected field whose
   label matches signature/initials patterns ("Signature", "Sign here",
   "Initials", "Initial here", etc.) should not open as a text input — it
   should trigger the sign flow directly (draw / type / upload image / reuse
   a saved signature). This is a top-tier annoyance with paid competitors —
   PDFree gives it away free, unlimited signatures, no per-document cap.
4. **WYSIWYG text sizing, always.** Whatever font size renders on screen for
   a filled/overlaid text field must be exactly what's in the exported PDF —
   no silent shrink-on-export, no clipping. Decision: allow auto-shrink-to-fit
   *at edit time* (rendered live as the user types), but the box must never
   silently resize or clip text only at export — what you see while editing
   is what you get in the file, full stop.
5. **No File-menu-only actions for core operations.** Merge, split,
   add/remove page, and import should be reachable via a persistent
   in-canvas affordance (e.g., a "+" button) — not buried in a menu bar. The
   user should never need an extra click/context-switch for something this
   common.
6. **Minimal toolbar.** Acrobat-style button sprawl is an anti-goal. Default
   toolbar should cover the actual common path — open, fill, sign, add/remove
   page, export — and nothing else competes for visual attention on first
   load. Anything else (annotate styles, advanced page ops) can live one
   level deep.
7. **Reliability over polish.** UI can be rough around the edges; core
   functionality (open, fill, sign, merge/split, export) must be rock solid —
   every time, no exceptions. Requires real end-to-end tests: drag-and-drop
   import, file-picker import, and the full fill→sign→export path, not just
   unit tests on `pdfree-core`.
8. **No paywalls on core features.** Signing (unlimited), filling, merge/
   split, annotate, and export must always be free and fully functional. Any
   future paid tier must be additive (see Potential Paid Features below), never
   a cap on the core path.

#### UX research findings (not Wes's own words — flagged separately from the
numbered list above so provenance stays honest; these are patterns found
while researching "what makes the best possible PDF-tool experience"
2026-07-03, not something dictated in the 2026-07-01 review):

- **Icon-only controls are a real, well-documented failure mode** — Adobe's
  own users cite Acrobat's redesigned toolbar as "counterintuitive" and
  "indecipherable" specifically because icons lack text labels, and users
  report hunting for tools that used to have obvious positions. Applies
  directly to Principle #6's "minimal toolbar": every icon-only affordance
  in the Inspector/toolbar across all three shells should carry a persistent
  tooltip/accessibility label, not rely on the icon alone being self-evident
  — cheap to do, and the linked complaints suggest it's an easy place for a
  minimal toolbar to become a *confusing* one if skipped.
- **Fixed-width, non-resizable panels are a specific, named complaint**
  against the new Acrobat UI ("can't be resized to fit specific screen
  sizes"). Worth keeping in mind for the Inspector/Pages-sidebar panels on
  every shell as they mature past their current fixed-width scaffolding.
- **Page-jump/page-number should be prominent, not buried** — a specific
  complaint was not being able to find the page-number control to jump
  pages quickly because it was tucked in a bottom corner. The web app's
  existing page-count/prev/next pill (`App.tsx`) already does reasonably
  well here (bottom-center, always visible) — worth carrying the same
  visibility standard forward to macOS/iOS rather than letting it drift
  toward Acrobat's mistake.

### Potential Paid Features (post-v1, not blocking core roadmap)

Surfaced during this discussion — do not build until v1 core is solid:

- **Legal-grade e-signature** (ESIGN/eIDAS certified audit trail, notarized
  chain of custody) — v1 ships a *lightweight* local audit record only
  (timestamp, signer name, device info where available, baked into
  PDF metadata/incremental update); full certified audit trail is deferred,
  tracked under the existing "Signature legal validity" open question below.
- **Redact-and-overwrite existing field values** (e.g., white-out + replace
  a filled field cleanly) — explicitly deferred, not needed for v1.
- Possible general direction: AI features (Phase 5-7) as the one paid tier,
  since those carry real per-call cost — see existing open question below.

---

## AI / ML Integrations

**Design principle: local-first, cloud-optional.** See `docs/ai-design.md` for the
full tiered plan (Q&A/RAG, smart form fill, OCR cleanup, summary; then redaction,
contract analysis, table extraction, semantic search, classification; then v2+
translation, layout-aware editing, agentic workflows). AI features must honor the
privacy pitch:

- Default to on-device models so documents never leave the machine.
- Offer cloud providers (Claude, GPT, Gemini) as an explicit opt-in.
- Every AI action states where processing happens. No silent uploads.

The provider abstraction lives in `crates/pdfree-ai/src/provider.rs`.

---

## License

**BUSL-1.1** (matching Squish) — free for personal/commercial non-SaaS use;
protects against competitors wrapping it as a SaaS.

---

## Phase Plan

### Phase 0 — Foundation ✅ DONE
- [x] Init Cargo workspace: `pdfree-core`, `pdfree-ai`, `pdfree-ffi`, `pdfree-wasm`
- [x] Integrate `pdfium-render`: open a PDF, render page 1 to PNG
- [x] Expose `open_document()` and `render_page()` in pdfree-core API
- [x] Write unit/integration tests for open + render
- [x] Confirm PDFium binary bundling strategy per platform (`docs/pdfium-bundling.md`)

### Phase 1 — Core Read + Fill ✅ DONE (documented gaps below)
- [x] `document.rs`: open, save, metadata (open/save/metadata done in Phase 0)
- [x] `renderer.rs`: render pages to images at arbitrary DPI ✅ (done in Phase 0)
- [x] `forms.rs`: detect AcroForm fields (`forms::fields`), fill text/checkbox
      (`forms::fill`). **Dropdown/list-box writing is not supported** —
      `pdfium-render` 0.8.37 exposes no public setter for selecting a combo/
      list box option (only text-field and checkbox setters exist). Calling
      `fill()` on a dropdown/list-box/radio/signature field returns
      `PdfError::UnsupportedFieldFill` rather than silently no-opping.
      Revisit if a future `pdfium-render` release adds the setter, or drop to
      lower-level `AcroForm` dictionary writes if that's ever worth the risk.
- [x] `forms.rs`: overlay text boxes on non-interactive PDFs (`forms::overlay_text`)
- [x] Tests: fill a real IRS Form 1040 PDF (`tests/fixtures/irs_f1040.pdf`,
      fetched from irs.gov), assert field values persist after save/reload
- [x] `forms.rs`: `FormField` now carries `page: u16` plus flat `x`/`y`/
      `width`/`height: f32` fields (matching the `TextRun`/`AnnotationInfo`
      convention already used elsewhere in the crate, not a `rect` tuple),
      populated from each widget annotation's own bounds. **Unblocks Phase 4**:
      a shell can now scan the whole document once and get page + bounding
      rect for every field to pre-render an input affordance, no manual
      double-click placement needed. Mirrored into `pdfree-ffi`'s `FormField`
      record so the FFI surface doesn't drift behind core. Verified against
      the real, multi-page IRS 1040 fixture (`tests/forms.rs`) — every
      discovered field reports a plausible page index and non-empty rect.
- [x] `forms.rs`: `fill()`'s deterministic font-size-fit-once logic was
      **investigated and confirmed not achievable** with the current binding,
      rather than left as an open TODO. Read `pdfium-render` 0.8.37's own
      source: setting a text field's rendered font size means writing its
      widget's `/DA` string, and the only calls that can touch an annotation's
      dictionary keys (`FPDFAnnot_SetStringValue_str` and friends) live behind
      a `pub(crate)`-only trait the crate deliberately keeps unexposed — there
      is no annotation handle or dictionary-key setter reachable from outside
      `pdfium-render` for a `PdfFormField`. So sizing stays entirely
      `PDFium`'s own form-render behavior at export time (the likely source of
      the "text resizes/gets cut off on export" symptom), and this is
      documented as a real gap in `forms.rs`'s module doc and `docs/api.md`
      rather than silently left unaddressed. Revisit only if a future
      `pdfium-render` release exposes a public setter, or forking/vendoring
      the binding is ever worth the maintenance cost.
- [x] `forms.rs` / `pdfree-ffi`: `FormField` also carries a
      `signature_kind` (`None`/`Signature`/`Initials`) classified once in core
      via `SignatureFieldKind::classify`, so every shell routes signature/
      initials fields to the sign flow identically (Core UX Principle #3).

### Phase 2 — Sign + Annotate ✅ CORE DONE (crypto signing deferred)
- [x] `signatures.rs`: place signature image at coordinates (`place_signature`)
- [ ] `signatures.rs`: digital certificate signing (PKCS#12). Deliberately
      **not implemented** — `PDFium` has no cryptography; this needs a real
      crypto/PKI stack choice plus incremental-update byte-range signing, and
      depends on the "v1 = basic e-sign only, or pursue ESIGN/eIDAS from day
      one?" open question below. `sign_with_certificate` stays
      `PdfError::NotImplemented` until that's decided.
- [x] `annotations.rs`: highlight, underline, strikethrough, sticky notes
      (`annotate` to add, `list` to read back). **Known gap**: highlight/
      underline/strikeout write correct, spec-compliant data (`/QuadPoints`,
      `/Rect`, `/C` — verified via `list`) that most real-world viewers
      render correctly per the PDF spec's default-appearance-synthesis rule,
      but `pdfium-render` 0.8.37 doesn't expose a way to attach an explicit
      appearance stream to those three annotation types, and `PDFium`'s own
      rendering doesn't synthesize one — so they won't show in `pdfree-core`'s
      own render preview yet. Sticky notes are unaffected (PDFium synthesizes
      their icon appearance natively; confirmed by rendering).
- [ ] `signatures.rs`: basic audit metadata capture — `signer_name`,
      `signer_email`, `ip_address`, `signed_at`. This is a new, separate,
      **non-deferred** item: free-tier core, not blocked on the deferred
      PKCS#12 crypto-signing work above. Embed as PDF metadata and/or an
      appended signature-certificate page.
- [ ] `forms.rs` / `signatures.rs`: "Initials" vs. "Signature" field
      classification. PDF has no distinct Initials field type — PDFium only
      gives us `FieldKind::Signature`. Needs a name-based heuristic (regex
      over field name/tooltip) layered on top so the shell can route
      "Initials" boxes to a lighter-weight signer UI than full signatures.
- [ ] Web: `SignaturePad.tsx` using canvas → PNG → core

### Phase 3 — Edit + Merge/Split + Convert ✅ CORE DONE (DOCX deferred)
- [x] `editor.rs`: detect font of clicked text (`text_runs`, `text_run_at_point`),
      replace in-place (`replace_text`). Font is preserved by construction —
      the matching text object's own content is mutated, not recreated, so
      there's no font-matching heuristic to get wrong. **Known scope
      boundary**: a run containing the search text more than once replaces
      every occurrence together; there's no character-offset-precise
      "replace just this one instance" within a run yet.
- [x] `pages.rs`: merge N PDFs (`merge`); split by range (`split`);
      rotate (`rotate`); extract (`extract`, also powers `reorder` — a
      single `FPDF_ImportPages` call with an explicit page order handles
      both "pull these pages out" and "put them in this order").
      **Implementation note for future edits**: never call
      `crate::pdfium::bind()` twice within one call chain — confirmed
      empirically that two live `PDFium` bindings in the same process hangs.
      `pages::extract`/`reorder` share one binding via a private
      `extract_with(&Pdfium, ...)` helper for exactly this reason.
      **Newly discovered gap (2026-07-02, found while verifying the macOS
      redesign's drag-to-reorder)**: `FPDF_ImportPages` builds a fresh
      document container, which does not carry over document-level metadata
      (Title/Author/etc.) from the source — reordering or extracting pages
      silently drops the document's Title. Confirmed live: the macOS
      titlebar (`store.title`, which prefers metadata Title over the
      filename) visibly changed from "2025 Form 1040" to the bare filename
      immediately after a drag-reorder. Not fixed here — same shape of
      problem as the DOCX/dropdown-fill gaps above, needs a scoped
      decision (re-apply the source metadata after import, exposed as an
      explicit `pages::` option) rather than a quick patch.
- [x] `convert.rs`: `to_text` (all-pages plain text) and `from_image`
      (image → single-page PDF, sized to the image) are fully implemented.
      **PDF ↔ DOCX is deliberately not implemented** — `to_docx`/`from_docx`
      stay `PdfError::NotImplemented`. This isn't a small API gap like the
      Phase 1/2 ones: DOCX conversion needs a document *layout* engine
      (paragraphs, styles, reflow) that neither `PDFium` nor anything else in
      this workspace provides. Picking one (a layout-reconstruction crate,
      shelling out to a conversion service, or a much lower-fidelity
      text-only export) is a real dependency decision — added to the open
      questions below rather than guessed at.

### Phase 4 — Platform Shells
- [x] `renderer.rs`: add a `fit_to_page()` helper (pure math: page size in
      points + viewport size in pixels → `RenderOptions`/DPI) so every shell
      computes the default fit-to-page zoom identically instead of each
      platform back-computing it separately. Paired with a new
      `Document::page_size`/`renderer::page_size_points` so a shell can read
      a page's PDF-point dimensions without rendering it first (avoids a
      render-to-discover-size chicken/egg). Exposed over FFI as
      `PdfDocument.pageSize(index:)` and the free function
      `fitToPageDpi(pageWidthPts:pageHeightPts:viewportWidthPx:viewportHeightPx:)`.
- [x] Wire UniFFI codegen for `pdfree-ffi` — migrated to proc-macro mode
      (`#[uniffi::export]` on `src/lib.rs` directly; the old hand-maintained
      `pdfree.udl` is deleted so the interface can't drift from the Rust code).
      Covers the full Phase 0–3 surface (forms, signatures, annotations,
      editor, pages, convert), not just Phase 0. `scripts/build-macos.sh`
      builds the dylib and runs `uniffi-bindgen` (a local bin target, no
      global install) to emit Swift into `apps/macos/Sources/Bridge/`.
      Currently aarch64-only (Apple Silicon first, per this doc); the script
      auto-detects and adds x86_64 if that target is ever installed.
- [x] macOS SwiftUI app wrapping pdfree-ffi via UniFFI — `apps/macos/`
      (`xcodegen`-generated project from `project.yml`; run `xcodegen
      generate` after pulling). Full v1 feature set wired into the UI, not
      just open/render: form fill (text/checkbox fields via a side panel),
      sign (draw-signature pad, tap to place), annotate (drag for highlight/
      underline/strikeout, tap for sticky notes), edit text (tap a run,
      replace in place), overlay text on non-interactive PDFs, pages sidebar
      (thumbnails, rotate/delete/reorder-by-drag, merge another PDF, insert
      an image as a page, split into ranges), and text extraction. Plus
      scan-on-load box filling: every time the current page changes,
      `PDFDocumentStore` calls `boxesOnPage` and caches the result; every
      detected box is drawn as a highlighted outline on the canvas up front
      (see `PageCanvasView`'s `detectedBoxes` overlay) — clicking directly on
      one in Select mode opens an inline, in-place-editable `TextField`
      exactly over it, no double-click needed. Double-click-anywhere remains
      as the manual fallback for spots the scan didn't pick up as a box
      (falls back to a fixed 140×18pt box centered on the click). Committing
      calls `overlay_text` at the box's position — see
      `PDFDocumentStore.boxContaining`/`detectedBoxes` and
      `ContentView.handleTap`/`handleDoubleTap`. All canvas tools work in PDF
      points (72/inch, bottom-left origin) computed from the rendered PNG's
      pixel size — see `PageCanvasView.swift`. The inline editor's text is
      explicit `.foregroundColor(.black)` — without it, the field's text
      color came out unreadable (white-on-white against the yellow
      highlight) in testing. Gotchas worth knowing: (1) the FFI's RGB color
      record is named `AnnotationColor`, not `Color` — a bare `Color` record
      silently shadows `SwiftUI.Color` once both are in the same Swift
      module, which broke every default SwiftUI color reference until
      renamed on the Rust side (`crates/pdfree-ffi/src/lib.rs`) and
      regenerated; (2) the app links against
      `target/aarch64-apple-darwin/release/libpdfree_ffi.dylib` (the
      per-target dir `scripts/build-macos.sh` actually rebuilds), not
      `target/release/` (a separate, easily-stale artifact from a plain
      `cargo build --release` with no `--target` flag) — linking the wrong
      one silently ships a stale dylib missing whatever FFI symbols were
      added most recently; (3) see `docs/api.md`'s `boxes` section for two
      real detection bugs hit and fixed against the actual IRS 1040 fixture
      (an untransformed-path-matrix bug that put every ruled line in the
      wrong place, and a cross-row divider-pairing bug) — both are exactly
      the kind of thing that looks fine against a synthetic single-rect test
      fixture and silently breaks on a real multi-row form; verify any
      future change here against a real form, not just synthetic geometry.
      Verified against real PDFs (IRS Form 1040 for render and box
      detection — confirmed by rendering the detected boxes back onto the
      page image and reviewing it; `form_sample.pdf` for the full mutation
      surface — forms/signatures/annotations/editor/pages/convert all
      confirmed working through the compiled dylib, plus a dedicated
      `tests/boxes.rs` covering closed-cell, open-cell, and point-lookup
      cases) — but end-to-end click-driven UI testing (actually dragging a
      highlight, drawing a signature, clicking a highlighted box in the
      running app) wasn't done from this sandbox; only the underlying FFI
      calls each UI action makes were verified directly. Dev-only linking:
      the app links
      `libpdfree_ffi.dylib` by absolute rpath; it isn't embedded into the
      `.app` bundle yet, so this isn't distributable as-is — that packaging
      step, and PKCS#12 crypto signing (still `NotImplemented`, see Phase 2),
      are still open. Deployment target is macOS 14.0, not 13.0 as originally
      set up — bumped after Xcode 26's toolchain threw a `SwiftUICore`
      direct-linking error at 13.0 (a known class of Xcode/SDK version-skew
      issue, not a code problem).

      **2026-07-02 visual/interaction redesign**: the UI layer described
      above (top toolbar, tool `Picker`, scan-on-load box overlay only) was
      replaced with the calm-dark, right-inspector design in
      `design_handoff_swiftui_redesign/` — see the UX fixes checklist right
      below for exactly what changed. The FFI/engine integration notes and
      gotchas above (linking, `AnnotationColor` naming, box-detection bugs)
      are still accurate and still apply; only `ContentView`'s layout and
      the field-overlay/sign-flow views were reworked, not the store's FFI
      call sites.

      **2026-07-12 performance + field-detection accuracy pass** (Wes tested
      the build: "slow to load, especially opening the file picker" and
      "fillable fields are still off — highlights things that aren't fields,
      misses real fields; should only highlight fields that have labels"):
      - *Accuracy*: new `pdfree_core::fields::fillable_fields` replaces the
        raw `boxes_on_page` scan as the macOS overlay source (also exposed
        over FFI + wasm; web UI adoption is follow-up). It returns every
        `AcroForm` widget (so a real field with no drawn box is never missed)
        **plus** only those detected boxes that have a human-readable label
        immediately to their left or just above them (so decorative/layout
        rectangles with no label stop being highlighted) — the exact "only
        detect fields with labels" ask. Label-matching (`best_label`) is a
        pure function, unit-tested without `PDFium`; a labeled "Signature"
        line on a flat form routes to the sign flow (new
        `ActiveSheet.signatureBox`) instead of a text caret. `PDFDocumentStore`
        dropped `detectedBoxes`/`computeFieldOverlays` for this single call;
        the canvas field-count chip and `boxContaining` now read the
        label-aware `fieldOverlays`. See `docs/api.md`'s "Fields" section.
      - *Performance*: every `PDFium`-backed FFI call (`fromBytes`, render,
        page-size, field scan, and all mutations) now runs off the main
        thread on one **serial** `ffiQueue` in `PDFDocumentStore`, publishing
        results back on main — so opening a document, flipping pages, filling,
        and signing never freeze the UI. Serial on purpose: `PDFium` isn't
        safe to bind/drive from two threads at once. `docToken`/`renderToken`
        generation guards drop stale background results (fast page flips apply
        only the newest render). Added free byte-slice `render_page`/
        `page_size` FFI functions so rendering/measuring happen from `Data`
        off-main without sharing a `Document` handle across threads;
        thumbnails render lazily on the same queue. Field overlays carry their
        label as tooltip/accessibility text (CLAUDE.md UX research on
        labelless controls). Rust builds/tests/clippy/fmt clean across
        core/ffi/wasm; the Swift app couldn't be compiled from this sandbox
        (no Xcode/SwiftUI SDK) — call sites were verified against the FFI
        metadata and reviewed by hand, same limitation class noted elsewhere
        in this doc.
- [ ] **UX fixes from 2026-07-01 review** (see Core UX Principles above for
      full rationale) — these should land before further platform-shell work:
  - [x] Fix default zoom to fit-whole-page on load and on resize — the
        macOS canvas now measures its available area with a `GeometryReader`
        (`ContentView.canvasArea`) and calls `PDFDocumentStore.updateViewport`
        on appear and on every size change; the store re-renders the current
        page at the DPI from `fitToPageDpi` (backed by the new
        `renderer::fit_to_page`) instead of a fixed 150 DPI. Verified against
        the IRS 1040 test doc at several window sizes — whole page visible,
        no scrollbar, no clipping, box overlays stay aligned since gesture
        coordinates are still derived from the same rendered image's own
        pixel size (`pagePointSize`).
  - [x] Auto-run `boxes_on_page` + AcroForm field scan on load for *every*
        page up front — `PDFDocumentStore.computeFieldOverlays` merges
        `detectedBoxes` (vector scan) with `formFieldsList` (now carrying
        `page`/`rect`, see the Phase 1 gap below) into `fieldOverlays`, and
        any signature/initials field left unmatched by the vector scan still
        gets an overlay synthesized directly from its own `FormField` rect —
        so a real signature field is never silently undiscoverable just
        because it has no drawn box around it. All rendered immediately on
        load, no manual placement needed as the primary flow.
  - [x] Detect signature/initials fields by label pattern and route them to
        the sign flow (draw/type/upload/reuse-saved) instead of a text field
        — `PDFDocumentStore.isSignatureField` matches `FieldKind == .signature`
        or the name against `/sign|initial/i`; `PageCanvasView` renders those
        as an amber "Sign here" affordance (never a text caret) and
        `ContentView.beginSigning` routes the click into the sign flow
        (`SignatureSheet` first-time, `SignPopover` once a mark of that kind
        is saved). Verified against a synthetic AcroForm fixture with a
        `signature_1` text field (real-world fixtures on hand — IRS 1040,
        `form_sample.pdf` — happen to have zero true signature-kind or
        sign-named fields, so this fixture was necessary to exercise the
        path at all).
  - [x] `overlay_text` shrink-to-fit — audited and fixed. `overlay_text`
        itself was always deterministic (it stamps literally at whatever
        `font_size` it's given, no internal resizing), but the *caller's*
        two font-size formulas had drifted: the live editor sized off the
        box's on-screen **pixel** height while the export path sized off the
        box's **PDF-point** height — coincidentally close at common DPIs, but
        not the same number, and neither accounted for text **width**
        overflowing the box at all. Fixed with `TextFit.swift`: one pure
        function (text, box width/height in PDF points → font size), called
        with identical inputs by both `PageCanvasView`'s live `TextField`
        (converted back to pixel space for display) and
        `ContentView.commitInlineEdit`'s `overlay_text` call — so the same
        text can never render two different sizes between edit and export.
  - [ ] `forms::fill()` shrink-to-fit — investigated, confirmed still
        blocked, not attempted. A real `AcroForm` text field's rendered size
        is governed by its `/DA` (default appearance) string; many
        real-world fillable PDFs (auto-sized fields, font size `0` in `/DA`)
        leave that sizing to whatever viewer renders them, which is the
        actual "drifts between viewers" risk. Fixing it means writing an
        explicit computed size into the field's `/DA` — but `pdfium-render`
        0.8.37's `PdfFormTextField` has no font-size/`/DA` setter, and the
        `FPDF_ANNOTATION` handle needed to drop to the raw
        `FPDFAnnot_SetStringValue` binding is a private field with no public
        accessor, so this isn't reachable through the crate's safe API at
        all (confirmed by reading `pdfium-render`'s source, not just
        skimming docs). Lower priority than it first appears, though: the
        macOS app's actual interactive fill path is `overlay_text` (fixed
        above), not `forms::fill()` — the latter is only reachable through
        the secondary `FormsPanel` sheet. Real fix needs either an unsafe
        raw-FFI escape hatch (risk noted in the Phase 1 dropdown/list-box gap
        above applies equally here) or a `pdfium-render` upstream change.
  - [x] Add persistent "+" quick-action (import/merge/split/add page) in the
        main canvas UI, not just File menu — the inspector's "Add or merge"
        button opens `AddMenuPopover` (Open / Merge / Insert blank page /
        Image as a page / Split or extract).
  - [x] Trim default toolbar to: open, fill, sign, add/remove page, export —
        the old top toolbar (Open/Save/tool picker/Merge/Insert Image/Split/
        Extract Text/Fill Form buttons) is gone; those actions now live in
        the right `InspectorView` (Add or merge, Fill fields, Sign, Annotate,
        Insert/Rotate/Delete page, Export).
  - [x] Add saved-signature reuse (store drawn/typed/uploaded signature,
        insert on later documents without redrawing) — `SavedSignature` +
        `PDFDocumentStore`'s PNG-blob-plus-JSON-index persistence in
        `~/Library/Application Support/PDFree/signatures/`, loaded on
        launch; `SignPopover` shows saved marks as tap-to-place chips and
        hops to the next pending signature/initials field after each
        placement (spring animation), ending on a "Everything's signed"
        confirmation with a reset. Verified across an app relaunch.
  - [x] Lightweight local audit metadata on sign (timestamp, signer name,
        device info where available) — not the deferred certified/legal-grade
        trail, see Potential Paid Features above. `signatures.rs` gained
        `SignatureAudit` + `place_signature_with_audit`: stamps a small
        caption directly beneath the signature image reading
        `"Signed by {name} · {timestamp} · {device}"`. **Investigated and
        confirmed not embeddable as invisible document metadata**:
        `pdfium-render` 0.8.37's `PdfMetadata` is read-only (no `set()` at
        all, not even for the standard Title/Author/etc. tags) — so a
        visible on-page caption is the only audit mechanism reachable
        through the crate's safe API, not a deliberate design choice over a
        metadata approach. `PDFDocumentStore.applySignature` now calls
        `placeSignatureWithAudit`, defaulting signer name to the macOS
        account's full name (`NSFullUserName()`, persisted, no extra setup
        required) and device info to the OS version string — there's no
        "enter your name" UI yet, which is fine for a single-user local app
        but would need one before multi-account or export-and-forget use
        cases matter.
  - [x] Automated test suite — added, though scoped as unit/integration
        tests rather than full drag-and-drop/file-picker UI automation
        (XCUITest driving real OS-level drag gestures and NSOpenPanel is a
        much larger lift; deferred, see below). New `PDFreeTests` target in
        `project.yml` (`xcodebuild -scheme PDFreeTests test`): compiles
        `Sources/PDFree` + `Sources/Bridge` directly into the test bundle
        (simpler than host-application `@testable import` for a project this
        size — see the target's comment) with the same dev-only FFI linker
        settings as `PDFree` itself. 12 tests across two files:
        `TextFitTests.swift` (pure `TextFit` shrink-to-fit math — the
        determinism guarantee the WYSIWYG fix above depends on) and
        `PDFDocumentStoreTests.swift` (open/close, form-field loading,
        signature/initials classification, and specifically a regression
        test for the "unmatched signature field gets no overlay at all" bug
        fixed this pass — a synthetic `Tests/Fixtures/signature_fields.pdf`
        with zero vector graphics, so the only way its fields get an overlay
        is the synthesis path). Skips gracefully (not fails) when the
        `PDFium` dylib isn't bundled, matching `pdfree-core`'s own test
        convention. Deliberately does **not** exercise `saveSignature`/saved-
        signature persistence — that writes real files under
        `~/Library/Application Support/PDFree/` with no sandboxing hook, and
        polluting the developer's actual app-data directory on every test
        run is worse than the coverage gap. Still open: drag-and-drop import
        and the full click-driven fill→sign→export path (verified manually
        this pass via computer-use — empty state, open, fill, sign sheet +
        popover + hop + done, drag-reorder, export panel, window resize —
        but not automated).
  - [x] Page thumbnail sidebar (macOS shell) — `PagesSidebarView` reworked to
        the 88×114pt thumbnail rail spec (green ring + shadow on the current
        page), including drag-to-reorder (`onDrag`/`onDrop` +
        `PageReorderDropDelegate`, wired to the pre-existing
        `PDFDocumentStore.movePages`). Deliberately does **not** live-reorder
        the list on every drag-over event the way `List`'s `.onMove` would —
        `movePages` round-trips the whole document through the FFI
        (re-serializing the PDF), too expensive to run on each hover — so the
        dragged thumbnail just dims and the actual reorder commits once, on
        drop.
- [x] `pdfree-core`: `src/pdfium.rs` split into `#[cfg(not(target_arch =
      "wasm32"))] mod native` (the original vendor-dir/system-library dylib
      search, unchanged) and `#[cfg(target_arch = "wasm32")] mod wasm` (calls
      `Pdfium::bind_to_system_library()`, which on wasm32 checks
      `PdfiumRenderWasmState::lock().is_ready()` — i.e. requires JS to have
      already called the crate's auto-exported `initialize_pdfium_render()`).
      Both re-exported under the same `pdfium::bind()` name so every other
      module in `pdfree-core` is unaffected. Needed because
      `pdfium-render` 0.8.37 gates `bind_to_library`/`pdfium_platform_library_name`
      behind `#[cfg(not(target_arch = "wasm32"))]` — confirmed by reading the
      crate source, not guessed. Native build/tests (34 tests) and a
      `wasm32-unknown-unknown` build both verified green.
- [x] `pdfree-wasm`: expanded from the Phase 0 stub (open/pageCount/title/
      author/renderPage only) to the full v1 surface — mirrors
      `pdfree-ffi` function-for-function: forms, signatures (incl. audit),
      annotations, editor, pages, convert, boxes. Complex types cross the JS
      boundary as plain camelCase JSON via `serde-wasm-bindgen` (`to_value`/
      `from_value` helpers) rather than typed UniFFI records, since
      wasm-bindgen can't marshal arbitrary Rust enums/structs directly.
      **AI (Phase 5/6) is deliberately not exposed here** — `pdfree-ai`'s
      `reqwest::blocking` HTTP calls don't work in a browser; a real web AI
      integration needs `fetch` via `web-sys`, noted as future work, not
      attempted this pass. Builds clean native + wasm32; `cargo fmt`/
      `clippy -D warnings` clean on native (wasm32 clippy hit an unrelated
      toolchain-cache collision between Homebrew's and rustup's rustc
      sharing one `target/` dir — the actual `cargo build` for wasm32
      succeeded cleanly regardless, so this was left as a known tooling
      quirk, not a code defect).
- [x] `scripts/build-wasm.sh` — rewritten from a `wasm-pack`-based stub to a
      direct `cargo build --target wasm32-unknown-unknown --release` +
      `wasm-bindgen` CLI invocation (`--target web`, out to
      `apps/web/src/wasm/`). Requires `rustup target add wasm32-unknown-unknown`
      and `cargo install wasm-bindgen-cli --version 0.2.126 --locked` (version
      must exactly match the `wasm-bindgen` crate version in `Cargo.lock`, or
      the module fails at runtime with a schema mismatch, not a build error).
- [x] `apps/web` — Vite + React 18 + TypeScript (strict) app wired directly to
      `pdfree-wasm`, implementing the full Core UX Principles set: fit-to-page
      zoom recomputed on `ResizeObserver` resize; scan-on-load field overlays
      (`boxesOnPage` + `formFields` merged, unmatched signature fields
      synthesized an overlay from their own rect — same merge logic as
      `PDFDocumentStore.swift`, ported to `usePdfDocumentStore.ts`); signature/
      initials fields routed to a distinct sign flow, never a text input;
      WYSIWYG shrink-to-fit via `lib/textFit.ts` (direct port of
      `TextFit.swift`, using Canvas 2D `measureText()` instead of `NSFont`
      metrics — one function computes the size, used identically by the live
      `<input>` and the final `overlayText` stamp); persistent "+" quick
      actions and a right `Inspector` (no menu bar to bury actions in, so this
      was simpler here than on macOS); minimal toolbar by construction (no
      top toolbar at all — titlebar + canvas + inspector only).
      **PDFium WASM binary is not bundled** — `apps/web/public/pdfium/README.md`
      documents the manual setup (source: `paulocoutinhox/pdfium-lib` GitHub
      releases) and the `initialize_pdfium_render()` wiring, same policy as
      `vendor/pdfium/` for native (fetched, gitignored, never committed) and
      left un-downloaded here since fetching a third-party binary needs
      explicit user permission. **Verified live** via the Claude Preview tool:
      dev server boots, our own compiled `pdfree_wasm.js`/`.wasm` load
      successfully, and the app correctly surfaces "PDFium failed to load —
      PDFium WASM module not found" (the expected/correct behavior with no
      real `pdfium.wasm` present) — confirmed via screenshot, console logs,
      a11y snapshot, and direct DOM inspection. `npx tsc --noEmit` (strict)
      and `npm run build` (→ `apps/web/dist/`) both clean.
- [x] `apps/desktop` — Tauri v2 project (`src-tauri` added as workspace member
      `pdfree-desktop`) with `src/commands.rs` wrapping `pdfree-core` directly
      (native, no WASM) as `#[tauri::command]` functions — stateless, take
      `pdf_bytes: Vec<u8>` fresh each call (matches `pdfree-core`'s own
      function shapes rather than a persistent document handle, since Tauri
      IPC calls are stateless). Covers the subset `apps/web`'s current UI
      calls (document info, render, form fields, overlay text, boxes, sign
      w/ audit, merge, rotate, extract, image-to-PDF) — documented as not
      full parity with `pdfree-wasm`. **Genuinely reuses the web frontend**,
      not just nominally: `apps/web/src/lib/runtime.ts`'s `isTauri()` checks
      `window.__TAURI_INTERNALS__`, and `lib/engine.ts` is the single module
      that branches per-function between calling the WASM module directly or
      `invoke()`-ing the matching Tauri command — every function is `async`
      uniformly across both backends so calling code never branches.
      Functions not yet wired as Tauri commands throw a clear
      "not available under Tauri yet" error rather than silently
      misbehaving. `frontendDist` points at `apps/web/dist`, `devUrl` at the
      Vite dev server. Verified: `cargo build -p pdfree-desktop` succeeds
      (native, pulls in real Tauri deps), `cargo fmt`/`clippy -D warnings`
      clean, full `cargo build --workspace` + `cargo test --workspace` still
      green with this crate added. **Not verified**: an actual running Tauri
      window (needs a GUI session this sandbox doesn't have) — same class of
      gap as the macOS AI panel's noted computer-use limitation.
- [x] `apps/ios` — new Xcode project (`xcodegen`-generated from
      `project.yml`), **explicitly not a port of macOS's AppKit-flavored
      views** — `apps/macos/Sources/PDFree/Views/*.swift` use `NSImage`/
      `NSFont`/`NSPasteboard`/`NSOpenPanel`/`NSFullUserName()`, none of which
      exist on iOS/UIKit, so literal file-sharing isn't possible without a
      real cross-platform abstraction layer (out of scope this pass). What
      *is* shared, per the roadmap's actual intent: the entire Rust engine
      and the FFI interface — `apps/ios/Sources/Bridge/pdfree_ffi.swift` is
      generated from the exact same `crates/pdfree-ffi` crate, unmodified.
      `ContentView.swift` is a minimal but real SwiftUI shell (`fileImporter`
      → `PdfDocument.fromBytes` → `pageSize` → `fitToPageDpi` → `renderPage`
      → `UIImage`), not a stub.
      `scripts/build-ios.sh` (new) builds `pdfree-ffi` as a `staticlib` for
      both `aarch64-apple-ios` (device) and `aarch64-apple-ios-sim`
      (simulator) — iOS requires a signed, bundle-embedded static lib, unlike
      macOS dev builds which can `dlopen` an unsigned absolute-path dylib —
      then combines them into `apps/ios/Frameworks/PdfreeFFI.xcframework` via
      `xcodebuild -create-xcframework`. Confirmed the XCFramework contains
      real `ios-arm64/libpdfree_ffi.a` and `ios-arm64-simulator/libpdfree_ffi.a`
      slices with headers.
      **Known, unsolved gap** (documented in `ContentView.swift`'s doc
      comment, surfaced as a real runtime error rather than hidden):
      `pdfree_core::pdfium::bind()`'s native path assumes a filesystem to
      search (vendor dir, then system library) — that doesn't exist inside an
      iOS app sandbox. A real iOS PDFium integration needs its own bundled
      `.xcframework` (mirroring `docs/pdfium-bundling.md`'s per-platform
      strategy) plus an iOS-specific `pdfium.rs` binding branch; not
      attempted this pass, same shape of gap as the web app's "PDFium WASM
      module not found" state.
      **Build verification**: `xcodebuild -destination 'generic/platform=iOS
      Simulator'` failed in this sandbox with a `CoreSimulator is out of
      date` / `iOS 26.5 is not installed` error — an environment-level Xcode/
      simulator-runtime gap, not a code defect. Worked around by building
      directly against the SDK instead of a device destination
      (`xcodebuild -sdk iphonesimulator ARCHS=arm64 ONLY_ACTIVE_ARCH=YES
      build`), which exercises the entire real toolchain (Rust cross-compile,
      UniFFI Swift codegen, bridging header, linking against the XCFramework,
      app-bundle assembly) end-to-end — **`** BUILD SUCCEEDED **`**, producing
      a real signed-for-dev `PDFree.app` with the 16MB linked Rust dylib
      embedded. Actually *booting* it in a simulator is still blocked by the
      same CoreSimulator version mismatch (`xcrun simctl` hangs/fails in this
      sandbox) — that's an environment limitation to resolve outside this
      session (Xcode → Settings → Components, or a CoreSimulator reinstall),
      not something to work around further here.

### Phase 5 — AI Tier 1 (Core AI)
- [x] `pdfree-ai/provider.rs`: `Provider` trait with two real backends —
      `OllamaProvider` (local, `POST /api/generate` against
      `http://localhost:11434` by default, confirmed working against
      `qwen3:4b`/`qwen2.5:7b`/`qwen3:8b` in dev) and `AnthropicProvider`
      (cloud, `POST /v1/messages`, API key supplied by the caller — never
      hardcoded, constructing this provider *is* the user's cloud opt-in).
      Both are real HTTP round-trips, not stubs; unit tests exercise both
      against live endpoints (skip, don't fail, when unavailable — same
      pattern as `pdfree-core`'s `skip_without_pdfium!()`).
- [x] `ocr.rs`: shells out to the `tesseract` CLI (`recognize(page_png)`) —
      write bytes to a temp file, run `tesseract <in> <out>`, read
      `<out>.txt`, clean up. Chose shelling out over a `tesseract-sys`
      binding since the binary is already a platform dependency users may or
      may not have, and this keeps the crate's own dependency tree free of a
      C toolchain requirement. **Apple Vision on macOS/iOS is not
      implemented** — `ocr.rs` is intentionally single-backend for now;
      revisit if `tesseract` proves too heavy/slow on-device.
- [x] `rag.rs`: chunk → retrieve → answer, but retrieval is **lexical
      (word-overlap scoring with a small stopword list), not
      embedding-based** — no embedding model to download or run, so
      single-document Q&A stays fully on-device with zero extra setup.
      `chunk()` splits on word-count windows with configurable overlap;
      `retrieve()` ranks chunks by stopword-filtered token overlap
      (mild `sqrt`-length normalization so one giant chunk doesn't win
      purely by size); `answer()` wires both together against
      `pdfree_core::convert::to_text` and a `Provider`. Kyro/Kohaku/Vectro
      wiring (real vector index, cross-document library search) is
      deliberately **not** pursued yet — worth revisiting only if usage
      patterns ever span a whole library rather than one open document at a
      time; the doc comment now says this explicitly instead of implying
      it's still planned as-is.
- [x] `summarize.rs`: extracts text, then map-reduces if the document is too
      long for one pass (chunk → summarize each chunk → summarize the
      summaries) — `MAX_SINGLE_PASS_WORDS = 6000` conservative ceiling,
      chosen for local models' more limited context windows plus headroom
      for the prompt wrapper.
- [x] `formfill.rs` (smart form fill from profile): given a document's
      detected `AcroForm` fields and an arbitrary user profile
      (`HashMap<String, String>`), asks the model to propose a field-name →
      profile-value mapping and returns it as a **suggestion list**, not an
      in-place write — a wrong guess is meant to be caught in a review UI
      before ever reaching `forms::fill`. Signature/initials fields are
      always excluded (Core UX Principle #3 routes those to the sign flow);
      dropdown/list-box/radio fields are excluded too, since `forms::fill`
      has no setter for them yet (see the Phase 1 gap above) — suggesting a
      value there would be a promise this layer can't keep. Hallucinated
      field names the model invents are silently dropped rather than
      written.
- [x] FFI: `pdfree-ffi` now depends on `pdfree-ai` and exports
      `ai_summarize`, `ai_rag_answer`, `ai_ocr_recognize`, and
      `ai_suggest_form_fills`, all taking an explicit `AiProviderConfig`
      enum (`Ollama { model, base_url }` / `Anthropic { api_key, model }`)
      — there's no default/stored provider baked into the FFI layer, so
      every call site is forced to make the local-vs-cloud choice visible,
      per the "no silent uploads" rule. `PdfFreeError` gained an
      `AiProvider(String)` variant; `pdfree_ai::AiError` maps onto the
      existing flat-error scheme (`Core` → the existing `PdfError` mapping,
      `Provider` → the new `AiProvider` variant, `NotImplemented` → the
      existing `NotImplemented` variant).
- [x] Document Q&A chat UI; auto-summary UI — `apps/macos/Sources/PDFree/Views/AIPanel.swift`,
      reached via a new "AI" group in `InspectorView` ("Ask AI") and a new
      `ActiveSheet.aiAssistant` case in `ContentView`. One sheet, two modes
      (segmented control): Summarize and Ask a question, both backed by the
      same provider picker (`AiProviderConfig`: on-device Ollama with a model
      field, or cloud Anthropic with a secure API-key field) — defaults to
      Ollama, so the AI panel opens local-first with no setup, and choosing
      Anthropic is the explicit per-CLAUDE.md cloud opt-in. The result area
      always labels which one actually ran ("Ran on-device" / "Ran via
      Anthropic"), and the Anthropic path shows an amber warning that the
      document's text leaves the machine — no silent uploads. FFI calls
      (`aiSummarize`/`aiRagAnswer`) are blocking Rust (`reqwest::blocking`),
      so `AIPanel` dispatches them via `DispatchQueue.global(qos:
      .userInitiated)` rather than Swift concurrency — matches the one other
      place this app already does async work (`EmptyStateView`'s
      `DispatchQueue.main.async`), so there's now no mixed
      Task-based/callback-based async style in the app. Ollama model name and
      Anthropic API key are persisted via `UserDefaults` (`PDFree.ai.*` keys)
      so they don't need retyping — same pattern as `signerName`/
      `recentFiles`; **known simplification**: this is plaintext, not
      Keychain, worth revisiting before this ships. **Verification status**:
      the full macOS app target (including this panel) builds clean via
      `xcodebuild` with zero warnings, `pdfree-ffi`'s new `ai_*` exports and
      `AiProviderConfig`/`SuggestedFormFill` were confirmed present with the
      expected signatures in the regenerated
      `apps/macos/Sources/Bridge/pdfree_ffi.swift`, and the underlying
      `summarize`/`rag::answer` code this panel calls is the same code
      already exercised by real (non-mocked) `pdfree-ai` tests against live
      Ollama/tesseract/Anthropic. **Not verified**: an actual interactive
      click-through (open a doc → click Ask AI → run Summarize → see a real
      result on screen) — attempted via computer-use in this sandbox but
      blocked by an environment-level input issue (clicks landing well above
      their intended y-coordinate, e.g. a click aimed at the window's
      traffic-light buttons reproducibly opened the menu-bar's View menu
      instead), not a defect traced to the app itself. This is the same class
      of sandbox limitation noted for the Phase 4 redesign
      ("end-to-end click-driven UI testing... wasn't done from this sandbox").
      Worth a real click-through pass next time this app is tested from a
      environment where computer-use click coordinates are reliable.
- [x] **Engine-side quick wins from the 2026-07-03 feature research pass**
      (merged from PR #9) — 4 features buildable/testable without a
      macOS/Xcode toolchain or network: `search::find_text` (in-document
      "⌘F", reuses `editor::text_runs`), `bookmarks::outline` (document
      outline tree, wraps `pdfium-render`'s bookmark API), `pages::bates_number`
      (sequential legal/discovery stamping, reuses the `overlay_text`
      primitive), and `pdfree_ai::confidence::ground_check` (grounding/
      hallucination check for any future AI-produced value — no model call,
      no provider needed). All four are pure `pdfree-core`/`pdfree-ai`, fully
      unit tested (`tests/search.rs`, `tests/bookmarks.rs`, the `bates_*`
      tests in `tests/pages.rs`, `confidence::tests` inline) — see
      `docs/api.md` and `docs/ai-design.md`. **Shell wiring** (search UI,
      outline sidebar, Bates dialog) is not yet done.

### Phase 6 — AI Tier 2 (Differentiators)
- [x] `redact.rs`: PII detection is regex-based (SSN, email, phone, credit
      card — the last Luhn-checked to cut false positives on arbitrary
      digit runs), not an LLM call — deterministic and fully local, matched
      against `pdfree_core::editor::text_runs`. **Redaction actually
      overwrites the underlying text** — reuses `editor::replace_text`,
      which mutates the matched text object's own content, not a visual
      overlay on top of it. Placeholder is `'X'` repeated, not a block
      character (`█`): the standard-14 PDF fonts `pdfree_core` writes text
      in have no glyph outside Latin-1, so a block char would silently fail
      to encode or vanish on read-back — discovered by the redact round-trip
      test failing until switched. **Known scope boundary**: matched
      positions are the *containing text run's* bounding box (no sub-string
      bounds available), so `PiiSpan::{x,y,width,height}` are good enough
      to highlight "the run this PII is in" for a review UI, not to draw a
      glyph-tight redaction rectangle. Freeform PII (names, addresses) isn't
      covered — would need an LLM pass layered on top; not pursued yet,
      the structured kinds above are the common, unambiguous case. 7 tests,
      including a real PDFium round-trip (stamp real PII text via
      `forms::overlay_text`, detect it, redact it, confirm the original text
      is gone from re-extracted text and the placeholder is present).
- [x] `extract.rs`: table extraction reuses `pdfree_core::boxes` (the same
      lattice-based ruled-line cell reconstruction that powers the macOS
      app's box-on-load scan) — cells are clustered into rows by y-position
      (row-of-boxes → sorted by x within the row), and each cell's text
      comes from `pdfree_core::editor::text_runs` whose center point falls
      inside the cell's box. Fully local, geometry-driven — no LLM call,
      matching the module's original "specialized extractors + LLM
      validation, not LLM alone" design principle (the LLM-validation half
      isn't wired up yet, since the geometry-only extractor already tested
      correct on a real form). A page needs ≥2 rows and at least one row
      with ≥2 columns to count as a table, so incidental non-grid boxes
      (checkboxes, signature boxes) on a page don't get misread as a
      degenerate table. **Contract analysis is out of scope for this
      pass** — that's LLM-over-freeform-text work, a different shape of
      feature from table extraction; tracked as a follow-up, not attempted
      here. 5 tests: 4 synthetic (clustering logic, blank-cell handling,
      too-few-cells / single-row rejection) plus a real pass against the
      IRS 1040 fixture confirming at least one genuine multi-row, multi-
      column grid is found (not asserting exact cell text — real-world
      grids are often ragged, e.g. merged cells or single-column
      continuation rows, so the test checks structural sanity, not
      uniformity).
- [x] `classify.rs`: LLM-driven classification into a small fixed label set
      (`contract`, `invoice`, `tax_form`, `receipt`, `letter`, `form`,
      `resume`, `report`, `other`) via a `Provider` prompt over the
      document's extracted text (first ~1500 words — enough to identify a
      document's type without a large prompt). Response parsing is a
      case-insensitive substring match against the known label list
      (tolerates the model wrapping its answer in prose despite being asked
      not to), falling back to `"other"` rather than ever surfacing an
      unrecognized/hallucinated label to a caller's UI. **Embeddings-based
      semantic search and whole-library auto-organization are explicitly
      out of scope** — same tradeoff already documented on `rag.rs`: no
      local embedding model to download/run, and worth revisiting only if
      usage ever spans a whole library rather than one open document. 7
      tests: label-parsing edge cases (exact match, embedded in prose,
      case-insensitivity, `tax_form` vs. the shorter `form` label,
      unrecognized-response fallback), a fast-fail check that a blank
      document never reaches the model, and a real pass against the IRS
      1040 fixture via local Ollama (asserts the result is *some* known
      label, not an exact one — small local models don't reliably agree on
      `tax_form` vs. `form`).
- [x] FFI: `pdfree-ffi` exports `ai_detect_pii`, `ai_redact`,
      `ai_extract_tables` (all fully local, no `AiProviderConfig` needed —
      matches these functions' actual local-only implementation, doesn't
      pretend they need a provider choice they don't use), and `ai_classify`
      (takes `AiProviderConfig`, same local/cloud choice as the Phase 5 AI
      functions). New records: `PiiSpan`/`PiiKind` (mirrors
      `pdfree_ai::redact`), `Table`/`TableRow` (a table is `Vec<TableRow>`,
      a row is `{cells: Vec<String>}` — chosen over exposing the engine's
      raw `Vec<Vec<Vec<String>>>` directly so Swift gets named types
      (`Table`, `TableRow`) instead of anonymous triple-nested arrays;
      not verified whether UniFFI would have accepted the bare nested-Vec
      return type, this is a readability choice, not a worked-around
      limitation). Full workspace build/test clean; Swift bindings
      regenerated and the macOS app target still builds against them (Phase
      6 functions aren't wired into any UI yet — that's unscoped follow-up
      work, same as Phase 5's FFI-before-UI landing order).

### Phase 7 — AI Tier 3 (v2+ expansion)
- [ ] Layout-aware translation, editing; voice-to-fill; grammar/tone rewrite.
      **This bullet actually bundles 4 distinct features**, broken out below
      with research findings (2026-07-03) grounding each one, since they
      differ a lot in engineering risk:
  - [ ] **Grammar/tone rewrite** — lowest-risk of the four, same shape as
        every LLM feature already shipped. New `pdfree-ai/rewrite.rs`: given
        text (a single `editor::TextRun`'s content, to start) and an
        instruction ("fix grammar", "more formal", "more concise"), one
        `Provider.complete()` call, returns suggested replacement text —
        never auto-applied, handed to `editor::replace_text` only after the
        user confirms (same "suggestion, not a write" rule every other AI
        feature in this crate follows). Whole-document proofread is a
        natural v2 extension of the same primitive once single-run rewrite
        is proven, not a separate feature.
  - [ ] **Layout-aware translation** — the hard part is *reflow*, not
        translation (any `Provider` can translate text; the risk is what
        happens when the translated string doesn't fit the original run's
        box). Two concrete findings from researching `pdfium-render`
        (0.8.x, `ajrcarey/pdfium-render` on GitHub) before scoping this:
      - Font coverage: this codebase's existing text-writing paths
        (`redact.rs`'s placeholder, `forms.rs`'s overlay) all use PDFium's
        built-in standard-14 fonts, which only cover Latin-1 — no Cyrillic,
        CJK, Arabic, or Hebrew glyphs. Translating into those scripts needs
        a real embedded font. `pdfium-render` 0.8.3 introduced a `PdfFonts`
        collection (moved off `PdfFont`, fixing a prior borrow-checker
        issue tracked as [ajrcarey/pdfium-render#79](https://github.com/ajrcarey/pdfium-render/issues/79))
        with `load_true_type_from_file()` confirmed to exist for native
        targets (WASM-only variants `load_true_type_from_fetch()`/
        `load_true_type_from_blob()` also exist) — strongly suggesting a
        native bytes-based loader exists too following this crate's
        consistent bytes/file/fetch/blob loader pattern elsewhere (e.g.
        `Pdfium::load_pdf_from_byte_slice` vs. `load_pdf_from_file`), but
        **the exact native from-bytes function name needs confirming
        against the crate source at implementation time** — don't guess it
        the way this doc's own convention (see the Phase 4 `pdfium.rs`
        native/wasm split) insists on reading source over assuming API
        shape. **Recommendation: scope v1 of this feature to Latin-alphabet
        language pairs** (English ↔ Spanish/French/German/etc., where the
        existing standard-14 fonts suffice) and defer non-Latin scripts
        until a font is actually bundled and the loader's confirmed.
      - Reflow math: the shrink-to-fit algorithm (`TextFit.swift` /
        `lib/textFit.ts`) that makes WYSIWYG text sizing deterministic
        (Phase 4's UX-fix item) currently lives duplicated client-side, per
        platform. Translation needs this exact primitive (translated text
        measured against the original run's box, shrunk/wrapped to fit) —
        **worth porting into `pdfree-core` once** (a pure function over
        PDFium's own text-measurement API) so shells and this new feature
        share one implementation instead of a third client-side copy
        drifting the way pre-fix Phase-4 sizing already did once.
      - "Layout-aware editing" (the middle clause of the original bullet) is
        this same reflow primitive, not a separate feature — recommend
        folding it in rather than building the same thing twice under a
        different name.
  - [ ] **Voice-to-fill** — capture is inherently platform-specific
        (AVFoundation on macOS/iOS, `MediaRecorder`/Web Speech API on web,
        native mic access under Tauri), but speech-to-text itself doesn't
        have to be: researched **`whisper-rs`** ([tazz4843/whisper-rs](https://github.com/tazz4843/whisper-rs),
        ~115k downloads/month, Rust bindings to `whisper.cpp`) as the
        on-device option — keeps this local-first like every other AI
        feature, matching the exact "shell out to a CLI/native lib rather
        than a cloud call" tradeoff `ocr.rs` already made for `tesseract`.
        GPU acceleration (Metal/CUDA/ROCm) is available per-platform, which
        matters for real-time-feeling transcription. Once transcribed, the
        genuinely new engine work is *just* the STT step — mapping a
        transcript onto form fields is a near-verbatim reuse of
        `formfill::suggest_fills`'s existing shape (a `Provider` call given
        field names/kinds + free text, returning a suggested field→value
        map); recommend either calling it with the transcript standing in
        for a profile, or a thin `suggest_fills_from_text` variant rather
        than duplicating the mapping logic.
- [x] Schema-driven extraction; document diff/redline — see below.
      **Deliberately scoped to just these two items this pass**: Phase 7
      bundles several loosely-related features under one umbrella; asked
      Wes which to start with (2026-07-03), and the answer was these two
      specifically, over translation/voice-to-fill/grammar-rewrite (heavier
      UI surface, needs a translation-capable provider + per-platform mic
      capture) and lopi-agentic-workflows (blocked on lopi's actual
      interface, which isn't in this workspace) — see the sibling checklist
      items above/below, still unstarted.
  - [x] `pdfree-ai/schema_extract.rs`: given a caller-supplied schema (field
        name + a human description of what it means — the description is
        what the model actually reads, since a bare name like `"total"` is
        often ambiguous on a real document) and a `Provider`, returns a
        suggestion list of fields it found an actual value for. Same
        shape as `formfill.rs`'s profile-mapping problem, inverted: never
        auto-applied, hallucinated field names are silently dropped (same
        defensive pattern), and long documents map-reduce across chunks
        merging the first non-empty value found per field (documented as a
        simple, non-confidence-scored merge — an earlier chunk's wrong
        guess can shadow a better match later in the document, acceptable
        for a suggestion list a human reviews before anything's written).
  - [x] `pdfree-ai/diff.rs`: compare two versions of a PDF, page-aligned
        (`pdfree_core::convert::to_text_per_page` — a new small core
        primitive split out of the existing `to_text`, so page boundaries
        don't have to be inferred by splitting a joined string back apart),
        word-level diff via a classic suffix-LCS dynamic program (not a
        diff crate — page-sized text is comfortably within the DP's
        practical O(n\*m) range, and this avoids a new dependency for
        something straightforward to implement and test directly). Fully
        local, no LLM — same "specialized extractor, not an LLM" tradeoff
        as `extract.rs`'s table extraction. Produces per-page
        Added/Removed/Unchanged runs; pages are aligned by index, not
        content-matched, so a page inserted/removed mid-document shows
        every following page as fully changed — noted as a real scope
        boundary, not silently glossed over, since fixing it would need a
        page-similarity matching step before the word-level diff runs. A
        page pair over 2,000 words on either side falls back to a coarse
        whole-page Removed/Added (or Unchanged) pair rather than the O(n\*m)
        DP, since the DP table is quadratic in both time *and* memory.
        16 tests: word-level edit cases (replace/insert/delete), page-count
        mismatches (whole page added/removed falls out naturally from an
        empty-string comparison, no special-casing needed), the coarse-
        fallback cap, and a real round-trip — stamp a distinctive string
        onto a copy of the IRS 1040 fixture via `forms::overlay_text`, diff
        original vs. modified through real `PDFium` text extraction, confirm
        the addition is detected on the right page and every other page
        reports fully unchanged.
  - [x] `pdfree-ai/json_util.rs`: pulled the "find the first balanced
        `{...}` in a model response" helper (local models wrap JSON in
        prose/code-fences despite being asked not to) out of `formfill.rs`
        into a shared module, since `schema_extract.rs` needed the exact
        same logic — the second real consumer is what justified the
        extraction, not it being duplicated speculatively ahead of time.
  - [x] FFI: `pdfree-ffi` exports `ai_extract_schema` (takes
        `AiProviderConfig`, same local/cloud choice as the other LLM-backed
        Phase 5/6 functions; new `SchemaField`/`ExtractedValue` records) and
        `diff_documents` (no provider param — matches `ai_detect_pii`/
        `ai_redact`/`ai_extract_tables`'s precedent of not pretending a
        fully local function needs one; new `ChangeKind`/`TextChange`/
        `PageDiff` records). Full workspace build/test/fmt/clippy clean;
        Swift bindings regenerated (`aiExtractSchema`, `diffDocuments`,
        confirmed present with the expected signatures) and the macOS app
        target still builds against them. **Not wired into any UI yet** —
        same unscoped-follow-up pattern as Phase 6's FFI-before-UI landing.
- [ ] Agentic document workflows (lopi); confidence scoring + review routing.
      **Also 3 distinct pieces**, only one of which is actually blocked:
  - [ ] **Confidence scoring** — buildable now, no lopi dependency at all.
        Add a confidence signal to the suggestion-shaped outputs this crate
        already returns (`formfill::SuggestedFill`, `schema_extract::
        ExtractedValue`, `classify`'s label). Researched whether Anthropic's
        API exposes token-level log-probabilities the way some other
        providers do (which would give a cheap, principled confidence
        number) — it does not, for the kind of completion calls this crate
        makes, so that's not an available signal here. **Recommendation:
        a grounding check as the primary signal** — does the model's
        returned value string actually appear verbatim in the document's
        own extracted text? Free (no extra model call), fully deterministic,
        and answers the question a caller actually cares about (did the
        model invent this, or is it really in the document) — a natural
        extension of the same "never surface a hallucinated field name"
        defense already in `formfill.rs`/`schema_extract.rs`, just applied
        to values instead of field names. Format validation where
        applicable (date/email/phone shape) as a secondary signal, same
        spirit as `redact.rs`'s Luhn check narrowing its own false-positive
        rate.
  - [ ] **Review routing** — a thin policy layer once confidence exists:
        given a list of `(item, confidence)`, bucket into auto-accept /
        needs-review / reject by threshold. Genuinely just a function, not
        a model call — a small `pdfree-ai/review.rs`. The actual review
        *queue UI* is shell-side, same shape as the existing FormsPanel/
        AIPanel suggestion-review flows already built this session.
  - [ ] **Agentic workflows (lopi)** — still genuinely blocked: lopi's
        actual interface isn't in this workspace, so a real integration
        can't be written without either its source or an API spec. What
        research surfaced as the better answer than a bespoke "lopi tool
        manifest," though: **the Model Context Protocol (MCP) has a mature
        official Rust SDK** (`rmcp`, from [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk),
        async/tokio-based, with `#[tool]`/`#[tool_router]` proc macros for
        exposing typed Rust functions as schema-described tools an LLM can
        call). Exposing `pdfree-core`/`pdfree-ai`'s existing operations
        (merge, split, fill, sign, classify, `schema_extract`, redact,
        table extraction, diff...) as an MCP server, rather than a
        bespoke manifest custom-built for lopi, means **any** MCP-speaking
        agent — lopi (if/when it speaks MCP), Claude Code itself, Claude
        Desktop, any third-party agent — can drive multi-step PDFree
        workflows ("extract vendor/total/date from these 10 invoices into
        a CSV"), not just lopi specifically. This is the concrete, buildable
        piece of "agentic workflows" — a new `pdfree-mcp` crate wrapping
        the existing engine functions as `#[tool]`s — while the actual
        lopi-specific wiring stays deferred pending its interface, exactly
        as already documented above.

### Candidate features surfaced during research (not yet scheduled)

Found while researching Phase 7 and, separately, "what makes the best
possible PDF-tool experience" (2026-07-03) — genuinely new ideas, not part
of any committed phase. Flagged with feasibility confidence so a future
scoping pass doesn't start from zero. Kept deliberately short: a handful of
well-reasoned candidates, not a brainstorm dump.

- **Bates numbering / stamping** — sequential page-numbering stamps
  ("ABC-000001") across one or many documents, a routine legal/discovery
  workflow. **High feasibility** — this is just `forms::overlay_text` called
  in a loop with a computed string per page; no new engine capability
  needed, closer to a `pages.rs` convenience function than a new module.
- **Multi-reviewer annotation diff/merge** — compare or merge the
  highlight/underline/note annotations two people added to their own copy
  of the same source document (distinct from `diff.rs`'s *content* diff,
  which is text, not annotations). **Medium feasibility, well-positioned**
  — `annotations::list` already reads every annotation back with its
  position and kind; the new work is a position-based matching step (did
  reviewer A and B annotate the same span?) rather than anything PDFium
  doesn't already expose. Natural pairing with `diff.rs` once both exist.
- **PDF encryption / permissions** (password-protect on export, restrict
  print/copy/edit) — **feasibility unconfirmed, needs a source check before
  scoping**: researched `pdfium-render`'s docs/README and confirmed it can
  *open* password-protected PDFs, but found no documented API for *creating*
  an encrypted PDF or setting permission flags on export. Don't schedule
  this without first reading `pdfium-render`'s source (or `PDFium`'s own C
  API) to confirm the capability actually exists — this is exactly the kind
  of claim this doc's own convention says to verify, not assume, before
  committing to a phase.
- **PDF/A archival export** — a compliance/differentiation angle (many free
  tools don't offer this). **Feasibility unconfirmed** — same caveat as
  encryption above: PDF/A conformance is a validation + constrained-subset-
  of-features problem (embedded fonts, no encryption, specific metadata),
  and nothing in this research confirmed `pdfium-render` has native support
  for producing or validating it. Would need real investigation (possibly
  a separate validation dependency) before this becomes a scoped phase item,
  not just an aspiration.
- **In-document search (Cmd/Ctrl+F)** — **currently entirely absent from
  every shell**, and this is a bigger gap than it might look: researched
  what power users actually expect from a PDF viewer's keyboard shortcuts,
  and search is the single most universal one cited (alongside page-jump
  and page-navigation, both of which PDFree already has). **High
  feasibility, high priority** — the engine primitives already exist
  (`editor::text_runs` gives every run's text + page + bounds); a new
  `pdfree_core::search` (or a shell-side pure function over `text_runs`)
  that matches a query against run text and returns page+bounds for
  jump-to and on-page highlight is close to a pure aggregation over data
  this crate already produces, not a new capability. Worth treating as
  close to a v1 gap, not a v2 nice-to-have — every competitor has this.
- **Bookmarks / outline navigation panel** — reading a PDF's existing table-
  of-contents/bookmark tree for a jump-to-section sidebar. **High
  feasibility** — `pdfium-render` has bound `FPDFBookmark_*()` into a
  `PdfBookmarks` collection in its high-level API since 0.5.3, confirmed via
  the crate's own changelog; this is a read-only wrapper over an API that
  already exists, not new engine work.
- **Undo/redo** — **currently entirely absent from every shell**: there is
  no way to undo a mistaken fill/sign/edit anywhere in PDFree today short of
  manually reversing it, which cuts against Core UX Principle #7
  ("reliability over polish" — an app that can't undo a mistake doesn't
  feel safe to edit in, regardless of how solid the core operations are).
  Researched document-editor undo/redo architecture generally (rope data
  structures, operation logs) — concluded that class of design **doesn't
  fit this engine's actual mutation model** and would be over-engineering:
  every `pdfree-core` mutation already takes whole-document bytes in and
  returns whole-document bytes out (confirmed by every function signature
  in `forms.rs`/`pages.rs`/`annotations.rs`/etc.), so the natural fit is a
  **bounded-depth stack of whole-document byte snapshots** on the shell
  side (e.g. the last 20 states) — simple, matches the existing engine
  shape exactly, and realistic document sizes (a few MB) make even 20
  snapshots a non-issue in memory. A rope/operation-log design would only
  make sense if the engine mutated incrementally in place, which it
  deliberately doesn't.
- **PDF accessibility (tagged reading order, form field tab order/tooltips,
  screen-reader support)** — researched what actually makes a PDF
  accessible: correct structure tags (headings/paragraphs/tables) for
  reading order, a defined tab order across form fields, and descriptive
  tooltip text per field for screen readers to announce. **Real, current
  gap**: nothing in `pdfree-core` today reads or writes structure tags,
  and `forms::fill`/`forms::overlay_text` don't touch tab order or field
  descriptions. **Feasibility unconfirmed** — confirmed `pdfium-render`
  binds bookmarks and form fields into its high-level API, but found no
  documented evidence it exposes PDFium's structure-tree
  (`FPDF_StructTree_*`) functions at all, at any level. Same discipline as
  the encryption/PDF-A items above: this needs a source-level check
  against `pdfium-render`'s actual bindings before it's scoped as a real
  phase item — accessibility is exactly the kind of feature where a
  half-working implementation (tags claimed but not actually read by real
  assistive tech) is worse than being honest that it isn't there yet.
- **macOS Quick Look extension** — sign/annotate/fill a PDF directly from a
  Finder/Spotlight preview, without opening the full app. **Re-scoped after
  a 2026-07-16 deeper pass (this bullet's original "feasibility confirmed"
  overclaimed what's actually possible — corrected here rather than left
  standing)**: `QLPreviewingController`/`QLPreviewProvider`, the current
  (post-`.qlgenerator`, macOS 15+) Quick Look App Extension API, is a
  **preview-rendering surface only** — it has no supported way to add
  custom interactive controls (a "Sign here" button, an annotate toolbar)
  to the Quick Look panel, and no write-back mechanism to modify the
  previewed file at all. The Markup button users already see in Quick
  Look's PDF preview is Apple's own system feature (via Preview.app
  integration), not something exposed to third-party extensions — a
  third-party PDF app cannot add its own equivalent. It's also sandboxed
  down to read-only access on just the one previewed file (confirmed via
  an Apple Developer Forums thread on this exact restriction). So the
  realistic version of this feature is **preview-only**: a custom Quick
  Look extension that renders a nicer/more-accurate preview than the
  system default (using `pdfree-core` directly, no PDFium-in-Finder
  weirdness) — genuinely buildable, but "sign/annotate/fill directly from
  Finder" as originally envisioned is not achievable through this API;
  that flow would still need to hand off to the full app. Given the
  preview-only ceiling, this is now a lower-priority nice-to-have rather
  than the "zero friction" differentiator it was first pitched as — worth
  revisiting only if Apple's Quick Look extension API ever grows
  interactive/write capabilities.
- **Platform-native digital signature signing** (macOS/iOS Keychain
  `SecIdentity` + `CMSEncoder`, instead of PDFree building its own crypto/
  PKI stack) — a possible different path into the deferred PKCS#12
  crypto-signing item from Phase 2, and the "signature legal validity" open
  question above. The general PAdES signing shape (reserve a `/ByteRange`
  placeholder in the PDF, compute a detached CMS/PKCS#7 signature over
  those bytes, write it back into the reserved space) is confirmed and
  standard practice — what's genuinely novel here is offloading the *crypto*
  half to the OS's own Keychain-backed identity/Security framework rather
  than pulling in (or writing) a general-purpose crypto stack, so PDFree
  only has to build the PDF-specific byte-range wrapper. **Feasibility only
  partially confirmed** — found the standard PAdES process, but nothing
  confirming `CMSEncoder` specifically produces a signature in the exact
  form a PDF signature dictionary needs; this needs a real prototype spike
  (sign one byte range, verify a PDF viewer accepts it) before treating it
  as a real alternative to the "pick a crypto/PKI stack" plan already
  flagged as an open question — and it would only ever cover macOS/iOS,
  leaving Windows/Linux/web needing their own answer regardless.
- **Image-based (scanned) PII redaction** — `redact.rs` today only redacts
  PII found in real PDF *text objects*; a scanned page (PII visible only as
  pixels in a raster image) isn't covered at all. Researched how real
  redaction tools handle this: OCR (already have `ocr.rs`) locates the PII
  within the image, then the pixels are **physically replaced** ("pixel-
  burn"), not just covered by a drawn box — covering alone is recoverable
  with basic image editing since the underlying pixel data survives
  untouched. **Important gotcha surfaced by this research, worth flagging
  now even before scoping the feature**: a scanned-and-OCR'd PDF commonly
  keeps the original scanned image *and* stores the recognized text as an
  invisible text layer behind it — redacting only the visible image pixels
  while leaving that invisible OCR text layer intact would silently leave
  the PII copy-pasteable, the exact same class of mistake `redact.rs`'s
  existing text-object approach was designed to avoid. Any image-redaction
  work needs to redact both layers, not just the visible one.
- **Print support** — **currently entirely absent from every shell**, and a
  bigger surprise gap than it first looks, similar to in-document search.
  **High feasibility, low effort — especially on macOS**: confirmed
  `PDFKit`'s `PDFDocument.printOperation(for:scalingMode:autoRotate:)` gives
  a complete native print flow (print panel, pagination, scaling) in a
  handful of lines around AppKit's `NSPrintOperation` — SwiftUI itself has
  no native print API, so this has to drop to AppKit either way, but the
  PDFKit-specific entry point means PDFree doesn't need to hand-roll paging.
  Web gets this close to free too (the browser's own print dialog against
  the rendered PDF). Worth treating as close to a v1 gap given how little
  work it is relative to how universally expected it is.
- **Opt-in signature sync across a user's own Apple devices** (CloudKit) —
  saved signatures currently live in a local, per-device app-support
  directory (`~/Library/Application Support/PDFree/signatures/`, macOS
  only) — draw a signature on the Mac, and it isn't there on an iPhone.
  Researched CloudKit's private database as the sync mechanism: it's
  scoped to the user's own iCloud account, invisible to PDFree's own
  developers, and supports an offline-first/local-first model (edit
  offline, sync opportunistically) — genuinely compatible with "local-
  first, cloud-optional, no silent uploads" as long as it's an explicit
  opt-in, not a default. **Real scope boundary, not a small caveat**: this
  is an Apple-only mechanism — Windows/Linux/web would need a different
  answer (or none at all, staying single-device there), so this can't be
  "the" cross-platform sync story, only the Apple-platform one.
- **Watch-folder / batch automation** — drop PDFs into a folder, have
  PDFree automatically apply a configured operation (classify, redact,
  extract-schema, merge) to each one — a natural pairing with the MCP-
  server idea above (same underlying engine calls, different trigger:
  a file landing in a folder instead of an agent's tool call). Researched
  the watching mechanism: the `notify` crate ([notify-rs/notify](https://github.com/notify-rs/notify))
  is the mature, standard cross-platform choice — its `RecommendedWatcher`
  already selects the right native backend per platform (FSEvents on
  macOS, inotify on Linux, `ReadDirectoryChangesW` on Windows), with a
  debouncing companion crate for collapsing the noisy burst of events a
  single file save can generate. **High feasibility** — the watching layer
  is a solved problem; the actual work is wiring configured actions to
  the existing `pdfree-core`/`pdfree-ai` calls, not inventing anything new
  at the file-system layer.
- **Apple Pencil / `PencilKit` for iPad** — `apps/ios/Sources/PDFree/
  ContentView.swift` today is a bare open-and-render stub (Phase 4);
  researched what would actually make an iPad build feel like a real PDF
  app rather than a viewer demo, and Apple's own `PencilKit` framework
  (`PKCanvasView`, `PKToolPicker`, pressure/tilt-aware ink, Scribble
  handwriting-to-text) is the obvious, well-documented answer for both
  signing and freehand annotation — markedly better than adapting the
  macOS mouse-driven draw-signature pad to a touch target. **Confirmed
  feasible** (mature first-party framework, not a research gap) and
  arguably the actual **differentiating** iOS feature, more than parity
  with macOS's own AppKit-flavored views ever would be.
- **Multi-language OCR — confirmed real bug, not just a candidate idea**:
  read `pdfree-ai/src/ocr.rs`'s actual `recognize()` implementation while
  researching this — it shells out to `tesseract <in> <out>` with **no
  `-l` language flag at all**, so it silently always uses whatever
  Tesseract's default-language data happens to be (`eng`, assuming that's
  the only trained-data file installed). Confirmed via Tesseract's own docs
  that multi-language recognition is just `-l eng+fra` (or similar) — this
  is a small, mechanical fix (accept a language parameter, pass it
  through), not a research question. Low priority only insofar as OCR
  itself isn't wired into any shell's UI yet (Phase 5 landed the engine +
  FFI only) — but worth fixing at the same time OCR gets its first UI, not
  after, since it's a silent-wrong-result bug for any non-English scan
  otherwise.
- **Continuous-scroll page view mode** — researched user sentiment on
  single-page vs. continuous-scroll PDF viewing and found a strong,
  consistent preference for continuous scroll, with single-page-per-view
  defaults a recurring complaint against Acrobat/other viewers. **This is
  flagged as a real tension with an existing decision, not a slam-dunk
  addition**: Core UX Principle #1 (above) mandates fit-to-page-on-load: a
  continuous-scroll mode where multiple full pages stack and scroll past
  each other is compatible with that principle (each page can still open
  at fit-to-page zoom), but a continuous mode where the *content itself*
  scrolls within a zoomed-in page is not what #1 describes at all. Needs
  Wes's actual input on what "continuous scroll" should mean here before
  scoping — recommend surfacing this explicitly rather than an engineer
  quietly picking an interpretation that contradicts an existing principle.
- **Tabs / multiple open documents + a real recents list** — **confirmed
  current gap**: every shell built so far is single-document (open one PDF,
  close it, open the next); there's no way to have two documents open at
  once, and no persistent recent-files list to reopen something quickly.
  For a "daily driver" tool this is a real gap, not a nicety. Researched
  the macOS-specific path: SwiftUI's `DocumentGroup` scene gives windowed
  multi-document support close to free, and macOS 26's tab-bar merging
  means those windows present as tabs automatically — but research also
  surfaced a real caveat worth flagging before assuming this is trivial:
  SwiftUI's `FileDocument`/`ReferenceFileDocument` protocols are
  considered "underpowered... compared to AppKit's `NSDocument`" 
  specifically for recent-files/document-URL bookkeeping, so a *good*
  recents list may need dropping to AppKit sooner than a first pass would
  assume — same shape of SwiftUI-limitation-discovered-by-reading-real-
  source as this session's earlier `AnnotationColor`/`Color` naming
  collision. Web/Tauri get multi-document more cheaply (browser tabs /
  multiple windows are already free); only the native shells need real work.

  **2026-07-16 scoping pass (this bullet was flagged for scoping, not
  implementation — findings below, no code changed):**
  - **Correction to the note above**: automatic window→tab merging (View >
    Show Tab Bar, `NSWindow.allowsAutomaticWindowTabbing`) is a general
    AppKit window-management feature that's been available for years, not
    something specific to macOS 26 or to `DocumentGroup` — it applies to
    any multi-window macOS app (`WindowGroup` or `DocumentGroup` alike).
    So multi-window support gets tab-merging "for free" regardless of
    which scene type is chosen, and regardless of macOS version within any
    reasonably current range — one less version-gating concern than
    previously written here.
  - **The real, newly-identified architectural blocker**: `PDFDocumentStore`
    today owns a `private let ffiQueue = DispatchQueue(...)` as an
    *instance* property, and `pages.rs`'s own doc comment already
    establishes the hard invariant this exists to protect — "never call
    `pdfium::bind()` twice within one call chain... two live `PDFium`
    bindings in the same process hangs" (confirmed empirically, see the
    Phase 3 entry above). A single-window app satisfies that invariant
    trivially (one store, one serial queue, only ever one call in flight).
    Multiple windows each holding their *own* `PDFDocumentStore` — the
    natural first design for "each document tab has its own state" —
    would create multiple independent serial queues that can each dispatch
    a `PDFium`-touching call at the same time as another window's queue,
    which is exactly the two-live-bindings hang condition. **This must be
    fixed before multi-document is safe to build, not discovered after**:
    the fix is straightforward (promote `ffiQueue` from a per-store
    instance property to one shared/static queue every `PDFDocumentStore`
    instance dispatches onto), but it's a real prerequisite step, not
    incidental to the DocumentGroup/tab-UI work itself.
  - **Effort shape, given the above**: (1) shared static FFI queue fix
    (small, isolated); (2) `DocumentGroup` + `FileDocument`/
    `ReferenceFileDocument` adoption, replacing `PDFreeApp`'s current single
    `WindowGroup { ContentView() }` with one document type wrapping the
    existing `PDFDocumentStore`; (3) migrate or bridge the existing
    UserDefaults-backed `recentFiles` list against whatever `DocumentGroup`
    provides natively, per the already-flagged `NSDocument`-superiority
    caveat — needs a small prototype to decide before committing, not a
    guess. Medium-sized, multi-step effort; no single piece is huge, but
    it touches the app's window-management architecture, not just a new
    view. Web/Tauri still get multi-document more cheaply (browser tabs /
    multiple windows are already free); this write-up is macOS-specific.

## Claude Code Instructions

When continuing from this document, Claude Code should:

1. **Phase 0 is done** — build/test with `scripts/build-all.sh` (fetches PDFium first).
2. **Keep pdfree-core pure Rust** — no platform-specific code in core; use feature flags if needed.
3. **Keep pdfree-ai provider-agnostic** — never hardcode a single LLM; local-first, cloud-opt-in.
4. **Test with real-world PDFs** — IRS forms, contracts, scanned docs; not just synthetic files.
5. **PDFium bundling** — documented in `docs/pdfium-bundling.md`; binaries are fetched, never committed.
6. **No watermarks, no limits, no silent uploads** — any usage-tracking, limit, or hidden-upload code is a non-starter; it's the whole reason PDFree exists.
7. **License**: BUSL-1.1 (matching Squish).

---

## Open Questions for Wes to Decide
- [x] **Project name**: PDFree ✅
- [x] **PDFium bundling**: prebuilt binaries from `bblanchon/pdfium-binaries`, fetched at build time, loaded dynamically at runtime. ✅ (see `docs/pdfium-bundling.md`)
- [x] **Monetization / License**: BUSL-1.1 like Squish. ✅
- [ ] **Local AI default model**: which quantized model ships as the on-device default? (Squish-served; pick per RAM budget)
- [ ] **Cloud AI providers at launch**: Claude only, or Claude + GPT + Gemini via provider layer?
- [ ] **Signature legal validity**: v1 = basic e-sign only, or pursue ESIGN/eIDAS from day one?
      **Recommended default** (not yet confirmed by Wes): basic e-sign + free audit
      metadata (signer name, timestamp, IP) ships in v1 as core; legal-grade
      eIDAS/ESIGN certification becomes the later paid tier (see Potential Paid
      Features above). **New research angle for the separate, deferred PKCS#12
      crypto-signing item** (Phase 2) — see the candidate-features list below
      for a possible lower-risk path via platform-native signing (macOS/iOS
      Keychain) instead of PDFree building its own crypto/PKI stack.
- [ ] **iOS priority**: ship macOS + web first, then iOS in v1.1?
- [ ] **Domain**: pdfree.app? pdfree.io? getpdfree.com? (check availability)
- [ ] **AI as a paid tier?**: is cloud AI (real per-call cost) the one optional paid add-on?
- [ ] **PDF ↔ DOCX conversion strategy**: `convert.rs::to_docx`/`from_docx` are
      `NotImplemented` (Phase 3). `PDFium` has no DOCX support at all — this
      needs either (a) a Rust document-layout-reconstruction crate, (b)
      shelling out to a conversion service/binary (e.g. LibreOffice
      headless) at the cost of the "no cloud, no dependencies we don't
      control" pitch, or (c) a deliberately lower-fidelity "extract text +
      basic structure" export instead of true layout-preserving conversion.
      Worth deciding before Phase 4 platform shells commit to a UI for it.
      **New research finding for option (c)'s output half**: see the
      candidate-features list below — pure-Rust `genpdf`/`printpdf` give a
      concrete, no-C-toolchain way to *generate* a PDF from extracted
      text/structure; doesn't solve DOCX *parsing*, just de-risks the half
      of option (c) that was previously unaddressed.
