# Handoff: PDFree macOS SwiftUI redesign

## Overview
A visual + interaction redesign of the PDFree macOS app: a calm, dark,
distinctly-branded shell built around a **right-hand inspector** instead of a
crowded top toolbar. Covers the default document view, the auto-detected field
overlay (normal vs. signature fields), the sign flow (first-time sheet + a
returning-user popover that hops field-to-field), the persistent "+" add/merge/
split action, the empty/open state, and the PDFree logo.

The engine (`pdfree-core` + `pdfree-ffi`) is complete and unchanged. This is a
**SwiftUI-only** task in `apps/macos`.

## About the design files
`designs/` contains **HTML design references** — prototypes that show the
intended look and behavior. They are *not* production code to copy. Open
`designs/pdFree UI Options.dc.html` in a browser to view them (it loads
`support.js` + `FormPage.dc.html` from the same folder). The task is to
**recreate these designs natively in the existing SwiftUI/AppKit app**, reusing
its patterns (`PDFDocumentStore`, the `Views/` sheets, `PageCanvasView`).

The prototype is organized as a scrollable options canvas. The **confirmed
direction** to implement is:
- Turn 5 (`#5a`, `#5b`, `#5c`) — the assembled layout, empty state, working
  state, and sign flow.
- Field overlay = the "quiet" style (`#2a`) for normal fields + the amber
  **"Sign here" button** (`#2b`) for signature fields.
- Logo = green tile / green document mark (`#7a`/`#7c`) + the `pd·free`
  green-pill wordmark (`#6d`). The empty state and titlebar use the wordmark;
  the drop zone uses the green-document mark.
Turns 1–4 and 6–8 are earlier exploration kept for context — do not implement
them separately.

## Fidelity
**High-fidelity.** Final colors, typography, spacing, radii, and interactions.
Recreate pixel-close using the tokens below.

## Target files (what to add / change)
Existing (in `apps/macos/Sources/PDFree/`):
- `ContentView.swift` — the layout shell + toolbar (to be replaced with the 3-column inspector layout)
- `Models/PDFDocumentStore.swift` — the FFI-backed document store (add signature-field + saved-signature state)
- `Models/Tool.swift` — canvas interaction modes (reused; surfaced via the inspector)
- `Views/PageCanvasView.swift` — the page + field overlays + inline edit
- `Views/PagesSidebarView.swift` — left thumbnail rail
- `Views/SignatureSheet.swift` — signing (extend to tabs + reuse)
- `Views/FormsPanel.swift`, `SplitSheet.swift`, `TextPromptSheet.swift`, `ExtractedTextSheet.swift` — existing sheets (restyle to tokens)

New files to add:
- `Theme.swift` — design tokens
- `Views/EmptyStateView.swift` — the drop-surface empty state
- `Views/InspectorView.swift` — the right inspector
- `Views/AddMenuPopover.swift` — the "+" popover menu
- `Views/SignPopover.swift` — the returning-user inline sign popover
- `Models/SavedSignature.swift` — persisted signature/initials model

---

## Screens / Views

### 1. Document view — default (working state) · ref `#5b`, `#1a`
**Purpose:** the main editing surface after a PDF is open.
**Layout:** `HSplitView`, left→right:
- **Thumbnail rail** — width ~132pt. Bg `#1a1815`, 1px right hairline
  `rgba(255,255,255,.06)`. Centered `PAGES` label (10pt, 600, uppercase,
  tracking ~1px, `#6f6860`). Page thumbnails ~88×114pt white, 3pt radius; the
  current page has a 2px green ring `#37c07a` + shadow; page number below
  (10pt, current = `#e8e2d8` 600, others `#8f887d` 400).
- **Canvas** — flexible width, min 520pt. Radial-gradient bg from `#161311`
  (center-top) to `#0f0d0b`. Page centered with `padding: 30`, drop shadow
  `0 18px 50px rgba(0,0,0,.55)`, 3pt radius. **Always fit-to-page** (see store).
  - Top-left overlay chip: green-tinted pill `"● 199 fillable fields detected"`
    — bg `rgba(55,192,122,.14)`, 1px border `rgba(55,192,122,.4)`, text
    `#6fdca2`, 11pt 600, radius 999, material/blur behind.
  - Bottom-center floating bar: `‹  1 / 2  ›`  +  `Fit to page` — bg
    `rgba(26,24,21,.82)`, hairline border, radius 999, blur.
- **Inspector** — width ~274pt (see §4).

