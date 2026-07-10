import { useCallback, useRef, useState } from "react";

interface Props {
  onOpen: (data: Uint8Array, fileName: string) => void;
  errorMessage: string | null;
}

/** "The drop surface IS the window" — no bundled sample, no auto-load. */
export function EmptyState({ onOpen, errorMessage }: Props) {
  const [dragOver, setDragOver] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const openFile = useCallback(
    async (file: File) => {
      const buf = new Uint8Array(await file.arrayBuffer());
      onOpen(buf, file.name);
    },
    [onOpen],
  );

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const file = e.dataTransfer.files[0];
      if (file) void openFile(file);
    },
    [openFile],
  );

  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "var(--color-panel-bg)",
      }}
    >
      <div style={{ width: 520, display: "flex", flexDirection: "column", gap: 24 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: 6 }}>
          <span style={{ fontWeight: 700, fontSize: 22, color: "var(--color-text-high)" }}>pd</span>
          <span
            style={{
              fontWeight: 700,
              fontSize: 14,
              color: "var(--color-green-ink)",
              background: "var(--color-green)",
              padding: "3px 10px",
              borderRadius: 999,
            }}
          >
            free
          </span>
        </div>

        <div
          onDragOver={(e) => {
            e.preventDefault();
            setDragOver(true);
          }}
          onDragLeave={() => setDragOver(false)}
          onDrop={handleDrop}
          onClick={() => fileInputRef.current?.click()}
          style={{
            border: `1px dashed ${dragOver ? "var(--color-green)" : "var(--color-hairline)"}`,
            borderRadius: "var(--radius-card)",
            padding: "56px 24px",
            textAlign: "center",
            cursor: "pointer",
            background: dragOver ? "var(--color-field-fill-wash)" : "transparent",
            transition: "background 0.15s, border-color 0.15s",
          }}
        >
          <div style={{ fontSize: 32, marginBottom: 12 }}>📄</div>
          <div style={{ fontWeight: 600, fontSize: 15, color: "var(--color-text-high)" }}>
            Drop a PDF or image to start
          </div>
          <div style={{ marginTop: 6, fontSize: 12.5, color: "var(--color-text-mid)" }}>
            or <span style={{ color: "var(--color-green)", fontWeight: 600 }}>browse your computer</span>{" "}
            — everything stays on your device
          </div>
        </div>

        {errorMessage && (
          <div style={{ color: "#ff8a80", fontSize: 12.5, textAlign: "center" }}>{errorMessage}</div>
        )}

        <input
          ref={fileInputRef}
          type="file"
          accept="application/pdf,image/*"
          style={{ display: "none" }}
          onChange={(e) => {
            const file = e.target.files?.[0];
            if (file) void openFile(file);
            e.target.value = "";
          }}
        />
      </div>
    </div>
  );
}
