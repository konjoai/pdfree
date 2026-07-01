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
`signatures.rs` + `annotations.rs` (Phase 2), `editor.rs` + `pages.rs` +
`convert.rs` (Phase 3). Later-phase modules exist as scaffolds returning
`PdfError::NotImplemented`.

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

### Phase 1 — Core Read + Fill ✅ DONE (with one documented gap)
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

### Phase 2 — Sign + Annotate
- [ ] `signatures.rs`: place signature image at coordinates
- [ ] `signatures.rs`: digital certificate signing (PKCS#12)
- [ ] `annotations.rs`: highlight, underline, strikethrough, sticky notes
- [ ] Web: `SignaturePad.tsx` using canvas → PNG → core

### Phase 3 — Edit + Merge/Split + Convert
- [ ] `editor.rs`: detect font of clicked text, replace in-place
- [ ] `pages.rs`: merge N PDFs; split by range; rotate/extract/reorder
- [ ] `convert.rs`: PDF → DOCX; DOCX/image → PDF

### Phase 4 — Platform Shells
- [ ] Wire UniFFI codegen for `pdfree-ffi` (UDL already frozen)
- [ ] macOS SwiftUI app wrapping pdfree-ffi via UniFFI
- [ ] Web app (React + WASM) with full toolbar
- [ ] Tauri desktop app for Windows/Linux (reuse web UI)
- [ ] iOS app (shared SwiftUI views from macOS)

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
- [ ] **iOS priority**: ship macOS + web first, then iOS in v1.1?
- [ ] **Domain**: pdfree.app? pdfree.io? getpdfree.com? (check availability)
- [ ] **AI as a paid tier?**: is cloud AI (real per-call cost) the one optional paid add-on?
