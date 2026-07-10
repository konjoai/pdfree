import { useCallback, useEffect, useRef, useState } from "react";
import { EmptyState } from "./components/EmptyState";
import { Inspector } from "./components/Inspector";
import { PageCanvas } from "./components/PageCanvas";
import { SignaturePad } from "./components/SignaturePad";
import * as engine from "./lib/engine";
import { usePdfDocumentStore } from "./hooks/usePdfDocumentStore";
import type { FieldOverlay } from "./types";

export default function App() {
  const {
    state,
    signatureFields,
    openReplacing,
    closeDocument,
    goToPage,
    updateViewport,
    mutate,
    clearError,
  } = usePdfDocumentStore();

  const [pdfiumReady, setPdfiumReady] = useState<"loading" | "ready" | "error">("loading");
  const [pdfiumError, setPdfiumError] = useState<string | null>(null);
  const [signingField, setSigningField] = useState<FieldOverlay | null>(null);
  const canvasAreaRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    engine
      .ensurePdfiumReady()
      .then(() => setPdfiumReady("ready"))
      .catch((e) => {
        setPdfiumReady("error");
        setPdfiumError(e instanceof Error ? e.message : String(e));
      });
  }, []);

  useEffect(() => {
    const el = canvasAreaRef.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      updateViewport(Math.max(width - 60, 0), Math.max(height - 60, 0));
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [updateViewport]);

  const handleOpen = useCallback(
    (data: Uint8Array, fileName: string) => openReplacing(data, fileName),
    [openReplacing],
  );

  const handleMerge = useCallback(
    (data: Uint8Array) => {
      mutate("Merge PDF", (bytes) => engine.mergeDocuments([bytes, data]));
    },
    [mutate],
  );

  const handleInsertBlankPage = useCallback(() => {
    mutate("Insert blank page", async (bytes) => {
      const blank = blankPagePng();
      const blankPdf = await engine.fromImage(blank, 72);
      return engine.mergeDocuments([bytes, blankPdf]);
    });
  }, [mutate]);

  const handleRotate = useCallback(() => {
    mutate("Rotate page", (bytes) => engine.rotatePage(bytes, state.pageIndex, "Clockwise90"));
  }, [mutate, state.pageIndex]);

  const handleDelete = useCallback(() => {
    if (state.pageCount <= 1) return;
    const remaining = Array.from({ length: state.pageCount }, (_, i) => i).filter(
      (i) => i !== state.pageIndex,
    );
    mutate("Delete page", (bytes) => engine.extractPages(bytes, remaining));
  }, [mutate, state.pageCount, state.pageIndex]);

  const handleExport = useCallback(() => {
    if (!state.data) return;
    const blob = new Blob([state.data as unknown as BlobPart], { type: "application/pdf" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = state.fileName ?? "document.pdf";
    a.click();
    URL.revokeObjectURL(url);
  }, [state.data, state.fileName]);

  const handleFillField = useCallback(
    (overlay: FieldOverlay, text: string, fontSizePts: number) => {
      mutate("Fill field", (bytes) =>
        engine.overlayText(bytes, [
          { page: overlay.box.page, x: overlay.box.x, y: overlay.box.y, text, fontSize: fontSizePts },
        ]),
      );
    },
    [mutate],
  );

  const handleSignPlace = useCallback(
    (pngBytes: Uint8Array) => {
      if (!signingField) return;
      const field = signingField;
      mutate("Place signature", (bytes) =>
        engine.placeSignatureWithAudit(
          bytes,
          pngBytes,
          { page: field.box.page, x: field.box.x, y: field.box.y, width: field.box.width, height: field.box.height },
          { signerName: "PDFree user", signedAt: new Date().toLocaleString(), deviceInfo: navigator.userAgent },
        ),
      );
      setSigningField(null);
    },
    [mutate, signingField],
  );

  if (pdfiumReady !== "ready") {
    return (
      <div style={{ height: "100%", display: "flex", alignItems: "center", justifyContent: "center" }}>
        <div style={{ color: "var(--color-text-mid)", textAlign: "center", maxWidth: 420 }}>
          {pdfiumReady === "loading" ? (
            "Loading PDFium…"
          ) : (
            <>
              <div style={{ color: "#ff8a80", marginBottom: 8 }}>PDFium failed to load</div>
              <div style={{ fontSize: 12.5 }}>{pdfiumError}</div>
              <div style={{ fontSize: 11.5, marginTop: 8, color: "var(--color-text-low)" }}>
                See apps/web/public/pdfium/README.md
              </div>
            </>
          )}
        </div>
      </div>
    );
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div
        style={{
          height: "var(--height-titlebar)",
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          position: "relative",
          background: "linear-gradient(var(--color-titlebar-top), var(--color-titlebar-bottom))",
          borderBottom: "1px solid rgba(0,0,0,0.4)",
          color: "var(--color-text-row)",
          fontSize: 13,
          fontWeight: 600,
        }}
      >
        {state.hasDocument ? state.title : "PDFree"}
        {state.hasDocument && (
          <button
            onClick={closeDocument}
            title="Close document"
            style={{
              position: "absolute",
              right: 12,
              background: "none",
              border: "none",
              color: "var(--color-text-mid2)",
              cursor: "pointer",
              fontSize: 15,
            }}
          >
            ×
          </button>
        )}
      </div>

      {!state.hasDocument ? (
        <EmptyState onOpen={handleOpen} errorMessage={state.errorMessage} />
      ) : (
        <div style={{ flex: 1, display: "flex", overflow: "hidden" }}>
          <div
            ref={canvasAreaRef}
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              overflow: "auto",
              padding: 30,
              background: "radial-gradient(circle at 50% 0%, var(--color-canvas-top), var(--color-canvas-bottom))",
              position: "relative",
            }}
          >
            {state.pageImageUrl ? (
              <PageCanvas
                pageImageUrl={state.pageImageUrl}
                pagePointSize={state.pagePointSize}
                fieldOverlays={state.fieldOverlays}
                onFillField={handleFillField}
                onSignField={setSigningField}
              />
            ) : (
              <div style={{ color: "var(--color-text-mid)" }}>Rendering…</div>
            )}

            {state.pageCount > 1 && (
              <div
                style={{
                  position: "absolute",
                  bottom: 18,
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  padding: "6px 10px",
                  borderRadius: 999,
                  background: "rgba(255,255,255,0.08)",
                  color: "var(--color-text-row)",
                  fontSize: 12,
                }}
              >
                <button onClick={() => goToPage(state.pageIndex - 1)} disabled={state.pageIndex === 0} style={navButtonStyle}>
                  ‹
                </button>
                <span>
                  {state.pageIndex + 1} / {state.pageCount}
                </span>
                <button
                  onClick={() => goToPage(state.pageIndex + 1)}
                  disabled={state.pageIndex + 1 >= state.pageCount}
                  style={navButtonStyle}
                >
                  ›
                </button>
              </div>
            )}
          </div>

          <Inspector
            hasDocument={state.hasDocument}
            fieldCount={state.formFieldsList.length}
            signatureFieldCount={signatureFields.length}
            onOpen={handleOpen}
            onMerge={handleMerge}
            onInsertBlankPage={handleInsertBlankPage}
            onRotate={handleRotate}
            onDelete={handleDelete}
            onExport={handleExport}
          />
        </div>
      )}

      {state.errorMessage && state.hasDocument && (
        <div
          role="alert"
          style={{ position: "fixed", bottom: 16, left: "50%", transform: "translateX(-50%)", background: "#3a1f1f", color: "#ffb4a8", padding: "10px 16px", borderRadius: 8, fontSize: 12.5, cursor: "pointer" }}
          onClick={clearError}
        >
          {state.errorMessage} (click to dismiss)
        </div>
      )}

      {signingField && (
        <SignaturePad
          title={signingField.signatureKind === "Initials" ? "Initial here" : "Sign here"}
          onPlace={handleSignPlace}
          onCancel={() => setSigningField(null)}
        />
      )}
    </div>
  );
}

const navButtonStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  color: "inherit",
  cursor: "pointer",
  fontSize: 14,
};

/** A blank, opaque-white 612x792pt (US Letter @ 72dpi) page image — mirrors
 * PDFDocumentStore.swift's blankPagePNG(), built via an offscreen canvas
 * instead of AppKit's NSImage. */
function blankPagePng(): Uint8Array {
  const canvas = document.createElement("canvas");
  canvas.width = 612;
  canvas.height = 792;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "white";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  const dataUrl = canvas.toDataURL("image/png");
  const base64 = dataUrl.split(",")[1];
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}
