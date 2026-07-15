//! WebAssembly bindings for `pdfree-core`.
//!
//! Mirrors `pdfree-ffi`'s surface (the same `pdfree-core` functions, the same
//! flat "operate on whole-document byte buffers" call shape) but for the
//! browser: exported via `wasm-bindgen` instead of `uniffi`, with data
//! structs crossing the JS boundary as plain objects via
//! `serde-wasm-bindgen` rather than typed records. `Vec<u8>` byte buffers
//! (PDF bytes, PNG bytes) map directly to/from a JS `Uint8Array` — no serde
//! needed for those.
//!
//! Every JS-facing struct/enum uses `#[serde(rename_all = "camelCase")]` so
//! the JS/TS side sees idiomatic camelCase field names, matching the
//! camelCase method names `wasm-bindgen` already generates for `js_name`-
//! annotated Rust functions.
//!
//! AI features (Phase 5/6, `pdfree-ai`) are **not** exposed here.
//! `pdfree-ai`'s providers use `reqwest::blocking`, which doesn't work in a
//! browser at all — a real web AI integration needs the `fetch` API via
//! `web-sys` and is a separate, not-yet-attempted piece of work (see
//! CLAUDE.md's Phase 4 checklist).

use pdfree_core::{
    annotations, boxes, convert, editor, fields, forms, pages, signatures, Document,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

fn to_js(err: pdfree_core::PdfError) -> JsError {
    JsError::new(&err.to_string())
}

fn to_value<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsError::new(&e.to_string()))
}

fn from_value<T: for<'de> Deserialize<'de>>(value: JsValue) -> Result<T, JsError> {
    serde_wasm_bindgen::from_value(value).map_err(|e| JsError::new(&e.to_string()))
}

/// Library + PDFree version string.
#[wasm_bindgen]
pub fn version() -> String {
    format!("pdfree-core {}", env!("CARGO_PKG_VERSION"))
}

// ---------------------------------------------------------------------------
// Document (Phase 0): open, metadata, render, page size.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSize {
    pub width: f32,
    pub height: f32,
}

/// A PDF document loaded in the browser.
#[wasm_bindgen]
pub struct PdfDocument {
    inner: Document,
}

#[wasm_bindgen]
impl PdfDocument {
    /// Load a document from raw bytes (e.g. a `File`/`ArrayBuffer` from an
    /// `<input type="file">`).
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: Vec<u8>) -> Result<PdfDocument, JsError> {
        let inner = Document::from_bytes(bytes, None).map_err(to_js)?;
        Ok(Self { inner })
    }

    /// Number of pages.
    #[wasm_bindgen(js_name = pageCount)]
    pub fn page_count(&self) -> u16 {
        self.inner.page_count()
    }

    /// Document title, if present.
    pub fn title(&self) -> Option<String> {
        self.inner.metadata().title.clone()
    }

    /// Document author, if present.
    pub fn author(&self) -> Option<String> {
        self.inner.metadata().author.clone()
    }

    /// Document subject, if present.
    pub fn subject(&self) -> Option<String> {
        self.inner.metadata().subject.clone()
    }

    /// Document creator application, if present.
    pub fn creator(&self) -> Option<String> {
        self.inner.metadata().creator.clone()
    }

    /// Document producer application, if present.
    pub fn producer(&self) -> Option<String> {
        self.inner.metadata().producer.clone()
    }

    /// Render page `index` (0-based) to PNG bytes at the given DPI.
    #[wasm_bindgen(js_name = renderPage)]
    pub fn render_page(&self, index: u16, dpi: f32) -> Result<Vec<u8>, JsError> {
        self.inner
            .render_page(index, &pdfree_core::RenderOptions::with_dpi(dpi))
            .map_err(to_js)
    }

    /// Page `index`'s size in PDF points (72/inch), without rendering it.
    /// Pair with [`fit_to_page_dpi`] to compute a default zoom that fits the
    /// whole page in the browser viewport before ever rendering it.
    #[wasm_bindgen(js_name = pageSize)]
    pub fn page_size(&self, index: u16) -> Result<JsValue, JsError> {
        let (width, height) = self.inner.page_size(index).map_err(to_js)?;
        to_value(&PageSize { width, height })
    }
}

/// The DPI that renders a `page_width_pts` x `page_height_pts` page as large
/// as possible while still fitting entirely inside a
/// `viewport_width_px` x `viewport_height_px` viewport — the shared "default
/// view = whole page visible" math every platform shell uses (see Core UX
/// Principles in `CLAUDE.md`).
#[wasm_bindgen(js_name = fitToPageDpi)]
pub fn fit_to_page_dpi(
    page_width_pts: f32,
    page_height_pts: f32,
    viewport_width_px: f32,
    viewport_height_px: f32,
) -> f32 {
    pdfree_core::fit_to_page(
        page_width_pts,
        page_height_pts,
        viewport_width_px,
        viewport_height_px,
    )
    .dpi
}

