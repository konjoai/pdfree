//! Format conversion: PDF <-> Word / image / text (Phase 3).

use crate::error::{PdfError, Result};

/// Extract the plain-text content of a PDF.
///
/// # Errors
///
/// Always returns [`PdfError::NotImplemented`] until Phase 3.
pub fn to_text(_pdf_bytes: &[u8]) -> Result<String> {
    Err(PdfError::NotImplemented("convert::to_text"))
}
