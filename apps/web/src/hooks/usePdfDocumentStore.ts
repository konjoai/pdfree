// Owns the current PDF's bytes and parsed PdfDocument handle — the web
// equivalent of apps/macos/Sources/PDFree/Models/PDFDocumentStore.swift.
// Every engine mutation flows through `mutate()`: takes the current bytes,
// produces new bytes, reloads every derived piece of state from the result.
//
// Every engine call here is awaited — `engine.ts` functions are async
// uniformly across both backends (WASM and Tauri IPC) even though the WASM
// calls themselves resolve synchronously-fast, so this hook doesn't need to
// know or care which backend is actually running.
import { useCallback, useMemo, useRef, useState } from "react";
import * as engine from "../lib/engine";
import type { FieldOverlay, FillableField, FormField, PageSize } from "../types";

export interface PdfDocumentState {
  hasDocument: boolean;
  fileName: string | null;
  data: Uint8Array | null;
  pageCount: number;
  pageIndex: number;
  title: string;
  pageImageUrl: string | null;
  pagePointSize: PageSize;
  formFieldsList: FormField[];
  fieldOverlays: FieldOverlay[];
  errorMessage: string | null;
  isBusy: boolean;
}

const EMPTY_STATE: PdfDocumentState = {
  hasDocument: false,
  fileName: null,
  data: null,
  pageCount: 0,
  pageIndex: 0,
  title: "Untitled",
  pageImageUrl: null,
  pagePointSize: { width: 0, height: 0 },
  formFieldsList: [],
  fieldOverlays: [],
  errorMessage: null,
  isBusy: false,
};

const FALLBACK_DPI = 150;

/** `engine.fillableFields()` already does the label-aware AcroForm/detected-
 * box merge (and the signature-field-with-no-drawn-box fallback) in one
 * PDFium parse — this just reshapes its flat geometry into the `FieldOverlay`
 * shape `PageCanvas` renders. */
function toFieldOverlays(fields: FillableField[]): FieldOverlay[] {
  return fields.map((f) => ({
    box: { page: f.page, x: f.x, y: f.y, width: f.width, height: f.height },
    signatureKind: f.signatureKind,
    fieldName: f.fieldName,
  }));
}

