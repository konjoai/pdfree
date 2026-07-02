# PDFree — UI Design Handoff

> For: a fresh Claude chat/design session (no repo access assumed)
> From: Konjo AI · Wes · 2026-07-01
> Goal: design the core PDFree user experience — mockups/flows, not code.

Paste this whole document in as context. Everything you need to design the
v1 UI is below; you do not need the codebase.

---

## What PDFree is

PDFree is a truly free, no-watermark, no-limit PDF app (macOS first, then
web, Windows/Linux via Tauri, iOS later). The pitch: every "free" PDF tool
today is fake-free — watermarks, page/task caps, paywalled signing, or
privacy-invasive cloud uploads. PDFree is the honest alternative: open a
PDF, fill it, sign it, merge/split it, export it — free, unlimited, local-first,
every time.

The engine (Rust core over PDFium) is done for the core operations. This
handoff is purely about the **interface** on top of it — first for macOS
(native SwiftUI), with the same interaction model carrying over to web/Tauri
later.

## Who it's for

Anyone who has ever had to fill out and sign a PDF — tax forms, contracts,
leases, applications. Not a power-user tool. Most sessions look like: open a
PDF → fill a few fields → sign it → maybe add/remove a page → export. That's
it. Design for that path being *effortless*, not for covering every possible
PDF operation up front.

## Design philosophy — read this before designing anything

**Anti-reference: Adobe Acrobat.** Acrobat has too many buttons, too many
menus, too much chrome. Every extra click/menu-dive is a defect. The
governing question for every screen: *"does a first-time user need to think
here?"* If yes, simplify.

**Reliability and clarity over visual flourish.** It's fine if this looks a
little plain at first — rough edges in aesthetics are acceptable. What's not
acceptable is ambiguity about whether a click did the right thing, or a user
needing to hunt for a feature they know must exist.

## Hard requirements (non-negotiable, from real user frustration)

1. **Default view = the whole page, always visible.** On document open, and
   on every window resize, the zoom level must fit the *entire* page (full
   height and width) inside the viewport — regardless of screen size or
   window size. Never open zoomed in past what fits. (This is a confirmed bug
   in the current build — mockups should make the intended default state
   obvious so it doesn't regress again.)

2. **Every fillable field is found automatically, instantly, on load.**
   The engine scans the whole document the moment it opens — real AcroForm
   fields *and* vector-drawn boxes/table cells that aren't technically form
   fields but look fillable to a human (this detection already exists in the
   Rust core). Every detected field should show some kind of highlighted,
   clickable affordance immediately, with no user action required to "turn
   on" field detection. Manually double-click-to-place-a-box is a fallback
   for the rare field the scan misses — not the primary interaction. Design
   for "it just works," not "user turns on a mode."

3. **Signature and initials fields are not text fields.** Any field whose
   label reads like "Signature," "Sign here," "Initials," "Initial here,"
   etc. should visually look distinct from a regular fillable box and, when
   clicked, should launch a sign flow (draw with trackpad/mouse, type in a
   cursive font, upload an image, or reuse a previously saved signature) —
   never a plain text cursor. This is the single biggest paid-competitor
   annoyance PDFree exists to fix — signing is unlimited and free.

4. **What you see while typing is exactly what exports.** Text in a filled
   field can auto-shrink to fit the box as the user types (that's fine and
   expected), but there must be no separate "export-time" resize or clipping
   that differs from what was on screen during editing. The mockup/interaction
   spec should make clear that the rendered live state *is* the final state —
   no surprise re-flow on save.

5. **Core document operations (import, merge, split, add/remove page) live
   in the main canvas, not hidden in a File menu.** Concretely: a persistent
   "+"-style action, reachable without a menu bar trip, that covers "add a
   file" / "add a page" / "merge another PDF in" style operations. Design the
   exact affordance and what it expands into (a popover? inline sheet? file
   picker directly?).

6. **Minimal default toolbar.** The default, always-visible toolbar should
   cover only: open, fill, sign, add/remove page, export. Everything else
   (annotation styles, advanced page reordering, format conversion, etc.)
   should be one level deeper — a secondary panel, not fighting for space in
   the primary chrome.

7. **No paywalls, no watermarks, no artificial caps anywhere in the flow.**
   The UI should never gate a core action (fill, sign — including unlimited
   signatures, merge, split, export) behind a paywall or nag screen. If a
   future paid tier exists (see below), it must be visually and functionally
   separate from this core path, never a blocker on it.

## Feature surface to design for (v1 core)

Already built in the engine, needs UI:
- Open + render a PDF (sidebar with per-page thumbnails for navigation)
- Fill AcroForm fields: text, checkboxes, dropdowns
- Fill non-interactive PDFs via auto-detected overlay boxes (see requirement 2)
- Sign: draw / type / image upload, placed at a click or auto-placed in a
  detected signature field (see requirement 3); reuse a saved signature
  across documents without redrawing
- Edit existing text in the PDF in place (click a text run, retype)
- Annotate: highlight, underline, strikethrough, sticky notes
- Merge multiple PDFs into one
- Split a PDF by page range
- Reorder / rotate / delete / insert pages (thumbnail sidebar, drag to
  reorder)
- Extract text from a document
- Convert an image to a single-page PDF
- Export / save, always preserving original layout

Explicitly out of scope for v1 (don't design for these yet):
- Real-time collaboration
- Cloud storage sync
- Certified/legal-grade e-signature audit trail (ESIGN/eIDAS) — v1 signing
  gets a *lightweight* local audit record only (timestamp, signer name,
  device info where available), not a full certified chain of custody
- Redact-and-overwrite an already-filled field (deferred, possibly a later
  paid feature)
- PDF ↔ DOCX conversion (undecided technically, not a UI concern yet)

## Platforms and rollout order

1. **macOS** (native SwiftUI) — design this first. Full v1 feature set.
2. **Web** (React) — same interaction model, browser chrome constraints.
3. **Windows/Linux** (Tauri, reusing the web UI)
4. **iOS** (later, shares SwiftUI views with macOS — touch-adapted)

Design macOS first, but flag anywhere a pattern (like the persistent "+"
action, or field-detection highlighting) will need to adapt for touch/web so
we don't design something macOS-only that has no answer on other platforms.

## What to actually produce

Please design/propose, in order of priority:

1. **Document view — default state.** Whole-page-visible zoom, page
   thumbnail sidebar, minimal toolbar. Show what "first thing you see after
   opening a PDF" looks like.
2. **Auto-detected field overlay.** How detected fillable fields (including
   distinct signature/initials fields) are visually indicated on the page
   without feeling cluttered, and what happens on click for a normal field
   vs. a signature field.
3. **Sign flow.** Draw/type/upload/reuse-saved, and how a signature gets
   placed onto a detected signature field vs. a manual click-to-place
   fallback.
4. **The "+" quick action** for import/merge/split/add-page — propose the
   exact UI (button placement, what it opens, how merge/split feel like one
   fluid action rather than a wizard).
5. **Toolbar and information hierarchy** — what's always visible vs. one
   level deep (annotation tools, page thumbnail sidebar toggle, export
   options, etc.).

Feel free to propose 2-3 directions for #1-#4 if there's a genuine tradeoff,
but keep total scope tight — this is meant to unblock implementation, not
become an open-ended exploration.
