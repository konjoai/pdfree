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
    annotations, boxes, convert, editor, forms, pages, pageview, signatures, Document,
    RenderOptions,
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
    #[error("AI provider error: {0}")]
    AiProvider(String),
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

impl From<pdfree_ai::AiError> for PdfFreeError {
    fn from(err: pdfree_ai::AiError) -> Self {
        use pdfree_ai::AiError as E;
        match err {
            E::Core(core_err) => core_err.into(),
            E::Provider(msg) => PdfFreeError::AiProvider(msg),
            E::NotImplemented(what) => PdfFreeError::NotImplemented(what.to_string()),
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

/// Whether a field routes to the sign flow instead of a plain text input,
/// and if so which weight of it. Mirrors `pdfree_core::forms::SignatureFieldKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
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

/// A form field discovered in a document.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FormField {
    pub name: String,
    pub kind: FieldKind,
    pub value: Option<String>,
    /// 0-based page index this field's widget is on.
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

/// A lightweight, local-only audit record captured at sign time — signer
/// name, timestamp, and (where available) device description. Not the
/// deferred certified/legal-grade chain of custody — see
/// `pdfree_core::signatures::SignatureAudit`.
#[derive(Debug, Clone, uniffi::Record)]
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
#[uniffi::export]
pub fn place_signature_with_audit(
    pdf_bytes: Vec<u8>,
    image_png: Vec<u8>,
    at: SignaturePlacement,
    audit: SignatureAudit,
) -> Result<Vec<u8>, PdfFreeError> {
    Ok(signatures::place_signature_with_audit(
        &pdf_bytes,
        &image_png,
        at.into(),
        &audit.into(),
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

/// A page's render plus its detected boxes, from a single bind + parse — see
/// [`pageview`]'s module doc for why loading these separately (as `renderPage`
/// and `boxesOnPage`, each independently binding `PDFium` and re-parsing the
/// whole document) was the actual root cause of slow document open and slow
/// page navigation.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PageView {
    pub png: Vec<u8>,
    pub boxes: Vec<DetectedBox>,
}

impl From<pageview::PageView> for PageView {
    fn from(v: pageview::PageView) -> Self {
        Self {
            png: v.png,
            boxes: v.boxes.into_iter().map(Into::into).collect(),
        }
    }
}

/// Render `page` and detect its fillable boxes together — the call a shell
/// should make instead of separate `renderPage` + `boxesOnPage` calls when it
/// needs both (document open, and every page navigation).
#[uniffi::export]
pub fn page_view(pdf_bytes: Vec<u8>, page: u16, dpi: f32) -> Result<PageView, PdfFreeError> {
    Ok(pageview::page_view(&pdf_bytes, page, dpi)?.into())
}

// ---------------------------------------------------------------------------
// AI (Phase 5): summarize, RAG Q&A, OCR, smart form fill.
//
// Every function here takes an explicit `AiProviderConfig` rather than a
// stored/default provider — CLAUDE.md's "no silent uploads" rule means the
// shell must always know, and choose, whether a given call runs on-device
// (Ollama) or leaves the machine (Anthropic). There is no default provider
// baked in here for that reason.
// ---------------------------------------------------------------------------

/// Which AI backend to run a call against, and its connection details.
/// `Ollama` runs fully on-device; `Anthropic` uploads the prompt (and
/// whatever document text/context it contains) to Anthropic's API.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum AiProviderConfig {
    /// Local inference via an Ollama instance. `base_url` defaults to
    /// `http://localhost:11434` when not given.
    Ollama {
        model: String,
        base_url: Option<String>,
    },
    /// Cloud inference via the Anthropic API. `model` defaults to
    /// `claude-opus-4-8` when not given. Constructing this variant is
    /// itself the user's explicit cloud opt-in — the shell must have
    /// obtained the API key from the user, never a bundled default.
    Anthropic {
        api_key: String,
        model: Option<String>,
    },
}

fn build_ai_provider(config: AiProviderConfig) -> Box<dyn pdfree_ai::provider::Provider> {
    match config {
        AiProviderConfig::Ollama { model, base_url } => match base_url {
            Some(base_url) => Box::new(pdfree_ai::provider::OllamaProvider::with_base_url(
                model, base_url,
            )),
            None => Box::new(pdfree_ai::provider::OllamaProvider::new(model)),
        },
        AiProviderConfig::Anthropic { api_key, model } => match model {
            Some(model) => Box::new(pdfree_ai::provider::AnthropicProvider::with_model(
                api_key, model,
            )),
            None => Box::new(pdfree_ai::provider::AnthropicProvider::new(api_key)),
        },
    }
}

/// Summarize a PDF document.
#[uniffi::export]
pub fn ai_summarize(
    pdf_bytes: Vec<u8>,
    provider: AiProviderConfig,
) -> Result<String, PdfFreeError> {
    let provider = build_ai_provider(provider);
    Ok(pdfree_ai::summarize::summarize(
        &pdf_bytes,
        provider.as_ref(),
    )?)
}

/// Answer a question about a PDF document via retrieval-augmented generation.
#[uniffi::export]
pub fn ai_rag_answer(
    pdf_bytes: Vec<u8>,
    question: String,
    provider: AiProviderConfig,
) -> Result<String, PdfFreeError> {
    let provider = build_ai_provider(provider);
    Ok(pdfree_ai::rag::answer(
        &pdf_bytes,
        &question,
        provider.as_ref(),
    )?)
}

/// Extract text from a scanned page image (PNG bytes) via OCR.
#[uniffi::export]
pub fn ai_ocr_recognize(page_png: Vec<u8>) -> Result<String, PdfFreeError> {
    Ok(pdfree_ai::ocr::recognize(&page_png)?)
}

/// One suggested field fill from [`ai_suggest_form_fills`], ready to pass
/// straight into [`form_fill`] after a user confirms it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SuggestedFormFill {
    pub field_name: String,
    pub value: String,
}

/// Ask the model to map a user's profile (arbitrary key/value pairs — name,
/// email, address, etc.) onto a document's detected `AcroForm` fields.
/// Returns only the fields it found a confident match for; the caller
/// should present these as a reviewable preview, not auto-apply them.
#[uniffi::export]
pub fn ai_suggest_form_fills(
    pdf_bytes: Vec<u8>,
    profile: std::collections::HashMap<String, String>,
    provider: AiProviderConfig,
) -> Result<Vec<SuggestedFormFill>, PdfFreeError> {
    let fields = forms::fields(&pdf_bytes)?;
    let provider = build_ai_provider(provider);
    let suggestions = pdfree_ai::formfill::suggest_fills(&fields, &profile, provider.as_ref())?;
    Ok(suggestions
        .into_iter()
        .map(|s| SuggestedFormFill {
            field_name: s.field_name,
            value: s.value,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// AI (Phase 6): PII redaction, table extraction, document classification.
// ---------------------------------------------------------------------------

/// The kind of PII a [`PiiSpan`] matched. Mirrors `pdfree_ai::redact::PiiKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PiiKind {
    Ssn,
    Email,
    Phone,
    CreditCard,
}

impl From<pdfree_ai::redact::PiiKind> for PiiKind {
    fn from(kind: pdfree_ai::redact::PiiKind) -> Self {
        match kind {
            pdfree_ai::redact::PiiKind::Ssn => PiiKind::Ssn,
            pdfree_ai::redact::PiiKind::Email => PiiKind::Email,
            pdfree_ai::redact::PiiKind::Phone => PiiKind::Phone,
            pdfree_ai::redact::PiiKind::CreditCard => PiiKind::CreditCard,
        }
    }
}

impl From<PiiSpan> for pdfree_ai::redact::PiiSpan {
    fn from(s: PiiSpan) -> Self {
        let kind = match s.kind {
            PiiKind::Ssn => pdfree_ai::redact::PiiKind::Ssn,
            PiiKind::Email => pdfree_ai::redact::PiiKind::Email,
            PiiKind::Phone => pdfree_ai::redact::PiiKind::Phone,
            PiiKind::CreditCard => pdfree_ai::redact::PiiKind::CreditCard,
        };
        Self {
            page: s.page,
            kind,
            text: s.text,
            x: s.x,
            y: s.y,
            width: s.width,
            height: s.height,
        }
    }
}

/// A detected span of personally-identifiable information. Mirrors
/// `pdfree_ai::redact::PiiSpan`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PiiSpan {
    pub page: u16,
    pub kind: PiiKind,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<pdfree_ai::redact::PiiSpan> for PiiSpan {
    fn from(s: pdfree_ai::redact::PiiSpan) -> Self {
        Self {
            page: s.page,
            kind: s.kind.into(),
            text: s.text,
            x: s.x,
            y: s.y,
            width: s.width,
            height: s.height,
        }
    }
}

/// Detect PII (SSNs, emails, phone numbers, credit card numbers) across
/// every page of a document via pattern matching — fully local, no AI
/// provider involved.
#[uniffi::export]
pub fn ai_detect_pii(pdf_bytes: Vec<u8>) -> Result<Vec<PiiSpan>, PdfFreeError> {
    Ok(pdfree_ai::redact::detect_pii(&pdf_bytes)?
        .into_iter()
        .map(Into::into)
        .collect())
}

/// Redact the given PII spans (typically a user-reviewed subset of
/// `ai_detect_pii`'s output), returning the updated document bytes with
/// each span's text overwritten in place.
#[uniffi::export]
pub fn ai_redact(pdf_bytes: Vec<u8>, spans: Vec<PiiSpan>) -> Result<Vec<u8>, PdfFreeError> {
    let spans: Vec<pdfree_ai::redact::PiiSpan> = spans.into_iter().map(Into::into).collect();
    Ok(pdfree_ai::redact::redact(&pdf_bytes, &spans)?)
}

/// A single row of a detected table — see [`ai_extract_tables`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct TableRow {
    pub cells: Vec<String>,
}

/// A table detected on some page of the document — see [`ai_extract_tables`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct Table {
    pub rows: Vec<TableRow>,
}

/// Extract every ruled-line table on every page of a document — fully
/// local, geometry-driven (no AI provider involved).
#[uniffi::export]
pub fn ai_extract_tables(pdf_bytes: Vec<u8>) -> Result<Vec<Table>, PdfFreeError> {
    Ok(pdfree_ai::extract::extract_tables(&pdf_bytes)?
        .into_iter()
        .map(|rows| Table {
            rows: rows.into_iter().map(|cells| TableRow { cells }).collect(),
        })
        .collect())
}

/// Classify a document into a fixed label set (contract, invoice, tax_form,
/// receipt, letter, form, resume, report, other) using an AI provider over
/// its extracted text.
#[uniffi::export]
pub fn ai_classify(pdf_bytes: Vec<u8>, provider: AiProviderConfig) -> Result<String, PdfFreeError> {
    let provider = build_ai_provider(provider);
    Ok(pdfree_ai::classify::classify(
        &pdf_bytes,
        provider.as_ref(),
    )?)
}

// ---------------------------------------------------------------------------
// AI (Phase 7): schema-driven extraction, document diff/redline.
// ---------------------------------------------------------------------------

/// One field to look for — see [`ai_extract_schema`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct SchemaField {
    pub name: String,
    pub description: String,
}

/// A field the model found a value for — see [`ai_extract_schema`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct ExtractedValue {
    pub field_name: String,
    pub value: String,
}

/// Extract caller-defined fields (name + description of what it means) from
/// a document via an AI provider. Returns only the fields it found an
/// actual value for; like `ai_suggest_form_fills`, this is a suggestion
/// list for a review UI, never auto-applied anywhere.
#[uniffi::export]
pub fn ai_extract_schema(
    pdf_bytes: Vec<u8>,
    schema: Vec<SchemaField>,
    provider: AiProviderConfig,
) -> Result<Vec<ExtractedValue>, PdfFreeError> {
    let schema: Vec<pdfree_ai::schema_extract::SchemaField> = schema
        .into_iter()
        .map(|f| pdfree_ai::schema_extract::SchemaField {
            name: f.name,
            description: f.description,
        })
        .collect();
    let provider = build_ai_provider(provider);
    let values = pdfree_ai::schema_extract::extract(&pdf_bytes, &schema, provider.as_ref())?;
    Ok(values
        .into_iter()
        .map(|v| ExtractedValue {
            field_name: v.field_name,
            value: v.value,
        })
        .collect())
}

/// What kind of change a [`TextChange`] represents. Mirrors
/// `pdfree_ai::diff::ChangeKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ChangeKind {
    Unchanged,
    Added,
    Removed,
}

impl From<pdfree_ai::diff::ChangeKind> for ChangeKind {
    fn from(kind: pdfree_ai::diff::ChangeKind) -> Self {
        match kind {
            pdfree_ai::diff::ChangeKind::Unchanged => ChangeKind::Unchanged,
            pdfree_ai::diff::ChangeKind::Added => ChangeKind::Added,
            pdfree_ai::diff::ChangeKind::Removed => ChangeKind::Removed,
        }
    }
}

/// One contiguous run of same-kind words — see [`diff_documents`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct TextChange {
    pub kind: ChangeKind,
    pub text: String,
}

/// The changes found on one page — see [`diff_documents`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct PageDiff {
    pub page: u16,
    pub changes: Vec<TextChange>,
}

/// Diff two versions of a document, page by page, word-level — fully local,
/// geometry/text-driven (no AI provider involved). Pages are aligned by
/// index, not matched by content, so inserting/removing a page in the
/// middle of a document shows every following page as fully changed — see
/// `pdfree_ai::diff`'s module docs for why.
#[uniffi::export]
pub fn diff_documents(
    old_bytes: Vec<u8>,
    new_bytes: Vec<u8>,
) -> Result<Vec<PageDiff>, PdfFreeError> {
    Ok(pdfree_ai::diff::diff_documents(&old_bytes, &new_bytes)?
        .into_iter()
        .map(|d| PageDiff {
            page: d.page,
            changes: d
                .changes
                .into_iter()
                .map(|c| TextChange {
                    kind: c.kind.into(),
                    text: c.text,
                })
                .collect(),
        })
        .collect())
}
