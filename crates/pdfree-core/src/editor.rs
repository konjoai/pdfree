//! In-place text and image editing with font matching (Phase 3).

use crate::error::{PdfError, Result};

/// Replace a run of text on a page, matching the original font where possible.
pub fn replace_text(_pdf_bytes: &[u8], _page: u16, _find: &str, _replace: &str) -> Result<Vec<u8>> {
    Err(PdfError::NotImplemented("editor::replace_text"))
}
