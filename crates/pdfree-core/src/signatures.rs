//! Signature placement and digital certificate signing (Phase 2).
//!
//! Two distinct things share this module's name in the PDF world, and this
//! crate only implements one of them so far:
//!
//! - **Visual e-signature** ([`place_signature`]): stamp a signature image
//!   (drawn, typed-and-rendered, or uploaded — the shell decides which) onto
//!   a page at a fixed position. This is what CLAUDE.md's v1 spec calls
//!   "basic e-sign" and is fully implemented here.
//! - **Cryptographic signing** (PKCS#12 / `PAdES` digital certificates): embeds
//!   a PKCS#7 signature blob computed over a byte range of the file, backed
//!   by a private key and certificate chain. `PDFium` doesn't do cryptography
//!   — this needs a real crypto stack (a TLS/PKI crate choice) plus careful
//!   incremental-update PDF surgery to compute the byte range correctly, and
//!   CLAUDE.md lists "v1 = basic e-sign only, or pursue ESIGN/eIDAS from day
//!   one?" as an open question for Wes. Implementing this now would mean
//!   guessing at that decision, so it stays [`PdfError::NotImplemented`]
//!   until it's made.

use pdfium_render::prelude::*;

use crate::error::{PdfError, Result};

/// Where and how big to stamp a signature image, in PDF points (72 per inch,
/// origin at the page's bottom-left corner) — the same convention as
/// [`crate::forms::TextOverlay`].
#[derive(Debug, Clone, Copy)]
pub struct SignaturePlacement {
    /// 0-based page index to stamp onto.
    pub page: u16,
    /// Horizontal position from the page's left edge.
    pub x: f32,
    /// Vertical position from the page's bottom edge.
    pub y: f32,
    /// Width to scale the signature image to.
    pub width: f32,
    /// Height to scale the signature image to.
    pub height: f32,
}

/// Stamp a signature image (PNG bytes) onto a page, returning the updated PDF
/// as new bytes.
///
/// # Errors
///
/// Returns [`PdfError::PageOutOfRange`] if `at.page` doesn't exist,
/// [`PdfError::InvalidSignaturePlacement`] if `at.width`/`at.height` is not a
/// positive finite number, and propagates image-decoding / `PDFium` / load
/// errors otherwise.
pub fn place_signature(
    pdf_bytes: &[u8],
    image_png: &[u8],
    at: SignaturePlacement,
) -> Result<Vec<u8>> {
    let valid_size =
        at.width.is_finite() && at.width > 0.0 && at.height.is_finite() && at.height > 0.0;
    if !valid_size {
        return Err(PdfError::InvalidSignaturePlacement(format!(
            "width/height must be positive, finite numbers (got {}x{})",
            at.width, at.height
        )));
    }

    let image = image::load_from_memory(image_png)?;

    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;

    let count = document.pages().len();
    if at.page >= count {
        return Err(PdfError::PageOutOfRange {
            index: at.page,
            count,
        });
    }

    let mut page = document.pages().get(at.page)?;
    page.objects_mut().create_image_object(
        PdfPoints::new(at.x),
        PdfPoints::new(at.y),
        &image,
        Some(PdfPoints::new(at.width)),
        Some(PdfPoints::new(at.height)),
    )?;

    Ok(document.save_to_bytes()?)
}

/// Sign a document with a PKCS#12 digital certificate (PAdES-style
/// cryptographic signature).
///
/// # Errors
///
/// Always returns [`PdfError::NotImplemented`] — see the module docs for why.
pub fn sign_with_certificate(
    _pdf_bytes: &[u8],
    _pkcs12: &[u8],
    _password: &str,
) -> Result<Vec<u8>> {
    Err(PdfError::NotImplemented(
        "signatures::sign_with_certificate",
    ))
}
