//! Native FFI wrapper around `pdfree-core`, exported to Swift/Kotlin via
//! UniFFI's proc-macro mode (`#[uniffi::export]`) — no `.udl` file. The
//! interface is derived straight from this file, so it can't drift out of
//! sync with `pdfree-core` the way a hand-maintained UDL can.
//!
//! Mirrors `docs/api.md` one module at a time. Every fallible operation here
//! works on whole-document byte buffers, same as `pdfree-core` itself, so the
//! same call shape works whether the bytes came from a file, a picker, or a
//! network fetch on the Swift side.

uniffi::setup_scaffolding!();

use std::sync::Arc;

use pdfree_core::{
    annotations, boxes, convert, editor, forms, pages, signatures, Document, RenderOptions,
};

/// A page's size in PDF points (72/inch).
#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct PageSize {
    pub width: f32,
    pub height: f32,
}

/// Errors crossing the FFI boundary. `flat_error` means Swift only sees each
/// variant's message (via `Display`), not its payload shape — simplest thing
/// that lets every `pdfree_core::PdfError` variant map over without also
/// having to make every payload type UniFFI-compatible.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum PdfFreeError {
    #[error("invalid document: {0}")]
    InvalidDocument(String),
    #[error("render failed: {0}")]
    Render(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
}

impl From<pdfree_core::PdfError> for PdfFreeError {
    fn from(err: pdfree_core::PdfError) -> Self {
        use pdfree_core::PdfError as E;
        match err {
            E::NotImplemented(what) => PdfFreeError::NotImplemented(what.to_string()),
            E::PageOutOfRange { .. } | E::InvalidRenderTarget(_) => {
                PdfFreeError::Render(err.to_string())
            }
            other => PdfFreeError::InvalidDocument(other.to_string()),
        }
    }
}

/// Library + PDFree version string.
#[uniffi::export]
pub fn version() -> String {
    format!("pdfree-core {}", env!("CARGO_PKG_VERSION"))
}

/// A PDF document, reference-counted so the Swift/Kotlin side can hold it.
#[derive(uniffi::Object)]
pub struct PdfDocument {
    inner: Document,
}

#[uniffi::export]
impl PdfDocument {
    /// Load a document from raw bytes.
    #[uniffi::constructor]
    pub fn from_bytes(data: Vec<u8>) -> Result<Arc<Self>, PdfFreeError> {
        let inner = Document::from_bytes(data, None)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Number of pages.
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
    pub fn render_page(&self, index: u16, dpi: u32) -> Result<Vec<u8>, PdfFreeError> {
        let png = self
            .inner
            .render_page(index, &RenderOptions::with_dpi(dpi as f32))?;
        Ok(png)
    }

    /// Page `index`'s size in PDF points (72/inch), without rendering it.
    /// Pair with [`fit_to_page_dpi`] to compute a default zoom that fits the
    /// whole page in a shell's viewport before ever rendering it.
    pub fn page_size(&self, index: u16) -> Result<PageSize, PdfFreeError> {
        let (width, height) = self.inner.page_size(index)?;
        Ok(PageSize { width, height })
    }
}

/// The DPI that renders a `page_width_pts` × `page_height_pts` page as large
/// as possible while still fitting entirely inside a
/// `viewport_width_px` × `viewport_height_px` viewport — the shared "default
/// view = whole page visible" math every platform shell should use (see
/// Core UX Principles in `CLAUDE.md`), so the fit is computed identically on
/// macOS, web, Tauri, and iOS instead of each back-computing its own.
#[uniffi::export]
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

/// The kind of an `AcroForm` field. Mirrors `pdfree_core::forms::FieldKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
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

/// A form field discovered in a document.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FormField {
    pub name: String,
    pub kind: FieldKind,
    pub value: Option<String>,
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
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
        }
    }
}

/// A value to fill into a named field, scoped to what `pdfree_core::forms`
/// can actually write: text fields and checkboxes.
#[derive(Debug, Clone, uniffi::Enum)]
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

/// One field to fill, by name.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FieldFill {
    pub name: String,
    pub value: FillValue,
}

/// Enumerate every interactive `AcroForm` field with its kind and current value.
#[uniffi::export]
pub fn form_fields(pdf_bytes: Vec<u8>) -> Result<Vec<FormField>, PdfFreeError> {
    Ok(forms::fields(&pdf_bytes)?
        .into_iter()
        .map(Into::into)
        .collect())
}

