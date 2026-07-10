// Typed wrappers around PDFree's engine — dispatches to the in-browser WASM
// module or, under Tauri, to native IPC commands (see `./runtime.ts`). Both
// backends speak the same camelCase JSON shapes (`../types`), so this file
// is the *only* place that needs to know which one is actually running —
// every component above it (App.tsx, hooks, other components) is backend-
// agnostic. This is what makes "Tauri reuses the web UI" true in practice:
// the exact same built `apps/web/dist` output runs unmodified in a plain
// browser tab and inside the Tauri window.
//
// Every Rust `Result<T, JsError>` (WASM) or `Result<T, String>` (Tauri)
// becomes "returns T or throws" in JS — callers should wrap calls in
// try/catch either way.

import { invoke } from "@tauri-apps/api/core";
import * as wasm from "../wasm/pdfree_wasm";
import { isTauri } from "./runtime";
import type {
  Annotation,
  AnnotationInfo,
  DetectedBox,
  FieldFill,
  FormField,
  PageRange,
  PageSize,
  Rotation,
  SignatureAudit,
  SignaturePlacement,
  TextOverlay,
  TextRun,
} from "../types";

export { ensurePdfiumReady } from "./pdfium";

interface DocumentInfo {
  pageCount: number;
  title: string | null;
  author: string | null;
}

export class PdfDocument {
  private readonly wasmDoc: wasm.PdfDocument | null;
  private readonly bytes: Uint8Array | null;
  private readonly info: DocumentInfo | null; // set only on the Tauri path

  private constructor(wasmDoc: wasm.PdfDocument | null, bytes: Uint8Array | null, info: DocumentInfo | null) {
    this.wasmDoc = wasmDoc;
    this.bytes = bytes;
    this.info = info;
  }

  /** Async because the Tauri path needs one IPC round trip to read
   * metadata up front (there's no persistent native document handle — see
   * apps/desktop/src-tauri/src/commands.rs's module docs). The WASM path
   * resolves synchronously-fast but stays a Promise so callers don't need
   * to branch on backend. */
  static async fromBytes(bytes: Uint8Array): Promise<PdfDocument> {
    if (isTauri()) {
      const info = await invoke<DocumentInfo>("document_info", { pdfBytes: Array.from(bytes) });
      return new PdfDocument(null, bytes, info);
    }
    return new PdfDocument(new wasm.PdfDocument(bytes), null, null);
  }

  pageCount(): number {
    return this.wasmDoc ? this.wasmDoc.pageCount() : this.info!.pageCount;
  }

  title(): string | undefined {
    if (this.wasmDoc) return this.wasmDoc.title() || undefined;
    return this.info!.title ?? undefined;
  }

  author(): string | undefined {
    if (this.wasmDoc) return this.wasmDoc.author() || undefined;
    return this.info!.author ?? undefined;
  }

  async renderPage(index: number, dpi: number): Promise<Uint8Array> {
    if (this.wasmDoc) return this.wasmDoc.renderPage(index, dpi);
    const bytes = await invoke<number[]>("render_page", { pdfBytes: Array.from(this.bytes!), index, dpi });
    return new Uint8Array(bytes);
  }

  async pageSize(index: number): Promise<PageSize> {
    if (this.wasmDoc) return this.wasmDoc.pageSize(index) as PageSize;
    return invoke<PageSize>("page_size", { pdfBytes: Array.from(this.bytes!), index });
  }
}

export async function fitToPageDpi(
  pageWidthPts: number,
  pageHeightPts: number,
  viewportWidthPx: number,
  viewportHeightPx: number,
): Promise<number> {
  if (isTauri()) {
    return invoke<number>("fit_to_page_dpi", {
      pageWidthPts,
      pageHeightPts,
      viewportWidthPx,
      viewportHeightPx,
    });
  }
  return wasm.fitToPageDpi(pageWidthPts, pageHeightPts, viewportWidthPx, viewportHeightPx);
}

export async function formFields(pdfBytes: Uint8Array): Promise<FormField[]> {
  if (isTauri()) return invoke<FormField[]>("form_fields", { pdfBytes: Array.from(pdfBytes) });
  return wasm.formFields(pdfBytes) as FormField[];
}

export function formFill(pdfBytes: Uint8Array, values: FieldFill[]): Uint8Array {
  assertBrowserOnly("formFill");
  return wasm.formFill(pdfBytes, values);
}

export async function overlayText(pdfBytes: Uint8Array, overlays: TextOverlay[]): Promise<Uint8Array> {
  if (isTauri()) {
    const bytes = await invoke<number[]>("overlay_text", { pdfBytes: Array.from(pdfBytes), overlays });
    return new Uint8Array(bytes);
  }
  return wasm.overlayText(pdfBytes, overlays);
}

export function placeSignature(pdfBytes: Uint8Array, imagePng: Uint8Array, at: SignaturePlacement): Uint8Array {
  assertBrowserOnly("placeSignature");
  return wasm.placeSignature(pdfBytes, imagePng, at);
}

