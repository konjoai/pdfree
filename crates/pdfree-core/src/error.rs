//! Error types for the PDFree core engine.

use std::path::PathBuf;

/// Result alias used throughout `pdfree-core`.
pub type Result<T> = std::result::Result<T, PdfError>;

/// Everything that can go wrong inside the PDF engine.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    /// The PDFium shared library could not be located or loaded.
    ///
    /// `searched` lists the paths that were tried, in order, so the user can
    /// see exactly where PDFree looked before falling back to the system
    /// library. See [`crate::pdfium`] for the discovery strategy.
    #[error("could not load the PDFium library (searched: {searched:?}): {source}")]
    PdfiumUnavailable {
        searched: Vec<PathBuf>,
        #[source]
        source: pdfium_render::prelude::PdfiumError,
    },

    /// An error surfaced by PDFium while working with a document.
    #[error("PDFium error: {0}")]
    Pdfium(#[from] pdfium_render::prelude::PdfiumError),

    /// The requested page index does not exist in the document.
    #[error("page {index} is out of range (document has {count} page(s))")]
    PageOutOfRange { index: u16, count: u16 },

    /// A render was requested with a nonsensical DPI/dimension.
    #[error("invalid render dimensions: {0}")]
    InvalidRenderTarget(String),

    /// Filesystem / IO failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Image encoding/decoding failure.
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    /// A feature exists in the API surface but is not implemented yet.
    ///
    /// Phase 0 ships `document` + `renderer`; later phases fill in the rest.
    #[error("`{0}` is not implemented yet")]
    NotImplemented(&'static str),
}
