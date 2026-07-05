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
added alongside that UI work), `search.rs` + `bookmarks.rs` (Phase 4
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
- [ ] `forms.rs`: `FormField` needs `page: u16` and `rect: (f32, f32, f32, f32)`
      added — currently only exposes `{name, kind, value}`, which is enough to
      know a field exists but not where to draw it. **Blocks Phase 4**: no
      shell can build the auto-detect-and-overlay-boxes UX (Core UX Principles,
      above) without per-field page + bounding rect from the engine.
- [ ] `forms.rs`: `fill()` needs deterministic font-size-fit-once logic —
      currently there is zero font-size handling in `fill()` (no `/DA`
      reading, no shrink logic); sizing is left entirely to PDFium's internal
      form-render behavior, which is the likely source of the "text
      resizes/gets cut off on export" problem. Needs to compute a size once
      from the field rect + text at fill time and bake it in, so it never
      changes between the fill view and the exported PDF.

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
  - [ ] Auto-run `boxes_on_page` + AcroForm field scan on load for *every*
        page up front (already partially done — extend so no field is left
        for manual double-click placement as the primary flow). Depends on
        the Phase 1 `FormField.page`/`rect` gap below being closed first.
  - [ ] Detect signature/initials fields by label pattern and route them to
        the sign flow (draw/type/upload/reuse-saved) instead of a text field
  - [ ] Confirm overlay/AcroForm text fill is shrink-to-fit at edit time with
        no export-time re-sizing or clipping (audit `forms.rs::fill` /
        `overlay_text` against this — currently unclear if export size can
        drift from the live-edit render)
  - [ ] Add persistent "+" quick-action (import/merge/split/add page) in the
        main canvas UI, not just File menu
  - [ ] Trim default toolbar to: open, fill, sign, add/remove page, export
  - [ ] Add saved-signature reuse (store drawn/typed/uploaded signature,
        insert on later documents without redrawing)
  - [ ] Lightweight local audit metadata on sign (timestamp, signer name,
        device info where available) — not the deferred certified/legal-grade
        trail, see Potential Paid Features above
  - [ ] End-to-end tests: drag-and-drop import, file-picker import, full
        fill→sign→export path against real-world fixtures
  - [ ] Page thumbnail sidebar (all shells)
- [x] **Engine-side quick wins from the 2026-07-03 feature research pass**
      (see the two linked feature-status/research artifacts) — the 4 of 9
      identified "quick win" features buildable and testable without a
      macOS/Xcode toolchain or network access to fetch model/OCR binaries:
      `search::find_text` (in-document "⌘F", reuses `editor::text_runs`),
      `bookmarks::outline` (document outline tree, wraps `pdfium-render`'s
      bookmark API), `pages::bates_number` (sequential legal/discovery
      stamping, reuses the `overlay_text` primitive), and
      `pdfree_ai::confidence::ground_check` (grounding/hallucination check
      for any future AI-produced value — no model call, no provider
      needed). All four are pure `pdfree-core`/`pdfree-ai`, fully unit
      tested (`tests/search.rs`, `tests/bookmarks.rs`, the `bates_*` tests
      in `tests/pages.rs`, `confidence::tests` inline) — see `docs/api.md`
      and `docs/ai-design.md`. **Not done in this pass**: shell wiring
      (search UI, an outline sidebar, a Bates-numbering dialog) — that
      needs a macOS/Xcode session to build and verify, which this session
      didn't have. The other 5 quick wins from the research report (print,
      undo/redo, grammar rewrite, multi-language OCR, watch-folder
      automation) were deliberately deferred: print/OCR need platform
      tooling (PDFKit, tesseract) unavailable here, undo/redo is mostly
      Swift-side shell state, and grammar rewrite needs a live cloud
      provider decision still open below.
- [ ] Web app (React + WASM) with full toolbar
- [ ] Tauri desktop app for Windows/Linux (reuse web UI)
- [ ] iOS app (shared SwiftUI views from macOS)

### Phase 5 — AI Tier 1 (Core AI)
- [x] `pdfree-ai/confidence.rs`: grounding/hallucination check (`ground_check`)
      pulled forward from Phase 7 — needs no provider, so it shipped early;
      see the Phase 4 quick-wins entry above and `docs/ai-design.md`
- [ ] `pdfree-ai/provider.rs`: local + cloud provider abstraction (trait scaffolded)
- [ ] `ocr.rs`: Tesseract + LLM cleanup; Apple Vision on macOS/iOS
- [ ] `rag.rs`: chunk → embed → retrieve; wire Kyro/Kohaku/Vectro
- [ ] Document Q&A chat UI; auto-summary; smart form fill from profile

### Phase 6 — AI Tier 2 (Differentiators)
- [ ] `redact.rs`: PII detection + one-click redaction
- [ ] `extract.rs`: contract analysis; table extraction to CSV/Excel/JSON
- [ ] `classify.rs`: auto-classify + library organization; semantic search

### Phase 7 — AI Tier 3 (v2+ expansion)
- [ ] Layout-aware translation, editing; voice-to-fill; grammar/tone rewrite
- [ ] Schema-driven extraction; document diff/redline
- [ ] Agentic document workflows (lopi); review routing
      (confidence scoring itself shipped early — see Phase 5 above)

---

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
      Features above).
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