// ---------------------------------------------------------------------------
// Forms (Phase 1): AcroForm field detection/filling, text overlays.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldKind {
    Text,
    Checkbox,
    RadioButton,
    Dropdown,
    ListBox,
    Signature,
    PushButton,
    Unknown,
}

impl From<forms::FieldKind> for FieldKind {
    fn from(kind: forms::FieldKind) -> Self {
        match kind {
            forms::FieldKind::Text => FieldKind::Text,
            forms::FieldKind::Checkbox => FieldKind::Checkbox,
            forms::FieldKind::RadioButton => FieldKind::RadioButton,
            forms::FieldKind::Dropdown => FieldKind::Dropdown,
            forms::FieldKind::ListBox => FieldKind::ListBox,
            forms::FieldKind::Signature => FieldKind::Signature,
            forms::FieldKind::PushButton => FieldKind::PushButton,
            forms::FieldKind::Unknown => FieldKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureFieldKind {
    None,
    Signature,
    Initials,
}

impl From<forms::SignatureFieldKind> for SignatureFieldKind {
    fn from(kind: forms::SignatureFieldKind) -> Self {
        match kind {
            forms::SignatureFieldKind::None => SignatureFieldKind::None,
            forms::SignatureFieldKind::Signature => SignatureFieldKind::Signature,
            forms::SignatureFieldKind::Initials => SignatureFieldKind::Initials,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    pub name: String,
    pub kind: FieldKind,
    pub value: Option<String>,
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub signature_kind: SignatureFieldKind,
}

impl From<forms::FormField> for FormField {
    fn from(f: forms::FormField) -> Self {
        Self {
            name: f.name,
            kind: f.kind.into(),
            value: f.value,
            page: f.page,
            x: f.x,
            y: f.y,
            width: f.width,
            height: f.height,
            signature_kind: f.signature_kind.into(),
        }
    }
}

/// Enumerate every interactive `AcroForm` field with its kind and current value.
#[wasm_bindgen(js_name = formFields)]
pub fn form_fields(pdf_bytes: Vec<u8>) -> Result<JsValue, JsError> {
    let fields: Vec<FormField> = forms::fields(&pdf_bytes)
        .map_err(to_js)?
        .into_iter()
        .map(Into::into)
        .collect();
    to_value(&fields)
}

/// A value to fill into a named field, scoped to what `pdfree_core::forms`
/// can actually write: text fields and checkboxes.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FillValue {
    Text { value: String },
    Checkbox { checked: bool },
}

impl From<FillValue> for forms::FillValue {
    fn from(v: FillValue) -> Self {
        match v {
            FillValue::Text { value } => forms::FillValue::Text(value),
            FillValue::Checkbox { checked } => forms::FillValue::Checkbox(checked),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldFill {
    pub name: String,
    pub value: FillValue,
}

/// Fill named `AcroForm` fields, returning the updated PDF as new bytes.
#[wasm_bindgen(js_name = formFill)]
pub fn form_fill(pdf_bytes: Vec<u8>, values: JsValue) -> Result<Vec<u8>, JsError> {
    let values: Vec<FieldFill> = from_value(values)?;
    let values: Vec<(String, forms::FillValue)> = values
        .into_iter()
        .map(|f| (f.name, f.value.into()))
        .collect();
    forms::fill(&pdf_bytes, &values).map_err(to_js)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOverlay {
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub font_size: f32,
}

impl From<TextOverlay> for forms::TextOverlay {
    fn from(o: TextOverlay) -> Self {
        Self {
            page: o.page,
            x: o.x,
            y: o.y,
            text: o.text,
            font_size: o.font_size,
        }
    }
}

/// Stamp text onto a non-interactive PDF (a scanned form, a flat template).
#[wasm_bindgen(js_name = overlayText)]
pub fn overlay_text(pdf_bytes: Vec<u8>, overlays: JsValue) -> Result<Vec<u8>, JsError> {
    let overlays: Vec<TextOverlay> = from_value(overlays)?;
    let overlays: Vec<forms::TextOverlay> = overlays.into_iter().map(Into::into).collect();
    forms::overlay_text(&pdf_bytes, &overlays).map_err(to_js)
}

// ---------------------------------------------------------------------------
// Signatures (Phase 2): visual signature placement.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignaturePlacement {
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<SignaturePlacement> for signatures::SignaturePlacement {
    fn from(p: SignaturePlacement) -> Self {
        Self {
            page: p.page,
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
        }
    }
}

/// Stamp a signature image (PNG bytes) onto a page.
#[wasm_bindgen(js_name = placeSignature)]
pub fn place_signature(
    pdf_bytes: Vec<u8>,
    image_png: Vec<u8>,
    at: JsValue,
) -> Result<Vec<u8>, JsError> {
    let at: SignaturePlacement = from_value(at)?;
    signatures::place_signature(&pdf_bytes, &image_png, at.into()).map_err(to_js)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureAudit {
    pub signer_name: String,
    pub signed_at: String,
    pub device_info: Option<String>,
}

impl From<SignatureAudit> for signatures::SignatureAudit {
    fn from(a: SignatureAudit) -> Self {
        Self {
            signer_name: a.signer_name,
            signed_at: a.signed_at,
            device_info: a.device_info,
        }
    }
}

/// Stamp a signature image (PNG bytes) onto a page, plus a small caption
/// beneath it recording who signed and when.
#[wasm_bindgen(js_name = placeSignatureWithAudit)]
pub fn place_signature_with_audit(
    pdf_bytes: Vec<u8>,
    image_png: Vec<u8>,
    at: JsValue,
    audit: JsValue,
) -> Result<Vec<u8>, JsError> {
    let at: SignaturePlacement = from_value(at)?;
    let audit: SignatureAudit = from_value(audit)?;
    signatures::place_signature_with_audit(&pdf_bytes, &image_png, at.into(), &audit.into())
        .map_err(to_js)
}

// ---------------------------------------------------------------------------
// Annotations (Phase 2): highlight, underline, strikeout, sticky notes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl From<AnnotationColor> for annotations::Color {
    fn from(c: AnnotationColor) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
        }
    }
}

impl From<annotations::Color> for AnnotationColor {
    fn from(c: annotations::Color) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnnotationKind {
    Highlight,
    Underline,
    StrikeOut,
    Note,
}

impl From<AnnotationKind> for annotations::AnnotationKind {
    fn from(k: AnnotationKind) -> Self {
        match k {
            AnnotationKind::Highlight => annotations::AnnotationKind::Highlight,
            AnnotationKind::Underline => annotations::AnnotationKind::Underline,
            AnnotationKind::StrikeOut => annotations::AnnotationKind::StrikeOut,
            AnnotationKind::Note => annotations::AnnotationKind::Note,
        }
    }
}

impl From<annotations::AnnotationKind> for AnnotationKind {
    fn from(k: annotations::AnnotationKind) -> Self {
        match k {
            annotations::AnnotationKind::Highlight => AnnotationKind::Highlight,
            annotations::AnnotationKind::Underline => AnnotationKind::Underline,
            annotations::AnnotationKind::StrikeOut => AnnotationKind::StrikeOut,
            annotations::AnnotationKind::Note => AnnotationKind::Note,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub page: u16,
    pub kind: AnnotationKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: Option<AnnotationColor>,
    pub note: Option<String>,
}

impl From<Annotation> for annotations::Annotation {
    fn from(a: Annotation) -> Self {
        Self {
            page: a.page,
            kind: a.kind.into(),
            x: a.x,
            y: a.y,
            width: a.width,
            height: a.height,
            color: a.color.map(Into::into),
            note: a.note,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationInfo {
    pub page: u16,
    pub kind: AnnotationKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: Option<AnnotationColor>,
    pub note: Option<String>,
}

impl From<annotations::AnnotationInfo> for AnnotationInfo {
    fn from(a: annotations::AnnotationInfo) -> Self {
        Self {
            page: a.page,
            kind: a.kind.into(),
            x: a.x,
            y: a.y,
            width: a.width,
            height: a.height,
            color: a.color.map(Into::into),
            note: a.note,
        }
    }
}

/// Add one or more markup/note annotations to a document.
#[wasm_bindgen(js_name = addAnnotations)]
pub fn add_annotations(pdf_bytes: Vec<u8>, annotations_js: JsValue) -> Result<Vec<u8>, JsError> {
    let items: Vec<Annotation> = from_value(annotations_js)?;
    let items: Vec<annotations::Annotation> = items.into_iter().map(Into::into).collect();
    annotations::annotate(&pdf_bytes, &items).map_err(to_js)
}

/// Enumerate the highlight/underline/strikeout/note annotations in a document.
#[wasm_bindgen(js_name = listAnnotations)]
pub fn list_annotations(pdf_bytes: Vec<u8>) -> Result<JsValue, JsError> {
    let items: Vec<AnnotationInfo> = annotations::list(&pdf_bytes)
        .map_err(to_js)?
        .into_iter()
        .map(Into::into)
        .collect();
    to_value(&items)
}

// ---------------------------------------------------------------------------
// Editor (Phase 3): font-preserving in-place text replacement.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRun {
    pub page: u16,
    pub text: String,
    pub font_name: String,
    pub font_size: f32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<editor::TextRun> for TextRun {
    fn from(r: editor::TextRun) -> Self {
        Self {
            page: r.page,
            text: r.text,
            font_name: r.font_name,
            font_size: r.font_size,
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

/// Enumerate every text run on every page, with font and position.
#[wasm_bindgen(js_name = textRuns)]
pub fn text_runs(pdf_bytes: Vec<u8>) -> Result<JsValue, JsError> {
    let runs: Vec<TextRun> = editor::text_runs(&pdf_bytes)
        .map_err(to_js)?
        .into_iter()
        .map(Into::into)
        .collect();
    to_value(&runs)
}

/// Hit-test a point (PDF points) against the text runs on `page`.
#[wasm_bindgen(js_name = textRunAtPoint)]
pub fn text_run_at_point(
    pdf_bytes: Vec<u8>,
    page: u16,
    x: f32,
    y: f32,
) -> Result<JsValue, JsError> {
    let run: Option<TextRun> = editor::text_run_at_point(&pdf_bytes, page, x, y)
        .map_err(to_js)?
        .map(Into::into);
    to_value(&run)
}

/// Replace every occurrence of `find` with `replace` on `page`, in place —
/// the matched text object's own content is mutated, so its font carries
/// over automatically.
#[wasm_bindgen(js_name = replaceText)]
pub fn replace_text(
    pdf_bytes: Vec<u8>,
    page: u16,
    find: String,
    replace: String,
) -> Result<Vec<u8>, JsError> {
    editor::replace_text(&pdf_bytes, page, &find, &replace).map_err(to_js)
}

// ---------------------------------------------------------------------------
// Pages (Phase 3): merge, split, rotate, extract, reorder.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rotation {
    None,
    Clockwise90,
    Clockwise180,
    Clockwise270,
}

impl From<Rotation> for pages::Rotation {
    fn from(r: Rotation) -> Self {
        match r {
            Rotation::None => pages::Rotation::None,
            Rotation::Clockwise90 => pages::Rotation::Clockwise90,
            Rotation::Clockwise180 => pages::Rotation::Clockwise180,
            Rotation::Clockwise270 => pages::Rotation::Clockwise270,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageRange {
    pub start: u16,
    pub end: u16,
}

/// Merge several PDFs (as byte buffers), in order, into a single document.
/// `documents` is a JS array of `Uint8Array`.
#[wasm_bindgen(js_name = mergeDocuments)]
pub fn merge_documents(documents: JsValue) -> Result<Vec<u8>, JsError> {
    let documents: Vec<Vec<u8>> = from_value(documents)?;
    pages::merge(&documents).map_err(to_js)
}

/// Split a PDF into pieces along the given inclusive page ranges. Returns a
/// JS array of `Uint8Array`, one per range.
#[wasm_bindgen(js_name = splitDocument)]
pub fn split_document(pdf_bytes: Vec<u8>, ranges: JsValue) -> Result<JsValue, JsError> {
    let ranges: Vec<PageRange> = from_value(ranges)?;
    let ranges: Vec<(u16, u16)> = ranges.into_iter().map(|r| (r.start, r.end)).collect();
    let pieces = pages::split(&pdf_bytes, &ranges).map_err(to_js)?;
    to_value(&pieces)
}

/// Rotate a single page. `rotation` is one of the [`Rotation`] variant
/// names as a JS string, e.g. `"Clockwise90"`.
#[wasm_bindgen(js_name = rotatePage)]
pub fn rotate_page(pdf_bytes: Vec<u8>, page: u16, rotation: JsValue) -> Result<Vec<u8>, JsError> {
    let rotation: Rotation = from_value(rotation)?;
    pages::rotate(&pdf_bytes, page, rotation.into()).map_err(to_js)
}

/// Pull the given 0-based pages, in exactly the order given, into a new document.
#[wasm_bindgen(js_name = extractPages)]
pub fn extract_pages(pdf_bytes: Vec<u8>, page_list: JsValue) -> Result<Vec<u8>, JsError> {
    let page_list: Vec<u16> = from_value(page_list)?;
    pages::extract(&pdf_bytes, &page_list).map_err(to_js)
}

/// Reorder every page of a document to a full permutation of its indices.
#[wasm_bindgen(js_name = reorderPages)]
pub fn reorder_pages(pdf_bytes: Vec<u8>, new_order: JsValue) -> Result<Vec<u8>, JsError> {
    let new_order: Vec<u16> = from_value(new_order)?;
    pages::reorder(&pdf_bytes, &new_order).map_err(to_js)
}

// ---------------------------------------------------------------------------
// Convert (Phase 3): text extraction, image -> PDF.
// ---------------------------------------------------------------------------

/// Extract plain text from every page, joined in page order.
#[wasm_bindgen(js_name = toText)]
pub fn to_text(pdf_bytes: Vec<u8>) -> Result<String, JsError> {
    convert::to_text(&pdf_bytes).map_err(to_js)
}

/// Wrap an image as a single-page PDF, sized to the image at the given DPI.
#[wasm_bindgen(js_name = fromImage)]
pub fn from_image(image_bytes: Vec<u8>, dpi: f32) -> Result<Vec<u8>, JsError> {
    convert::from_image(&image_bytes, dpi).map_err(to_js)
}

// ---------------------------------------------------------------------------
// Boxes (Phase 4 add-on): click-a-box detection for non-interactive forms.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedBox {
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<boxes::DetectedBox> for DetectedBox {
    fn from(b: boxes::DetectedBox) -> Self {
        Self {
            page: b.page,
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        }
    }
}

/// Find the tightest rectangular box (a drawn form box, or a table cell
/// formed by ruled lines) enclosing `(x, y)` on `page`. Returns `null` if no
/// enclosing box is found — a shell can fall back to a fixed-size overlay in
/// that case.
#[wasm_bindgen(js_name = boxAtPoint)]
pub fn box_at_point(pdf_bytes: Vec<u8>, page: u16, x: f32, y: f32) -> Result<JsValue, JsError> {
    let found: Option<DetectedBox> = boxes::box_at_point(&pdf_bytes, page, x, y)
        .map_err(to_js)?
        .map(Into::into);
    to_value(&found)
}

/// Reconstruct every fillable box (drawn rectangle, or ruled-line table
/// cell) on `page`, meant to be called once as a page loads so a shell can
/// highlight every box up front rather than guessing one at a time from a
/// click point.
#[wasm_bindgen(js_name = boxesOnPage)]
pub fn boxes_on_page(pdf_bytes: Vec<u8>, page: u16) -> Result<JsValue, JsError> {
    let found: Vec<DetectedBox> = boxes::boxes_on_page(&pdf_bytes, page)
        .map_err(to_js)?
        .into_iter()
        .map(Into::into)
        .collect();
    to_value(&found)
}

// ---------------------------------------------------------------------------
// Fields (label-aware fillable-field detection).
// ---------------------------------------------------------------------------

/// Where a [`FillableField`] came from. Mirrors `pdfree_core::fields::FieldSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldSource {
    AcroForm,
    Detected,
}

impl From<fields::FieldSource> for FieldSource {
    fn from(s: fields::FieldSource) -> Self {
        match s {
            fields::FieldSource::AcroForm => FieldSource::AcroForm,
            fields::FieldSource::Detected => FieldSource::Detected,
        }
    }
}

/// One field a shell should present an input affordance for, in PDF points.
/// Mirrors `pdfree_core::fields::FillableField`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillableField {
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: Option<String>,
    pub field_name: Option<String>,
    pub signature_kind: SignatureFieldKind,
    pub source: FieldSource,
}

impl From<fields::FillableField> for FillableField {
    fn from(f: fields::FillableField) -> Self {
        Self {
            page: f.page,
            x: f.x,
            y: f.y,
            width: f.width,
            height: f.height,
            label: f.label,
            field_name: f.field_name,
            signature_kind: f.signature_kind.into(),
            source: f.source.into(),
        }
    }
}

/// Detect every fillable field on `page` a shell should highlight, in a
/// single document parse — the accurate, label-aware replacement for scanning
/// `boxesOnPage` and `formFields` separately. A drawn box with no label next
/// to it is deliberately not reported, and every real `AcroForm` widget is
/// always reported even with no box drawn around it.
#[wasm_bindgen(js_name = fillableFields)]
pub fn fillable_fields(pdf_bytes: Vec<u8>, page: u16) -> Result<JsValue, JsError> {
    let found: Vec<FillableField> = fields::fillable_fields(&pdf_bytes, page)
        .map_err(to_js)?
        .into_iter()
        .map(Into::into)
        .collect();
    to_value(&found)
}