### 2. Auto-detected field overlay · ref `#2a` + signature from `#2b`
**Purpose:** show which areas are fillable, the moment the page loads.
Drawn on top of the page image in `PageCanvasView`, one overlay per
`store.detectedBoxes` entry (already scanned on load — do not add a toggle).
- **Normal field (quiet):** rounded rect (4pt radius) with 1.4px border
  `rgba(55,192,122,.6)` and fill `color-mix(green 8-12%, white)` — i.e. a very
  faint green wash on white. **Focused field:** 2px solid border `#37c07a`, fill
  `~10%`, focus ring `0 0 0 4px rgba(55,192,122,.18)`, text caret `#37c07a`.
  A small `auto-fit ✓` tag (9pt, `#37c07a`, `rgba(55,192,122,.12)` bg) signals
  text shrinks to fit.
- **Signature / initials field (distinct — never a text caret):** amber. In the
  page facsimile it's a filled **"Sign here" button**: bg `#e8b45a`, ink
  `#4a3200`, 6pt radius, pen glyph + `Sign here` (700). On the zoomed treatment
  (`#2a`) it's a 2px **dashed** amber border `#e8b45a`, fill
  `color-mix(#e8b45a 12%, white)`, a 26pt amber rounded pen badge, and the copy
  `Click to sign — draw, type, or reuse saved`. A `signature` tag sits top-right.
  **Detect signature fields** by field name matching `/sign|initial/i` and by
  AcroForm signature `FieldKind`; everything else is a normal fill box.
- **WYSIWYG:** text auto-shrinks to fit the box while typing; the on-screen
  render *is* the exported result (no separate export-time resize).

### 3. Sign flow · ref `#5c` (`#3a` sheet + `#3b` popover)
**Purpose:** sign/initial unlimited and free; reuse a saved mark without
redrawing.
- **First time (nothing saved) → full sheet** (`SignatureSheet`): title
  "Add your signature"; segmented tabs **Draw / Type / Upload / Saved**; a white
  draw pad (~150pt tall, dashed baseline, "Draw with trackpad or mouse"); a
  **"Save this signature for reuse"** toggle (green, on by default); footer
  `Clear · Cancel · Place signature` (Place = green primary). Type tab renders
  the typed name in a cursive face (`Snell Roundhand`/`Brush Script MT`); Upload
  accepts PNG/JPEG.
- **Returning (≥1 saved) → inline popover** (`SignPopover`) anchored to the
  clicked field: dark card (`#252220`, hairline, 14pt radius, big shadow) with a
  left-pointing arrow, a `SAVED SIGNATURES` label, tap-to-place chips (the drawn
  signature and typed initials on white cards), and `Draw new · Type · Upload`.
- **Hop animation:** after placing into a field, if more signature/initials
  fields remain, the popover **animates to the next one** (top/position, spring
  ~0.6s, cubic-bezier(.34,1.4,.5,1)) with a `n / total` progress pill. When all
  are done: a green check, "Everything's signed", "Saved locally · time & name
  recorded", and a "Run it again" reset. (The lightweight local audit = the
  timestamp + signer name only; no certified chain — matches v1 spec.)

