# Claude Code prompt — PDFree macOS SwiftUI redesign

> Paste everything below into Claude Code, run from the repo root (`pdfree/`).
> Read `design_handoff_swiftui_redesign/README.md` for the full spec, exact
> tokens, and the design reference files in `design_handoff_swiftui_redesign/designs/`.

---

You are implementing a visual + interaction redesign of the **PDFree macOS app**
(`apps/macos`). The Rust engine (`pdfree-core` via `pdfree-ffi`) is done and must
not change — this is a SwiftUI-only task. All FFI symbols you need already exist
and are wrapped in `PDFDocumentStore` (`apps/macos/Sources/PDFree/Models/PDFDocumentStore.swift`).

## What to build

Replace the current top-toolbar layout with a **calm dark, right-inspector**
layout. The reference HTML prototypes in `design_handoff_swiftui_redesign/designs/`
are the source of truth for look and behavior — recreate them natively in
SwiftUI/AppKit using the app's existing patterns. Do **not** try to embed the HTML.

Read `README.md` first. Then implement, in this order:

1. **Design tokens** — add `apps/macos/Sources/PDFree/Theme.swift` with the
   colors/typography/metrics from the README's token table. Everything else
   references it. Set the window to a dark appearance.

2. **Layout shell** (`ContentView.swift`) — three columns via `HSplitView`:
   left page-thumbnail rail (~132pt), center canvas, right **Inspector**
   (~274pt). Remove the horizontal top toolbar entirely; its actions move
   into the inspector. Keep the traffic-light titlebar; show the `pd·free`
   green-pill wordmark centered in the titlebar **only when no document is
   open**, and the filename when one is.

3. **Empty / open state** (new `EmptyStateView.swift`) — when no document is
   loaded, the canvas becomes a large dashed drop surface (drag-drop + browse),
   with the `pd·free` wordmark above it, the green-document logo mark inside
   the drop zone, and a "Recent" row. This replaces auto-loading `sample.pdf`.

4. **Right Inspector** (new `InspectorView.swift`) — top: an **"Add or merge"**
   button that opens a popover menu (Open / Merge / Insert page / Image as page /
   Split). Then a `TOOLS` group (Fill fields · Sign · Annotate) with the live
   field count badge, a `PAGES` group (Insert · Rotate · Delete), and a primary
   **Export** button pinned to the bottom. Tools are dimmed/disabled until a doc
   is open. This is where the current `Tool` picker + `Open/Save/Merge/Split/…`
   buttons go.

5. **Field overlay** (`PageCanvasView.swift`) — every `store.detectedBoxes`
   (already scanned on load) draws as a quiet green-tinted rounded rect;
   the focused one gets a stronger ring + caret. **Signature/initials fields
   render differently**: an amber "Sign here" pill-button, never a text caret.
   Detect them by name (`/sign|initial/i`) and by AcroForm signature kind.
   Text auto-shrinks to fit the box (WYSIWYG — what's on screen is what exports).

6. **Sign flow** (rework `SignatureSheet.swift` + new `SignPopover.swift`) —
   - First time (no saved signature): open the full **sheet** with tabs
     Draw / Type / Upload and a "Save for reuse" toggle (on by default).
   - Returning user (≥1 saved): open a compact **popover anchored at the field**
     showing saved signature/initials as tap-to-place chips.
   - After placing, if more signature/initials fields remain, the popover
     **animates to the next one** (spring, ~0.6s) with a `n / total` progress
     count. Persist saved signatures across launches (see README → State).

## Guardrails (non-negotiable, from the product spec)

- Default zoom always fits the **whole page**, on open and on every resize.
  (`PDFDocumentStore.fitDPIForCurrentPage()` already does this — keep it.)
- Fields are detected + highlighted automatically on load. No "turn on" mode.
  Manual double-click-to-place stays as the fallback only.
- Signing is unlimited and free. Never gate fill/sign/merge/split/export behind
  any paywall, nag, cap, or watermark.
- Keep every existing `PDFDocumentStore` call site working; you're re-skinning
  and re-organizing the UI, not rewriting the engine bridge.

## When done

- `xcodegen generate` (project uses `project.yml`) if you add files, then build.
- Verify: launch → empty state; open a PDF → whole page fits, fields
  highlighted; click a normal field → type; click a signature field → sign
  flow; the "+" menu opens import/merge/split; export works.
- Match the reference screenshots' spacing, colors, and radii closely.
