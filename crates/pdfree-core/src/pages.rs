//! Page operations: merge, split, rotate, extract, reorder (Phase 3).

use crate::error::{PdfError, Result};

/// Merge several PDFs (as byte buffers) into a single document.
///
/// # Errors
///
/// Always returns [`PdfError::NotImplemented`] until Phase 3.
pub fn merge(_documents: &[Vec<u8>]) -> Result<Vec<u8>> {
    Err(PdfError::NotImplemented("pages::merge"))
}

/// Split a PDF into one document per page range.
///
/// # Errors
///
/// Always returns [`PdfError::NotImplemented`] until Phase 3.
pub fn split(_pdf_bytes: &[u8], _ranges: &[(u16, u16)]) -> Result<Vec<Vec<u8>>> {
    Err(PdfError::NotImplemented("pages::split"))
}
