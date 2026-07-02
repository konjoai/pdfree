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
PDF↔DOCX deferred, see Phase 3 below). No further scaffold modules remain —
Phase 4 is platform shells, Phases 5–7 add `pdfree-ai`.

---

## v1 Feature Spec

### Must-Have (v1.0)

| Feature | Engine Layer | Notes |
|---|---|---|
| Open + render PDF | `pdfree-core/renderer.rs` | Via PDFium; smooth scroll; **default view fits the entire page in the viewport** — never opens pre-zoomed in |
| Auto-detect + overlay all fields | `pdfree-core/forms.rs` | On document load, scan every page and draw an interactive box for every AcroForm field automatically — user never manually places a text box on a document that already has real fields |
| Fill AcroForms | `pdfree-core/forms.rs` | Text fields, checkboxes, dropdowns; font size is computed once at fill time and baked in — never re-shrinks or truncates differently on export |
| Fill non-interactive PDFs | `pdfree-core/forms.rs` | Overlay text boxes |
| Sign documents | `pdfree-core/signatures.rs` | Draw, type, or image upload; Signature/Initials fields are auto-detected and open the signer UI directly, never a plain text cursor; unlimited signatures, free forever |
| Signature audit trail | `pdfree-core/signatures.rs` | Signer name, timestamp, IP address captured and embedded by default on every signature — core/free, not gated (see Premium Features for what *is* gated) |
| Edit existing text | `pdfree-core/editor.rs` | Font detection + matching |
| Annotate (highlight, underline, notes) | `pdfree-core/annotations.rs` | Standard PDF annotations |
| Merge PDFs | `pdfree-core/pages.rs` | N files → 1; reachable via a single persistent "+" entry point, not a file menu |
| Split PDFs | `pdfree-core/pages.rs` | By page range or bookmarks |
| Convert to/from formats | `pdfree-core/convert.rs` | PDF↔Word/image/text |
| Save / export | `pdfree-core/document.rs` | Preserve original layout |

### Out of scope for v1
- Real-time collaboration
- Cloud storage sync
- Legally binding, certified e-signature workflow (eIDAS/ESIGN-grade certification,
  tamper-evident hash chains, multi-party signer routing). Note: **basic** signature
  audit metadata (signer name, timestamp, IP) is in scope for v1 — see the table above
  and Premium Features below for the exact line between the two.

---

## Core UX Principles

These are durable product doctrine, not just v1 checklist items — every platform
shell (macOS, web, Tauri, iOS) must follow them:

- **Default view = fit-to-page, always.** Never open a document zoomed past what fits
  the viewport, regardless of screen size or window size. Recompute on resize.
- **Auto-detect, never manual-box.** On document load, scan every page for AcroForm
  fields and draw interactive boxes automatically. The user should never need to
  manually place a text box on a document that already has real fields — that's an
  engine failure, not a user task.
- **Signature/initials are first-class, not generic text fields.** A detected
  `Signature`-kind field (or a text field name-matched as "initials") opens a
  draw/type/upload signer UI directly — never a plain text cursor.
- **What you fill is what you export.** Font size is computed once at fill time and
  baked in — it must never shrink, re-wrap, or truncate differently between the fill
  view and the exported PDF.
- **Unlimited, audited signing, free forever.** No cap on number of signatures. Every
  signature capture includes signer name, timestamp, and IP address by default — no
  paywall on basic audit metadata (see Premium Features for what *is* reserved).
- **One add entry point, not a file menu.** Merging, inserting, and adding pages all
  go through a single persistent "+" affordance (opens a document picker /
  drag-drop target) — never buried behind File → Import.
- **Minimal toolbar.** The primary toolbar is exactly: Open, Fill, Sign, Add/Remove/
  Reorder pages (via +), Merge, Annotate (highlight/underline/note), Export. Anything
  else (AI features, advanced tools) lives in a secondary panel, never crowds the
  primary bar.

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

## Premium / Potential Paid Features

Separate from the AI Tier 2/3 roadmap (Phases 6–7) — these are non-AI ideas
surfaced during core UX planning that are explicitly **not** free/core:

- **Advanced signature audit trail**: tamper-evident hash chains, multi-party
  signer routing, legal-grade eIDAS/ESIGN-certified signing. Ties directly to the
  "Signature legal validity" open question below. Basic audit metadata
  (signer name, timestamp, IP) stays free/core regardless of how that's decided.
- **"Quick redact" field overwrite**: overwrite an existing field's contents with
  whitespace to correct a mistake without full redaction tooling. Explicitly
  parked, not needed right now — revisit as a paid-tier candidate later.

Everything else discussed in core UX planning — auto field detection, unlimited
signing, draw-signature, basic audit metadata, merge/split/reorder, fit-to-page
default, minimal toolbar — is explicitly core and free. This list exists to keep
that line clear, not to grow it casually.

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
- [ ] `renderer.rs`: add a `fit_to_page()` helper (pure math: page size in
      points + viewport size in pixels → `RenderOptions`/DPI) so every shell
      computes the default fit-to-page zoom identically instead of each
      platform back-computing it separately
- [ ] Wire UniFFI codegen for `pdfree-ffi` (UDL already frozen)
- [ ] macOS SwiftUI app wrapping pdfree-ffi via UniFFI
- [ ] Web app (React + WASM) with full toolbar
- [ ] Tauri desktop app for Windows/Linux (reuse web UI)
- [ ] iOS app (shared SwiftUI views from macOS)
- [ ] Page thumbnail sidebar (all shells)
- [ ] Auto field-overlay rendering on document load (Core UX Principles, above)
      — depends on the Phase 1 `FormField.page`/`rect` gap being closed first
- [ ] Single "+" add/merge/insert entry point (document picker + drag-drop),
      replacing any file-menu-driven import flow
- [ ] Minimal primary toolbar per Core UX Principles (Open, Fill, Sign,
      Add/Remove/Reorder pages, Merge, Annotate, Export only)
- [ ] `SignaturePad.tsx` (canvas → PNG → core, listed under Phase 2) plus a
      "saved signatures" store so a signer can reuse a prior signature/initials
      without redrawing every time

### Phase 5 — AI Tier 1 (Core AI)
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
- [ ] Agentic document workflows (lopi); confidence scoring + review routing

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
      eIDAS/ESIGN certification becomes the later paid tier (see Premium Features).
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
