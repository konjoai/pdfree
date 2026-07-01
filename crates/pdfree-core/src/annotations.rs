//! Highlight, underline, strikethrough, and sticky-note annotations (Phase 2).

use crate::error::{PdfError, Result};

/// A standard PDF markup annotation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKind {
    Highlight,
    Underline,
    StrikeOut,
    Note,
}

/// Add a markup annotation to a page.
pub fn annotate(_pdf_bytes: &[u8], _page: u16, _kind: AnnotationKind) -> Result<Vec<u8>> {
    Err(PdfError::NotImplemented("annotations::annotate"))
}
