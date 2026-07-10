// Deterministic shrink-to-fit font sizing for the inline field editor —
// computed once, in PDF points, and used identically for the live <input>
// and the exported overlayText stamp, so what's on screen while editing is
// exactly what exports (Core UX Principles: "WYSIWYG text sizing, always").
// Direct port of apps/macos/Sources/PDFree/Models/TextFit.swift — same
// constants, same algorithm — swapping AppKit's NSFont metrics for a
// Canvas 2D measureText() pass, since pdfree_core::forms::overlay_text
// draws literally at whatever font_size it's given and does no wrapping or
// clipping of its own; the shell owning this calculation is what makes the
// guarantee hold, not the engine.

const MAX_FONT_SIZE = 18;
const MIN_FONT_SIZE = 7;
const HORIZONTAL_INSET = 4;

let measureCanvas: HTMLCanvasElement | null = null;

function measureTextWidth(text: string, fontSize: number): number {
  measureCanvas ??= document.createElement("canvas");
  const ctx = measureCanvas.getContext("2d");
  if (!ctx) return text.length * fontSize * 0.55; // crude fallback, never hit in a real browser
  ctx.font = `${fontSize}px Helvetica, Arial, sans-serif`;
  return ctx.measureText(text).width;
}

/** The largest font size (in PDF points) that fits `text` inside a box
 * `boxWidthPts` x `boxHeightPts`, without exceeding MAX_FONT_SIZE. */
export function fontSizeFor(text: string, boxWidthPts: number, boxHeightPts: number): number {
  const heightBound = Math.max(MIN_FONT_SIZE, Math.min(boxHeightPts * 0.7, MAX_FONT_SIZE));
  if (!text) return heightBound;

  const available = boxWidthPts - HORIZONTAL_INSET;
  if (available <= 0) return heightBound;

  const measuredWidth = measureTextWidth(text, heightBound);
  if (measuredWidth <= available) return heightBound;

  const scale = available / measuredWidth;
  return Math.max(MIN_FONT_SIZE, heightBound * scale);
}