/// Fill named `AcroForm` fields, returning the updated PDF as new bytes.
#[uniffi::export]
pub fn form_fill(pdf_bytes: Vec<u8>, values: Vec<FieldFill>) -> Result<Vec<u8>, PdfFreeError> {
    let values: Vec<(String, forms::FillValue)> = values
        .into_iter()
        .map(|f| (f.name, f.value.into()))
        .collect();
    Ok(forms::fill(&pdf_bytes, &values)?)
}

/// One text stamp to overlay on a non-interactive PDF page.
#[derive(Debug, Clone, uniffi::Record)]
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
#[uniffi::export]
pub fn overlay_text(
    pdf_bytes: Vec<u8>,
    overlays: Vec<TextOverlay>,
) -> Result<Vec<u8>, PdfFreeError> {
    let overlays: Vec<forms::TextOverlay> = overlays.into_iter().map(Into::into).collect();
    Ok(forms::overlay_text(&pdf_bytes, &overlays)?)
}

// ---------------------------------------------------------------------------
// Signatures (Phase 2): visual signature placement.
// ---------------------------------------------------------------------------

/// Where and how big to stamp a signature image, in PDF points.
#[derive(Debug, Clone, Copy, uniffi::Record)]
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
#[uniffi::export]
pub fn place_signature(
    pdf_bytes: Vec<u8>,
    image_png: Vec<u8>,
    at: SignaturePlacement,
) -> Result<Vec<u8>, PdfFreeError> {
    Ok(signatures::place_signature(
        &pdf_bytes,
        &image_png,
        at.into(),
    )?)
}

// ---------------------------------------------------------------------------
// Annotations (Phase 2): highlight, underline, strikeout, sticky notes.
// ---------------------------------------------------------------------------

/// An RGB color, 0-255 per channel. Named `AnnotationColor` rather than
/// `Color` — a bare `Color` record collides with `SwiftUI.Color` once this
/// crate's generated bindings and SwiftUI are imported into the same Swift
/// module (as `apps/macos` does), silently shadowing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
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

/// A standard PDF markup annotation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
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

/// One annotation to add to a page, in PDF points.
#[derive(Debug, Clone, uniffi::Record)]
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

/// One annotation read back from a document, as reported by [`list_annotations`].
#[derive(Debug, Clone, uniffi::Record)]
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
#[uniffi::export]
pub fn add_annotations(
    pdf_bytes: Vec<u8>,
    annotations: Vec<Annotation>,
) -> Result<Vec<u8>, PdfFreeError> {
    let annotations: Vec<pdfree_core::annotations::Annotation> =
        annotations.into_iter().map(Into::into).collect();
    Ok(pdfree_core::annotations::annotate(
        &pdf_bytes,
        &annotations,
    )?)
}

/// Enumerate the highlight/underline/strikeout/note annotations in a document.
#[uniffi::export]
pub fn list_annotations(pdf_bytes: Vec<u8>) -> Result<Vec<AnnotationInfo>, PdfFreeError> {
    Ok(annotations::list(&pdf_bytes)?
        .into_iter()
        .map(Into::into)
        .collect())
}

// ---------------------------------------------------------------------------
// Editor (Phase 3): font-preserving in-place text replacement.
// ---------------------------------------------------------------------------

/// One run of text on a page, with its font and position.
#[derive(Debug, Clone, uniffi::Record)]
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
#[uniffi::export]
pub fn text_runs(pdf_bytes: Vec<u8>) -> Result<Vec<TextRun>, PdfFreeError> {
    Ok(editor::text_runs(&pdf_bytes)?
        .into_iter()
        .map(Into::into)
        .collect())
}

/// Hit-test a point (PDF points) against the text runs on `page`.
#[uniffi::export]
pub fn text_run_at_point(
    pdf_bytes: Vec<u8>,
    page: u16,
    x: f32,
    y: f32,
) -> Result<Option<TextRun>, PdfFreeError> {
    Ok(editor::text_run_at_point(&pdf_bytes, page, x, y)?.map(Into::into))
}

