//! Signature placement and digital certificate signing (Phase 2).

use crate::error::{PdfError, Result};

/// Where and how big to stamp a signature image, in PDF points.
#[derive(Debug, Clone, Copy)]
pub struct SignaturePlacement {
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Stamp a signature image (PNG bytes) onto a page.
pub fn place_signature(
    _pdf_bytes: &[u8],
    _image_png: &[u8],
    _at: SignaturePlacement,
) -> Result<Vec<u8>> {
    Err(PdfError::NotImplemented("signatures::place_signature"))
}
