//! Error types for the `PDFree` core engine.

use std::path::PathBuf;

/// Result alias used throughout `pdfree-core`.
pub type Result<T> = std::result::Result<T, PdfError>;

/// Everything that can go wrong inside the PDF engine.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    /// The `PDFium` shared library could not be located or loaded.
    ///
    /// `searched` lists the paths that were tried, in order, so the user can
    /// see exactly where `PDFree` looked before falling back to the system
    /// library. See [`crate::pdfium`] for the discovery strategy.
    #[error("could not load the PDFium library (searched: {searched:?}): {source}")]
    PdfiumUnavailable {
        /// Every path that was tried, in search order.
        searched: Vec<PathBuf>,
        /// The underlying `PDFium` binding error.
        #[source]
        source: pdfium_render::prelude::PdfiumError,
    },

    /// An error surfaced by `PDFium` while working with a document.
    #[error("PDFium error: {0}")]
    Pdfium(#[from] pdfium_render::prelude::PdfiumError),

    /// The requested page index does not exist in the document.
    #[error("page {index} is out of range (document has {count} page(s))")]
    PageOutOfRange {
        /// The page index that was requested.
        index: u16,
        /// The document's actual page count.
        count: u16,
    },

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

    /// [`crate::forms::fill`] was asked to fill a field name that doesn't
    /// exist in the document.
    #[error("no form field named \"{0}\" in this document")]
    UnknownFormField(String),

    /// [`crate::forms::fill`] was asked to fill a field with a
    /// [`crate::forms::FillValue`] that field's kind cannot accept — either
    /// because the value doesn't match the field (a checkbox value for a
    /// text field) or because pdfree-core doesn't support writing that kind
    /// yet (dropdowns, list boxes, radio groups).
    #[error("field \"{name}\" is a {kind:?} field, which cannot be filled with this value")]
    UnsupportedFieldFill {
        /// The field's name.
        name: String,
        /// The field's actual kind.
        kind: crate::forms::FieldKind,
    },

    /// A text overlay was requested with a nonsensical position or size.
    #[error("invalid text overlay: {0}")]
    InvalidOverlay(String),

    /// An annotation was requested with a nonsensical position or size.
    #[error("invalid annotation: {0}")]
    InvalidAnnotation(String),

    /// A signature placement was requested with a nonsensical position or size.
    #[error("invalid signature placement: {0}")]
    InvalidSignaturePlacement(String),

    /// [`crate::pages::merge`] or [`crate::pages::split`] was given an empty
    /// or otherwise nonsensical set of documents/page ranges.
    #[error("invalid page range: {0}")]
    InvalidPageRange(String),

    /// [`crate::pages::reorder`] was given a page list that isn't exactly a
    /// permutation of the document's existing pages.
    #[error("invalid page order: {0}")]
    InvalidPageOrder(String),

    /// [`crate::editor::replace_text`] found no occurrence of the search
    /// text on the given page.
    #[error("no occurrence of {find:?} found on page {page}")]
    TextNotFound {
        /// The page that was searched.
        page: u16,
        /// The search text that wasn't found.
        find: String,
    },

    /// [`crate::encrypt::encrypt_with_password`] needs an external CLI tool
    /// that isn't on `PATH`.
    #[error("required tool `{0}` was not found on PATH — is it installed?")]
    ToolNotFound(&'static str),

    /// The external tool [`crate::encrypt::encrypt_with_password`] shells
    /// out to reported a failure.
    #[error("encryption failed: {0}")]
    EncryptionFailed(String),

    /// [`crate::encrypt::encrypt_with_password`] was given an empty
    /// password.
    #[error("invalid password: {0}")]
    InvalidPassword(String),
}
