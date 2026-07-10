import { useRef } from "react";

interface Props {
  hasDocument: boolean;
  fieldCount: number;
  signatureFieldCount: number;
  onOpen: (data: Uint8Array, fileName: string) => void;
  onMerge: (data: Uint8Array) => void;
  onInsertBlankPage: () => void;
  onRotate: () => void;
  onDelete: () => void;
  onExport: () => void;
}

/** The persistent right-hand command surface — mirrors InspectorView.swift:
 * a single "+" add/merge action, then TOOLS/PAGES groups, then a pinned
 * Export button. No File-menu-only actions for core operations (Core UX
 * Principles #5) — there's no menu bar on the web at all, so everything
 * core lives here by construction. */
export function Inspector({
  hasDocument,
  fieldCount,
  signatureFieldCount,
  onOpen,
  onMerge,
  onInsertBlankPage,
  onRotate,
  onDelete,
  onExport,
}: Props) {
  const openInputRef = useRef<HTMLInputElement>(null);
  const mergeInputRef = useRef<HTMLInputElement>(null);

  const pickFile = (input: HTMLInputElement | null, onPick: (data: Uint8Array, name: string) => void) => {
    input?.click();
    const handler = async (e: Event) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) onPick(new Uint8Array(await file.arrayBuffer()), file.name);
      (e.target as HTMLInputElement).value = "";
    };
    if (input) input.onchange = handler;
  };

  return (
    <div
      style={{
        width: "var(--width-inspector)",
        flexShrink: 0,
        display: "flex",
        flexDirection: "column",
        gap: 15,
        padding: "18px 16px",
        background: "var(--color-panel-bg)",
        borderLeft: "1px solid var(--color-hairline-faint)",
      }}
    >
      <input ref={openInputRef} type="file" accept="application/pdf,image/*" style={{ display: "none" }} />
      <input ref={mergeInputRef} type="file" accept="application/pdf" style={{ display: "none" }} />

      <button
        onClick={() => pickFile(openInputRef.current, onOpen)}
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          gap: 8,
          height: 40,
          borderRadius: "var(--radius-button)",
          border: "none",
          fontWeight: 600,
          fontSize: 13.5,
          cursor: "pointer",
          background: hasDocument ? "var(--color-field-fill-wash)" : "var(--color-green)",
          color: hasDocument ? "var(--color-green-badge-text)" : "var(--color-green-ink)",
        }}
      >
        + Add or merge
      </button>

      {hasDocument && (
        <div style={{ display: "flex", flexDirection: "column", gap: 2, opacity: hasDocument ? 1 : 0.45 }}>
          <SectionLabel text="TOOLS" />
          <Row label="Fill fields" badge={fieldCount > 0 ? String(fieldCount) : undefined} />
          <Row label="Sign" badge={signatureFieldCount > 0 ? String(signatureFieldCount) : undefined} amber />

          <div style={{ height: 10 }} />
          <SectionLabel text="PAGES" />
          <Row label="Merge PDF…" onClick={() => pickFile(mergeInputRef.current, onMerge)} />
          <Row label="Insert blank page" onClick={onInsertBlankPage} />
          <Row label="Rotate page" onClick={onRotate} />
          <Row label="Delete page" onClick={onDelete} />
        </div>
      )}

      <div style={{ flex: 1 }} />

      <button
        onClick={onExport}
        disabled={!hasDocument}
        style={{
          height: 44,
          borderRadius: 11,
          border: "none",
          fontWeight: 700,
          fontSize: 14,
          cursor: hasDocument ? "pointer" : "default",
          opacity: hasDocument ? 1 : 0.45,
          background: "var(--color-green)",
          color: "var(--color-green-ink)",
        }}
      >
        Export
      </button>
      <div style={{ textAlign: "center", fontSize: 10.5, color: "var(--color-text-low)" }}>
        No watermark · no limits · runs in your browser
      </div>
    </div>
  );
}

function SectionLabel({ text }: { text: string }) {
  return (
    <div style={{ fontSize: 10, fontWeight: 600, letterSpacing: 1.2, color: "var(--color-text-low)", padding: "2px 4px 8px" }}>
      {text}
    </div>
  );
}

function Row({ label, badge, amber, onClick }: { label: string; badge?: string; amber?: boolean; onClick?: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "10px 11px",
        borderRadius: "var(--radius-row)",
        border: "none",
        background: "transparent",
        color: "var(--color-text-row)",
        fontSize: 13.5,
        cursor: onClick ? "pointer" : "default",
        textAlign: "left",
      }}
    >
      <span>{label}</span>
      {badge && (
        <span
          style={{
            fontSize: 11,
            fontWeight: 600,
            padding: "2px 8px",
            borderRadius: 999,
            color: amber ? "var(--color-amber-text)" : "var(--color-green-badge-text)",
            background: amber ? "rgba(232,180,90,0.22)" : "rgba(55,192,122,0.22)",
          }}
        >
          {badge}
        </span>
      )}
    </button>
  );
}