/// Replace every occurrence of `find` with `replace` on `page`, in place —
/// the matched text object's own content is mutated, so its font carries
/// over automatically.
#[uniffi::export]
pub fn replace_text(
    pdf_bytes: Vec<u8>,
    page: u16,
    find: String,
    replace: String,
) -> Result<Vec<u8>, PdfFreeError> {
    Ok(editor::replace_text(&pdf_bytes, page, &find, &replace)?)
}

// ---------------------------------------------------------------------------
// Pages (Phase 3): merge, split, rotate, extract, reorder.
// ---------------------------------------------------------------------------

/// How far to rotate a page, clockwise, from its current orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
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

/// An inclusive, 0-based page range, e.g. `{start: 0, end: 2}` is pages 0-2.
#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct PageRange {
    pub start: u16,
    pub end: u16,
}

/// Merge several PDFs (as byte buffers), in order, into a single document.
#[uniffi::export]
pub fn merge_documents(documents: Vec<Vec<u8>>) -> Result<Vec<u8>, PdfFreeError> {
    Ok(pages::merge(&documents)?)
}

/// Split a PDF into pieces along the given inclusive page ranges.
#[uniffi::export]
pub fn split_document(
    pdf_bytes: Vec<u8>,
    ranges: Vec<PageRange>,
) -> Result<Vec<Vec<u8>>, PdfFreeError> {
    let ranges: Vec<(u16, u16)> = ranges.into_iter().map(|r| (r.start, r.end)).collect();
    Ok(pages::split(&pdf_bytes, &ranges)?)
}

/// Rotate a single page.
#[uniffi::export]
pub fn rotate_page(
    pdf_bytes: Vec<u8>,
    page: u16,
    rotation: Rotation,
) -> Result<Vec<u8>, PdfFreeError> {
    Ok(pages::rotate(&pdf_bytes, page, rotation.into())?)
}

/// Pull the given 0-based pages, in exactly the order given, into a new document.
#[uniffi::export]
pub fn extract_pages(pdf_bytes: Vec<u8>, pages: Vec<u16>) -> Result<Vec<u8>, PdfFreeError> {
    Ok(pdfree_core::pages::extract(&pdf_bytes, &pages)?)
}

/// Reorder every page of a document to a full permutation of its indices.
#[uniffi::export]
pub fn reorder_pages(pdf_bytes: Vec<u8>, new_order: Vec<u16>) -> Result<Vec<u8>, PdfFreeError> {
    Ok(pdfree_core::pages::reorder(&pdf_bytes, &new_order)?)
}

// ---------------------------------------------------------------------------
// Convert (Phase 3): text extraction, image -> PDF.
// ---------------------------------------------------------------------------

/// Extract plain text from every page, joined in page order.
#[uniffi::export]
pub fn to_text(pdf_bytes: Vec<u8>) -> Result<String, PdfFreeError> {
    Ok(convert::to_text(&pdf_bytes)?)
}

/// Wrap an image as a single-page PDF, sized to the image at the given DPI.
#[uniffi::export]
pub fn from_image(image_bytes: Vec<u8>, dpi: f32) -> Result<Vec<u8>, PdfFreeError> {
    Ok(convert::from_image(&image_bytes, dpi)?)
}

// ---------------------------------------------------------------------------
// Boxes (Phase 4 add-on): click-a-box detection for non-interactive forms.
// ---------------------------------------------------------------------------

/// A detected rectangular box, in PDF points.
#[derive(Debug, Clone, Copy, uniffi::Record)]
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
/// formed by ruled lines) enclosing `(x, y)` on `page`. Returns `None` if no
/// enclosing box is found — a shell can fall back to a fixed-size overlay in
/// that case.
#[uniffi::export]
pub fn box_at_point(
    pdf_bytes: Vec<u8>,
    page: u16,
    x: f32,
    y: f32,
) -> Result<Option<DetectedBox>, PdfFreeError> {
    Ok(boxes::box_at_point(&pdf_bytes, page, x, y)?.map(Into::into))
}

/// Reconstruct every fillable box (drawn rectangle, or ruled-line table
/// cell) on `page`, meant to be called once as a page loads so a shell can
/// highlight every box up front rather than guessing one at a time from a
/// click point.
#[uniffi::export]
pub fn boxes_on_page(pdf_bytes: Vec<u8>, page: u16) -> Result<Vec<DetectedBox>, PdfFreeError> {
    Ok(boxes::boxes_on_page(&pdf_bytes, page)?
        .into_iter()
        .map(Into::into)
        .collect())
}