export function usePdfDocumentStore() {
  const [state, setState] = useState<PdfDocumentState>(EMPTY_STATE);
  const documentRef = useRef<engine.PdfDocument | null>(null);
  const viewportRef = useRef<{ width: number; height: number }>({ width: 0, height: 0 });
  // Guards against a stale render finishing after a newer one started (e.g.
  // rapid page navigation) and clobbering fresher state.
  const renderTokenRef = useRef(0);

  const renderCurrentPage = useCallback(async (data: Uint8Array, pageIndex: number) => {
    const doc = documentRef.current;
    if (!doc) return;
    const token = ++renderTokenRef.current;

    let dpi = FALLBACK_DPI;
    let pagePointSize: PageSize = { width: 0, height: 0 };
    const { width: vw, height: vh } = viewportRef.current;
    try {
      const size = await doc.pageSize(pageIndex);
      if (vw > 0 && vh > 0) {
        const fit = await engine.fitToPageDpi(size.width, size.height, vw, vh);
        dpi = fit > 0 ? fit : FALLBACK_DPI;
      }
      pagePointSize = { width: size.width, height: size.height };
    } catch {
      // fall through with defaults; render below will surface a real error
    }

    let pageImageUrl: string | null = null;
    let fieldOverlays: FieldOverlay[] = [];
    try {
      const png = await doc.renderPage(pageIndex, dpi);
      const blob = new Blob([png as unknown as BlobPart], { type: "image/png" });
      pageImageUrl = URL.createObjectURL(blob);
    } catch (e) {
      if (token === renderTokenRef.current) {
        setState((s) => ({ ...s, errorMessage: describeError(e) }));
      }
    }
    try {
      fieldOverlays = toFieldOverlays(await engine.fillableFields(data, pageIndex));
    } catch {
      fieldOverlays = [];
    }

    if (token !== renderTokenRef.current) {
      // A newer render superseded this one — drop the now-stale image URL
      // instead of leaking it or flashing an old page.
      if (pageImageUrl) URL.revokeObjectURL(pageImageUrl);
      return;
    }

    setState((s) => {
      if (s.pageImageUrl) URL.revokeObjectURL(s.pageImageUrl);
      return { ...s, pageImageUrl, pagePointSize, fieldOverlays };
    });
  }, []);

  const openReplacing = useCallback(
    async (data: Uint8Array, fileName: string | null) => {
      try {
        const doc = await engine.PdfDocument.fromBytes(data);
        documentRef.current = doc;
        const formFieldsList = await safely(() => engine.formFields(data), []);
        setState({
          ...EMPTY_STATE,
          hasDocument: true,
          fileName,
          data,
          pageCount: doc.pageCount(),
          pageIndex: 0,
          title: doc.title() ?? fileName ?? "Untitled",
          formFieldsList,
        });
        await renderCurrentPage(data, 0);
      } catch (e) {
        setState((s) => ({ ...s, errorMessage: describeError(e) }));
      }
    },
    [renderCurrentPage],
  );

  const closeDocument = useCallback(() => {
    setState((s) => {
      if (s.pageImageUrl) URL.revokeObjectURL(s.pageImageUrl);
      return EMPTY_STATE;
    });
    documentRef.current = null;
  }, []);

  const goToPage = useCallback(
    (index: number) => {
      setState((s) => {
        if (index < 0 || index >= s.pageCount || !s.data) return s;
        void renderCurrentPage(s.data, index);
        return { ...s, pageIndex: index };
      });
    },
    [renderCurrentPage],
  );

  const updateViewport = useCallback(
    (width: number, height: number) => {
      const prev = viewportRef.current;
      if (Math.abs(width - prev.width) <= 1 && Math.abs(height - prev.height) <= 1) return;
      viewportRef.current = { width, height };
      setState((s) => {
        if (s.data) void renderCurrentPage(s.data, s.pageIndex);
        return s;
      });
    },
    [renderCurrentPage],
  );

  /** Apply an operation that transforms the current bytes into new bytes,
   * then reload every derived piece of state from the result — the same
   * funnel PDFDocumentStore.mutate() is on macOS. */
  const mutate = useCallback(
    async (label: string, op: (data: Uint8Array) => Uint8Array | Promise<Uint8Array>) => {
      const current = state.data;
      if (!current) return;
      setState((s) => ({ ...s, isBusy: true }));
      try {
        const newData = await op(current);
        const doc = await engine.PdfDocument.fromBytes(newData);
        documentRef.current = doc;
        const pageCount = doc.pageCount();
        const pageIndex = Math.min(state.pageIndex, pageCount - 1);
        const formFieldsList = await safely(() => engine.formFields(newData), []);
        await renderCurrentPage(newData, pageIndex);
        setState((s) => ({
          ...s,
          data: newData,
          pageCount,
          pageIndex,
          title: doc.title() ?? s.fileName ?? "Untitled",
          formFieldsList,
          errorMessage: null,
          isBusy: false,
        }));
      } catch (e) {
        setState((s) => ({ ...s, errorMessage: `${label} failed: ${describeError(e)}`, isBusy: false }));
      }
    },
    [state.data, state.pageIndex, renderCurrentPage],
  );

  const clearError = useCallback(() => setState((s) => ({ ...s, errorMessage: null })), []);

  const signatureFields = useMemo(
    () => state.formFieldsList.filter((f) => f.signatureKind !== "None"),
    [state.formFieldsList],
  );

  return {
    state,
    signatureFields,
    openReplacing,
    closeDocument,
    goToPage,
    updateViewport,
    mutate,
    clearError,
  };
}

async function safely<T>(fn: () => T | Promise<T>, fallback: T): Promise<T> {
  try {
    return await fn();
  } catch {
    return fallback;
  }
}

function describeError(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}
