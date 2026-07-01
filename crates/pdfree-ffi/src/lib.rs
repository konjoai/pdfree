//! Native FFI wrapper around `pdfree-core`.
//!
//! This crate is the bridge the Apple (Swift) and future Android (Kotlin)
//! shells bind against. The public interface is frozen in [`pdfree.udl`]; the
//! Rust types below implement it. Phase 4 turns on UniFFI code generation
//! (`uniffi::include_scaffolding!` + `uniffi-bindgen`), which is a mechanical
//! step because the API here already matches the UDL one-for-one.
//!
//! [`pdfree.udl`]: https://github.com/konjoai/pdfree/blob/main/crates/pdfree-ffi/src/pdfree.udl

use std::sync::Arc;

use pdfree_core::{Document, RenderOptions};

/// Errors crossing the FFI boundary. Mirrors the `PdfFreeError` UDL enum.
#[derive(Debug, thiserror::Error)]
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
pub fn version() -> String {
    format!("pdfree-core {}", env!("CARGO_PKG_VERSION"))
}

/// A PDF document, reference-counted so the Swift/Kotlin side can hold it.
pub struct PdfDocument {
    inner: Document,
}

impl PdfDocument {
    /// Load a document from raw bytes.
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

    /// Render page `index` (0-based) to PNG bytes at the given DPI.
    pub fn render_page(&self, index: u16, dpi: u32) -> Result<Vec<u8>, PdfFreeError> {
        let png = self
            .inner
            .render_page(index, &RenderOptions::with_dpi(dpi as f32))?;
        Ok(png)
    }
}
