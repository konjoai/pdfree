import { useRef, useState } from "react";
import type { FieldOverlay, PageSize } from "../types";
import { fontSizeFor } from "../lib/textFit";

interface Props {
  pageImageUrl: string;
  pagePointSize: PageSize;
  fieldOverlays: FieldOverlay[];
  /** `text` is passed together with the WYSIWYG font size (PDF points)
   * computed by `fontSizeFor` — the exact value used to size the field
   * live, so the exported stamp can never silently differ. */
  onFillField: (overlay: FieldOverlay, text: string, fontSizePts: number) => void;
  onSignField: (overlay: FieldOverlay) => void;
}

/** Renders the current page PNG with field overlays positioned by
 * percentage of the page's PDF-point size — resolution independent, so it
 * doesn't matter what DPI the PNG was actually rendered at. Mirrors
 * PageCanvasView.swift's overlay math (percent-of-page instead of a
 * points-per-pixel constant, since CSS does the scaling for us). */
export function PageCanvas({ pageImageUrl, pagePointSize, fieldOverlays, onFillField, onSignField }: Props) {
  const [editing, setEditing] = useState<{ overlay: FieldOverlay; text: string } | null>(null);
  const imgRef = useRef<HTMLImageElement>(null);

  /** CSS pixels per PDF point, from the rendered image's actual displayed
   * width — needed to show the WYSIWYG font size correctly on screen
   * (the size stamped into the export is always in PDF points, unaffected
   * by this; this conversion only controls the live preview). */
  const pxPerPoint = () => {
    const el = imgRef.current;
    if (!el || pagePointSize.width <= 0) return 1;
    return el.clientWidth / pagePointSize.width;
  };

  const pctBox = (o: FieldOverlay) => {
    const { width: pw, height: ph } = pagePointSize;
    if (pw <= 0 || ph <= 0) return { left: "0%", top: "0%", width: "0%", height: "0%" };
    return {
      left: `${(o.box.x / pw) * 100}%`,
      top: `${((ph - o.box.y - o.box.height) / ph) * 100}%`,
      width: `${(o.box.width / pw) * 100}%`,
      height: `${(o.box.height / ph) * 100}%`,
    };
  };

  return (
    <div style={{ position: "relative", display: "inline-block", boxShadow: "0 25px 50px rgba(0,0,0,0.55)" }}>
      <img ref={imgRef} src={pageImageUrl} alt="" style={{ display: "block", maxWidth: "100%", height: "auto" }} />
      {fieldOverlays.map((o, i) => {
        const isSignature = o.signatureKind !== "None";
        const key = o.fieldName ?? `box-${i}`;
        const isEditing = editing?.overlay === o;
        return (
          <div
            key={key}
            style={{
              position: "absolute",
              ...pctBox(o),
              border: `1.5px solid ${isSignature ? "var(--color-amber)" : "var(--color-field-border)"}`,
              background: isSignature ? "var(--color-amber-field-wash)" : "var(--color-field-fill-wash)",
              borderRadius: 3,
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              justifyContent: isSignature ? "center" : "flex-start",
              fontSize: 11,
              color: isSignature ? "var(--color-amber-text)" : "var(--color-green-dark)",
              fontWeight: 600,
              overflow: "hidden",
            }}
            onClick={() => {
              if (isSignature) {
                onSignField(o);
              } else {
                setEditing({ overlay: o, text: "" });
              }
            }}
          >
            {isSignature && !isEditing && "Sign here"}
            {isEditing &&
              (() => {
                const fontSizePts = fontSizeFor(editing.text, o.box.width, o.box.height);
                return (
                  <input
                    autoFocus
                    value={editing.text}
                    onChange={(e) => setEditing({ overlay: o, text: e.target.value })}
                    onBlur={() => {
                      const text = editing.text.trim();
                      if (text) onFillField(o, text, fontSizeFor(text, o.box.width, o.box.height));
                      setEditing(null);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                      if (e.key === "Escape") setEditing(null);
                    }}
                    style={{
                      width: "100%",
                      height: "100%",
                      border: "none",
                      outline: "none",
                      background: "transparent",
                      color: "black",
                      fontSize: fontSizePts * pxPerPoint(),
                      padding: "0 3px",
                    }}
                  />
                );
              })()}
          </div>
        );
      })}
    </div>
  );
}
