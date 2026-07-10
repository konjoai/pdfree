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

/// Scale the placement rect down so the image fits *entirely inside* it while
/// preserving the image's aspect ratio, then center it — so a signature is
/// never stretched, squished, or cut off by a block whose proportions differ
/// from the drawn/typed/uploaded mark's. Returns `(x, y, width, height)` in
/// PDF points. Never scales *up* past the block, so a small mark stays small
/// rather than ballooning to fill a large field.
fn fit_within(at: &SignaturePlacement, image_w: u32, image_h: u32) -> (f32, f32, f32, f32) {
    if image_w == 0 || image_h == 0 {
        return (at.x, at.y, at.width, at.height);
    }
    let box_ratio = at.width / at.height;
    let image_ratio = image_w as f32 / image_h as f32;
    let (w, h) = if image_ratio >= box_ratio {
        // Image is wider than the block: width-constrained.
        (at.width, at.width / image_ratio)
    } else {
        // Image is taller than the block: height-constrained.
        (at.height * image_ratio, at.height)
    };
    let x = at.x + (at.width - w) / 2.0;
    let y = at.y + (at.height - h) / 2.0;
    (x, y, w, h)
}

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

    let (fx, fy, fw, fh) = fit_within(&at, image.width(), image.height());
    let mut page = document.pages().get(at.page)?;
    page.objects_mut().create_image_object(
        PdfPoints::new(fx),
        PdfPoints::new(fy),
        &image,
        Some(PdfPoints::new(fw)),
        Some(PdfPoints::new(fh)),
    )?;

    Ok(document.save_to_bytes()?)
}

/// A lightweight, local-only audit record captured at sign time — signer
/// name, a timestamp, and (where available) a device description. This is
/// deliberately *not* the deferred certified/legal-grade chain of custody
/// (ESIGN/eIDAS-style hash chains, multi-party routing — see CLAUDE.md's
/// "Signature legal validity" open question and Potential Paid Features);
/// it's just enough for "who signed this and when" to travel with the file.
///
/// `pdfree-core` stays clock/locale-free by design (see [`Document`] and
/// [`crate::renderer`]'s existing conventions) — the caller formats
/// `signed_at` however its platform/locale prefers.
///
/// [`Document`]: crate::document::Document
#[derive(Debug, Clone)]
pub struct SignatureAudit {
    /// Display name of the signer, as entered by the user.
    pub signer_name: String,
    /// A pre-formatted, human-readable timestamp.
    pub signed_at: String,
    /// Optional device/platform description (e.g. `"macOS 14.5"`).
    pub device_info: Option<String>,
}

/// Font size, in points, of the audit caption stamped by
/// [`place_signature_with_audit`].
const AUDIT_CAPTION_FONT_SIZE: f32 = 6.5;
/// Vertical gap, in points, between the signature image's bottom edge and
/// the audit caption above the page's bottom margin.
const AUDIT_CAPTION_GAP: f32 = 2.0;
/// Never places the caption's baseline below this many points from the
/// page's bottom edge, so a signature stamped near the very bottom of a
/// page doesn't push its caption off the page entirely.
const AUDIT_CAPTION_MIN_Y: f32 = 2.0;

/// Same as [`place_signature`], but also stamps a small caption directly
/// beneath the image recording who signed and when — see [`SignatureAudit`].
///
/// # Errors
///
/// Same as [`place_signature`].
pub fn place_signature_with_audit(
    pdf_bytes: &[u8],
    image_png: &[u8],
    at: SignaturePlacement,
    audit: &SignatureAudit,
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
    let mut document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;

    let count = document.pages().len();
    if at.page >= count {
        return Err(PdfError::PageOutOfRange {
            index: at.page,
            count,
        });
    }

    let (fx, fy, fw, fh) = fit_within(&at, image.width(), image.height());
    let font = document.fonts_mut().helvetica();
    let caption = audit_caption(audit);
    // Caption sits just below the *fitted* image, not the (possibly taller)
    // block, so it hugs the signature rather than floating in dead space.
    let caption_y = (fy - AUDIT_CAPTION_FONT_SIZE - AUDIT_CAPTION_GAP).max(AUDIT_CAPTION_MIN_Y);

    let mut page = document.pages().get(at.page)?;
    page.objects_mut().create_image_object(
        PdfPoints::new(fx),
        PdfPoints::new(fy),
        &image,
        Some(PdfPoints::new(fw)),
        Some(PdfPoints::new(fh)),
    )?;
    page.objects_mut().create_text_object(
        PdfPoints::new(fx),
        PdfPoints::new(caption_y),
        caption.as_str(),
        font,
        PdfPoints::new(AUDIT_CAPTION_FONT_SIZE),
    )?;

    Ok(document.save_to_bytes()?)
}

fn audit_caption(audit: &SignatureAudit) -> String {
    match &audit.device_info {
        Some(device) if !device.is_empty() => {
            format!(
                "Signed by {} · {} · {device}",
                audit.signer_name, audit.signed_at
            )
        }
        _ => format!("Signed by {} · {}", audit.signer_name, audit.signed_at),
    }
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