export async function placeSignatureWithAudit(
  pdfBytes: Uint8Array,
  imagePng: Uint8Array,
  at: SignaturePlacement,
  audit: SignatureAudit,
): Promise<Uint8Array> {
  if (isTauri()) {
    const bytes = await invoke<number[]>("place_signature_with_audit", {
      pdfBytes: Array.from(pdfBytes),
      imagePng: Array.from(imagePng),
      at,
      audit,
    });
    return new Uint8Array(bytes);
  }
  return wasm.placeSignatureWithAudit(pdfBytes, imagePng, at, audit);
}

export function addAnnotations(pdfBytes: Uint8Array, annotations: Annotation[]): Uint8Array {
  assertBrowserOnly("addAnnotations");
  return wasm.addAnnotations(pdfBytes, annotations);
}

export function listAnnotations(pdfBytes: Uint8Array): AnnotationInfo[] {
  assertBrowserOnly("listAnnotations");
  return wasm.listAnnotations(pdfBytes) as AnnotationInfo[];
}

export function textRuns(pdfBytes: Uint8Array): TextRun[] {
  assertBrowserOnly("textRuns");
  return wasm.textRuns(pdfBytes) as TextRun[];
}

export function textRunAtPoint(pdfBytes: Uint8Array, page: number, x: number, y: number): TextRun | null {
  assertBrowserOnly("textRunAtPoint");
  return wasm.textRunAtPoint(pdfBytes, page, x, y) as TextRun | null;
}

export function replaceText(pdfBytes: Uint8Array, page: number, find: string, replace: string): Uint8Array {
  assertBrowserOnly("replaceText");
  return wasm.replaceText(pdfBytes, page, find, replace);
}

export async function mergeDocuments(documents: Uint8Array[]): Promise<Uint8Array> {
  if (isTauri()) {
    const bytes = await invoke<number[]>("merge_documents", {
      documents: documents.map((d) => Array.from(d)),
    });
    return new Uint8Array(bytes);
  }
  return wasm.mergeDocuments(documents);
}

export function splitDocument(pdfBytes: Uint8Array, ranges: PageRange[]): Uint8Array[] {
  assertBrowserOnly("splitDocument");
  return wasm.splitDocument(pdfBytes, ranges) as Uint8Array[];
}

export async function rotatePage(pdfBytes: Uint8Array, page: number, rotation: Rotation): Promise<Uint8Array> {
  if (isTauri()) {
    const bytes = await invoke<number[]>("rotate_page", { pdfBytes: Array.from(pdfBytes), page, rotation });
    return new Uint8Array(bytes);
  }
  return wasm.rotatePage(pdfBytes, page, rotation);
}

export async function extractPages(pdfBytes: Uint8Array, pages: number[]): Promise<Uint8Array> {
  if (isTauri()) {
    const bytes = await invoke<number[]>("extract_pages", { pdfBytes: Array.from(pdfBytes), pageList: pages });
    return new Uint8Array(bytes);
  }
  return wasm.extractPages(pdfBytes, pages);
}

export function reorderPages(pdfBytes: Uint8Array, newOrder: number[]): Uint8Array {
  assertBrowserOnly("reorderPages");
  return wasm.reorderPages(pdfBytes, newOrder);
}

export function toText(pdfBytes: Uint8Array): string {
  assertBrowserOnly("toText");
  return wasm.toText(pdfBytes);
}

export async function fromImage(imageBytes: Uint8Array, dpi: number): Promise<Uint8Array> {
  if (isTauri()) {
    const bytes = await invoke<number[]>("from_image", { imageBytes: Array.from(imageBytes), dpi });
    return new Uint8Array(bytes);
  }
  return wasm.fromImage(imageBytes, dpi);
}

export function boxAtPoint(pdfBytes: Uint8Array, page: number, x: number, y: number): DetectedBox | null {
  assertBrowserOnly("boxAtPoint");
  return wasm.boxAtPoint(pdfBytes, page, x, y) as DetectedBox | null;
}

export async function boxesOnPage(pdfBytes: Uint8Array, page: number): Promise<DetectedBox[]> {
  if (isTauri()) return invoke<DetectedBox[]>("boxes_on_page", { pdfBytes: Array.from(pdfBytes), page });
  return wasm.boxesOnPage(pdfBytes, page) as DetectedBox[];
}

/** A handful of engine functions aren't wired up as Tauri commands yet
 * (see apps/desktop/src-tauri/src/commands.rs's doc comment) — only the
 * subset apps/web's current UI actually calls. Calling one of the rest
 * under Tauri fails loudly instead of silently invoking the (absent)
 * native PDFium-via-WASM path. */
function assertBrowserOnly(fnName: string): void {
  if (isTauri()) {
    throw new Error(
      `${fnName}() is not yet wired up as a Tauri command — see apps/desktop/src-tauri/src/commands.rs`,
    );
  }
}
