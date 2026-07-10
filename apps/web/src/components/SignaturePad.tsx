import { useRef, useState } from "react";

interface Props {
  title: string;
  onPlace: (pngBytes: Uint8Array) => void;
  onCancel: () => void;
}

/** Draw-to-sign modal. Keeps things to the single most common signing path
 * (freehand draw) — typed and uploaded signatures are on the macOS app but
 * not reimplemented here yet, see CLAUDE.md's Phase 4 checklist. */
export function SignaturePad({ title, onPlace, onCancel }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const drawing = useRef(false);
  const [hasStrokes, setHasStrokes] = useState(false);

  const ctx = () => canvasRef.current?.getContext("2d") ?? null;

  const pointerPos = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  };

  const start = (e: React.PointerEvent<HTMLCanvasElement>) => {
    drawing.current = true;
    const c = ctx();
    if (!c) return;
    const { x, y } = pointerPos(e);
    c.beginPath();
    c.moveTo(x, y);
  };

  const move = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!drawing.current) return;
    const c = ctx();
    if (!c) return;
    const { x, y } = pointerPos(e);
    c.lineTo(x, y);
    c.strokeStyle = "#1a2b6b";
    c.lineWidth = 2.5;
    c.lineCap = "round";
    c.lineJoin = "round";
    c.stroke();
    setHasStrokes(true);
  };

  const end = () => {
    drawing.current = false;
  };

  const clear = () => {
    const canvas = canvasRef.current;
    const c = ctx();
    if (!canvas || !c) return;
    c.clearRect(0, 0, canvas.width, canvas.height);
    setHasStrokes(false);
  };

  const use = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    canvas.toBlob(async (blob) => {
      if (!blob) return;
      const bytes = new Uint8Array(await blob.arrayBuffer());
      onPlace(bytes);
    }, "image/png");
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 100,
      }}
      onClick={onCancel}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          background: "var(--color-popover-bg)",
          borderRadius: "var(--radius-card)",
          padding: 20,
          width: 420,
          display: "flex",
          flexDirection: "column",
          gap: 14,
        }}
      >
        <div style={{ fontWeight: 600, color: "var(--color-text-high)" }}>{title}</div>
        <canvas
          ref={canvasRef}
          width={380}
          height={160}
          style={{ background: "white", borderRadius: "var(--radius-field)", touchAction: "none", cursor: "crosshair" }}
          onPointerDown={start}
          onPointerMove={move}
          onPointerUp={end}
          onPointerLeave={end}
        />
        <div style={{ display: "flex", justifyContent: "space-between" }}>
          <button onClick={clear} style={buttonStyle(false)}>
            Clear
          </button>
          <div style={{ display: "flex", gap: 8 }}>
            <button onClick={onCancel} style={buttonStyle(false)}>
              Cancel
            </button>
            <button onClick={use} disabled={!hasStrokes} style={buttonStyle(true, !hasStrokes)}>
              Use signature
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function buttonStyle(primary: boolean, disabled = false): React.CSSProperties {
  return {
    padding: "8px 14px",
    borderRadius: "var(--radius-button)",
    border: "none",
    fontWeight: 600,
    fontSize: 13,
    cursor: disabled ? "default" : "pointer",
    opacity: disabled ? 0.4 : 1,
    background: primary ? "var(--color-green)" : "transparent",
    color: primary ? "var(--color-green-ink)" : "var(--color-text-row)",
  };
}
