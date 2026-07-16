// Mirrors the JSON shapes pdfree-wasm's serde-wasm-bindgen structs produce
// (crates/pdfree-wasm/src/lib.rs) — field names are camelCase (the structs
// use #[serde(rename_all = "camelCase")]), enum values are the exact Rust
// variant names (PascalCase), kept in sync by hand since wasm-bindgen only
// generates typed signatures for its own `#[wasm_bindgen]` items, not for
// the serde-shaped payloads carried as `any`/`JsValue`.

export type FieldKind =
  | "Text"
  | "Checkbox"
  | "RadioButton"
  | "Dropdown"
  | "ListBox"
  | "Signature"
  | "PushButton"
  | "Unknown";

export type SignatureFieldKind = "None" | "Signature" | "Initials";

export interface FormField {
  name: string;
  kind: FieldKind;
  value: string | null;
  page: number;
  x: number;
  y: number;
  width: number;
  height: number;
  signatureKind: SignatureFieldKind;
  /** Position within its radio button group when `kind` is `Dropdown`'s
   * sibling `RadioButton` kind — `null` otherwise. Pass back via
   * `FillValue`'s `Radio` variant to select this specific option. */
  radioGroupIndex: number | null;
}

export type FillValue =
  | { type: "Text"; value: string }
  | { type: "Checkbox"; checked: boolean };

export interface FieldFill {
  name: string;
  value: FillValue;
}

export interface TextOverlay {
  page: number;
  x: number;
  y: number;
  text: string;
  fontSize: number;
}

export interface SignaturePlacement {
  page: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface SignatureAudit {
  signerName: string;
  signedAt: string;
  deviceInfo: string | null;
}

export type AnnotationKind = "Highlight" | "Underline" | "StrikeOut" | "Note";

export interface AnnotationColor {
  r: number;
  g: number;
  b: number;
}

export interface Annotation {
  page: number;
  kind: AnnotationKind;
  x: number;
  y: number;
  width: number;
  height: number;
  color: AnnotationColor | null;
  note: string | null;
}

export interface AnnotationInfo extends Annotation {}

export interface TextRun {
  page: number;
  text: string;
  fontName: string;
  fontSize: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

export type Rotation =
  | "None"
  | "Clockwise90"
  | "Clockwise180"
  | "Clockwise270";

export interface PageRange {
  start: number;
  end: number;
}

export interface DetectedBox {
  page: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PageSize {
  width: number;
  height: number;
}

/** A scanned box paired with whatever's known about the `AcroForm` field
 * underneath it — mirrors `PDFDocumentStore.FieldOverlayBox` on macOS, so
 * the canvas can style signature/initials fields distinctly (Core UX
 * Principles: "signature/initials fields are special-cased"). */
export interface FieldOverlay {
  box: DetectedBox;
  signatureKind: SignatureFieldKind;
  fieldName: string | null;
}