### 4. Right inspector + the "+" action · ref `#5b`, `#4a`, `#4b`
**Purpose:** the whole minimal command surface, calm on the right.
Panel: width ~274pt, bg `#201d1a`, 1px left hairline, `padding: 18/16`, `gap 15`.
- **Top — "Add or merge"** button: 40pt tall, 10pt radius, 1px border
  `rgba(55,192,122,.55)`, tonal bg `color-mix(green 15%, panel)`, text `#7fe0aa`,
  plus glyph + label. Opens `AddMenuPopover` (ref `#4a`): rows **Open a PDF…**
  (with subtitle "Replace what's open"), **Merge another PDF…** ("Append to the
  end"), **Insert blank page**, **Image as a page…**, divider, **Split or extract
  pages…** ("Pick a range → new file"). Each row = 32pt rounded icon tile + title
  (13pt 600) + optional subtitle (10.5pt `#8f887d`).
- **`TOOLS`** group label (10pt 600 uppercase, `#6f6860`), rows (10-11pt padding,
  9pt radius, 13.5pt): **Fill fields** (active state = `color-mix(green 13%)` bg,
  text `#f3efe8`, icon `#37c07a`, right-aligned count badge `199` in
  `rgba(55,192,122,.22)`/`#7fe0aa`), **Sign** (amber `1` badge), **Annotate**.
  Idle rows: icon `#8f887d`, text `#d8d2c8`, hover `rgba(255,255,255,.05)`.
- **`PAGES`** group: **Insert page**, **Rotate**, **Delete page** (maps to
  `store.insertImagePage`/blank, `store.rotate`, `store.deletePage`).
- **Bottom (pinned):** **Export** — 44pt, 11pt radius, solid green `#37c07a`,
  ink `#08130c`, 14pt 650, shadow `0 6px 18px -6px rgba(55,192,122,.7)`, share
  glyph. Under it: `No watermark · no limits · saved locally` (10.5pt `#6f6860`).
- Tools/pages/export are dimmed (~45% opacity, non-interactive) until a document
  is open.

### 5. Empty / open state · ref `#5a`
**Purpose:** first launch / no document. The drop surface **is** the window
(do not auto-open `sample.pdf`).
- Center column ~520pt: `pd·free` wordmark (see logo) on top; then a large
  dashed **drop surface** — 2px dashed `rgba(55,192,122,.5)`, bg
  `rgba(55,192,122,.05)`, 18pt radius, `padding 40/30` — containing the
  **green-document logo mark**, `Drop a PDF or image to start` (19pt 650), and
  `or browse your Mac — everything stays on your device` (13pt, "browse" green).
  Then a `RECENT` row of two file chips (white mini page + filename).
- Titlebar shows the `pd·free` wordmark centered (no icon).
- Accepts drag-drop of PDF/PNG/JPEG and click-to-browse; both call
  `store.openReplacing`.

### 6. Logo · ref `#7a`, `#7c`, `#6d`
- **Wordmark (`pd·free`):** `pd` in off-white (800 weight, tight tracking
  `-1px`) immediately followed by a **green pill** containing `free` (700, ink
  `#08130c`, bg `#37c07a`, radius 999, padding ~`4/11`). Used in titlebar (small,
  ~13/11pt) and empty-state hero (large, ~34/25pt). No document icon in the
  titlebar.
- **App mark:** a **document silhouette** (folded top-right corner via a
  `polygon(0 0,70% 0,100% 22%,100% 100%,0 100%)` clip) containing 3 horizontal
  bars (full / 74% / 52% width). Two fills:
  - `#7a` **green tile:** rounded-square (24pt radius) green gradient
    `linear-gradient(145deg,#42d089,#279a60)`, a **white** page inside with
    **green** bars (`#2ea36a`, faint `#9ed9bc`).
  - `#7c` **green document:** the page itself is the green gradient with **white**
    bars — used inside the empty-state drop zone.
- App icon should ship at 16/18, 34, 64, 72pt+ (bars stay legible when small).

---

## Interactions & behavior
- **Fit-to-page:** on open and on every window/pane resize, the whole page fits
  the viewport. Already implemented — `PDFDocumentStore.updateViewport` →
  `fitDPIForCurrentPage` → `fitToPageDpi` FFI. Keep it; never open zoomed past
  fit.
- **Field detection on load:** `store.detectedBoxes` is populated per page in
  `renderCurrentPage()` via `boxesOnPage`. Render all of them immediately as
  overlays. `store.boxContaining(x:y:)` resolves a click to a box.
- **Normal field click:** single click on a highlighted box → inline text editor
  in place (existing `inlineEditBox`/`inlineEditText` → `commitInlineEdit` →
  `store.applyOverlay(TextOverlay…)`). Font size clamps `max(9, min(h*0.7, 18))`.
- **Signature field click:** open the sign flow (sheet or popover per saved
  state) → on place, `store.applySignature(pngData:at:SignaturePlacement)`.
- **Manual fallback:** double-click anywhere still drops an inline box
  (`handleDoubleTap`), for spots the scan missed.
- **"+" popover:** Open→`store.openReplacing`; Merge→`store.mergeAppending`;
  Insert image→`store.insertImagePage`; Split→`SplitSheet`→`store.splitExport`.
- **Animations:** popover hop = spring ~0.6s; field focus ring fade ~0.3s;
  active-tool/row highlight transitions ~0.15s. Use SwiftUI
  `.animation(.spring(response:0.45,dampingFraction:0.72), value:)`.
- **Busy state:** keep the existing top capsule `ProgressView` during `mutate`.

## State management
Existing on `PDFDocumentStore` (reuse): `data`, `document`, `pageIndex`,
`pageImage`, `pagePointSize`, `formFieldsList`, `annotationsList`,
`detectedBoxes`, `errorMessage`, `isBusy`, `fileURL`, `pageCount`, `title`.
Add:
- `savedSignatures: [SavedSignature]` — persisted (see below). `SavedSignature`
  = `{ id, pngData, kind: .signature | .initials, createdAt }`.
- Computed `signatureFields: [DetectedBox]` (or a flag on each box) — boxes whose
  underlying field name matches `/sign|initial/i` or whose AcroForm kind is a
  signature field.
- Sign-session state (can live in the view): `pendingSignatureFields: [Field]`,
  `currentSignIndex: Int`, `signProgress = (currentSignIndex, total)`.
- App-open behavior: start with **no** document (empty state) instead of loading
  the bundled sample.
**Persistence:** store `savedSignatures` as PNG blobs in Application Support
(e.g. `~/Library/Application Support/PDFree/signatures/`) with a small JSON
index, or `UserDefaults` for the index + files on disk. Load on launch so the
returning-user popover appears from the first signature onward.

## Design tokens

**Color**
| Token | Hex / value |
|---|---|
| Titlebar gradient | `#2a2723` → `#242118` |
| Inspector / panel bg | `#201d1a` |
| Rail / thumbnail bg | `#1a1815` |
| Canvas bg | radial `#161311` → `#0f0d0b` |
| Hairline | `rgba(255,255,255,0.06)`–`0.08` |
| Text high | `#f3efe8` |
| Text mid | `#a49c90` / `#8f887d` |
| Text low | `#6f6860` |
| Accent green | `#37c07a` |
| Green (darker / bars) | `#2ea36a` · faint `#9ed9bc` |
| Green gradient | `linear-gradient(145deg,#42d089,#279a60)` |
| Green tint bg | `color-mix(#37c07a 13–16%, panel)` |
| Green badge | bg `rgba(55,192,122,.22)`, text `#7fe0aa` |
| Green button ink | `#08130c` |
| Signature amber | `#e8b45a` · ink `#4a3200` · text `#9a6c1e` |
| Field wash (on white) | `color-mix(#37c07a 8–12%, #fff)` |
| Traffic lights | `#ff5f57` · `#febc2e` · `#28c840` |
| Signature ink (cursive) | `#1a2b6b` |

**Typography** — system font (SF Pro / `system-ui`).
| Use | Size / weight |
|---|---|
| Empty-state wordmark | `pd` 34/800, `free` 25/700 |
| Titlebar wordmark / title | 13/600 (pill `free` 11/700) |
| Section label | 10/600, uppercase, tracking ~1.2 |
| Inspector row | 13.5 (idle 500 / active 600) |
| Primary button (Export) | 14/650 |
| Overlay chip / badge | 11/600 |
| Fit / page-nav | 12/500, monospaced digits for the counter |
| Cursive signature/initials | `Snell Roundhand`, `Brush Script MT`, cursive |

**Metrics**
| Token | Value |
|---|---|
| Window radius | 13pt |
| Card / sheet radius | 14–16pt |
| Button radius | 10–11pt |
| Inspector row radius | 9pt |
| Pill radius | 999 |
| Field radius | 4–6pt |
| Thumbnail rail width | ~132pt |
| Right inspector width | ~274pt |
| Thumbnail | 88×114pt |
| Canvas page padding | 30pt |
| Titlebar height | 44pt |
| Inspector padding | 18 (v) / 16 (h), gap 15 |
| Row padding | 10 (v) / 11 (h), gap 11 |
| Page shadow | `0 18px 50px rgba(0,0,0,.55)` |
| Window shadow | `0 40px 90px -25px rgba(0,0,0,.75)` + 1px inner hairline |

## Assets
No image assets — the logo, field boxes, icons, and chrome are all drawn with
shapes/SF Symbols. Icons in the prototype are simple line SVGs; map them to SF
Symbols where possible (e.g. plus → `plus`, sign → `signature`, fill →
`character.cursor.ibeam`/`rectangle.and.pencil.and.ellipsis`, pages →
`doc.on.doc`, rotate → `arrow.clockwise`, delete → `trash`, export →
`square.and.arrow.up`, merge → `arrow.triangle.merge`, split →
`arrow.triangle.branch`, open → `folder`). The cursive signature preview uses a
system cursive font; the amber pen mark can be `signature`/`pencil.tip`.

## Files in this bundle
- `CLAUDE_CODE_PROMPT.md` — the prompt to paste into Claude Code.
- `README.md` — this document.
- `designs/pdFree UI Options.dc.html` — the full options canvas (open this).
- `designs/FormPage.dc.html` — the reusable form-page facsimile it imports.
- `designs/support.js` — runtime the two HTML files load (keep alongside them).
