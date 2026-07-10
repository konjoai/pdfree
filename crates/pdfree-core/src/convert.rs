//! Format conversion: PDF <-> Word / image / text (Phase 3).
//!
//! `to_text` and `from_image` are fully implemented — both are squarely
//! within what `PDFium` (a PDF parser/renderer) can do. DOCX conversion in
//! either direction is a different problem entirely: it needs a document
//! *layout* engine (paragraphs, styles, reflow), which `PDFium` doesn't
//! provide and no dependency already in this workspace provides either. See
//! [`to_docx`] and [`from_docx`] for exactly why, and what adopting one would
//! involve.

use pdfium_render::prelude::*;

use crate::error::{PdfError, Result};

/// Extract the plain-text content of every page, joined with a blank line
/// between pages.
///
/// # Errors
///
/// Returns an error if `PDFium` cannot be loaded or the bytes are not a
/// readable PDF.
pub fn to_text(pdf_bytes: &[u8]) -> Result<String> {
    Ok(to_text_per_page(pdf_bytes)?.join("\n\n"))
}

/// Extract the plain-text content of each page separately, index-aligned
/// with the page number. Same underlying extraction as [`to_text`] — this
/// just skips the join, for callers (like `pdfree-ai`'s document diff) that
/// need page boundaries preserved rather than inferring them by splitting
/// `to_text`'s output back apart.
///
/// # Errors
///
/// Returns an error if `PDFium` cannot be loaded or the bytes are not a
/// readable PDF.
pub fn to_text_per_page(pdf_bytes: &[u8]) -> Result<Vec<String>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;

    let mut pages_text = Vec::new();
    for page in document.pages().iter() {
        pages_text.push(page.text()?.all());
    }
    Ok(pages_text)
}

/// Build a single-page PDF containing the given image (PNG, JPEG, or any
/// format the `image` crate reads), filling the page exactly at the given
/// resolution — a page-size PDF page whose dimensions are the image's pixel
/// dimensions converted to PDF points at `dpi`.
///
/// # Errors
///
/// Returns [`PdfError::InvalidRenderTarget`] if `dpi` is not a positive,
/// finite number, and propagates image-decoding / `PDFium` errors otherwise.
pub fn from_image(image_bytes: &[u8], dpi: f32) -> Result<Vec<u8>> {
    if !(dpi.is_finite() && dpi > 0.0) {
        return Err(PdfError::InvalidRenderTarget(format!(
            "dpi must be a positive, finite number (got {dpi})"
        )));
    }

    let image = image::load_from_memory(image_bytes)?;
    let scale = 72.0 / dpi;
    // Image dimensions are pixel counts, always far below f32's 2^24 exact-
    // integer range, so this conversion never actually loses precision.
    #[allow(clippy::cast_precision_loss)]
    let (width, height) = (
        PdfPoints::new(image.width() as f32 * scale),
        PdfPoints::new(image.height() as f32 * scale),
    );

    let pdfium = crate::pdfium::bind()?;
    let mut document = pdfium.create_new_pdf()?;
    let mut page = document
        .pages_mut()
        .create_page_at_end(PdfPagePaperSize::Custom(width, height))?;

    page.objects_mut().create_image_object(
        PdfPoints::ZERO,
        PdfPoints::ZERO,
        &image,
        Some(width),
        Some(height),
    )?;

    Ok(document.save_to_bytes()?)
}

/// Convert a PDF to a DOCX (Word) document.
///
/// # Errors
///
/// Always returns [`PdfError::NotImplemented`]. Faithfully reconstructing a
/// PDF's fixed-layout content (positioned text runs, images, tables built
/// from vector paths) as an *editable, reflowable* DOCX document needs a
/// document layout/reconstruction engine — a fundamentally different piece
/// of software than a PDF renderer. `PDFium` doesn't do this, and nothing
/// else in this workspace does either. This is a real dependency/scope
/// decision (a layout-reconstruction crate, shelling out to a converter
/// service, or accepting a much lower-fidelity "text + basic structure"
/// export), not a small gap like the ones flagged in Phases 1-2 — worth its
/// own entry in `CLAUDE.md`'s open questions before committing to an
/// approach.
pub fn to_docx(_pdf_bytes: &[u8]) -> Result<Vec<u8>> {
    Err(PdfError::NotImplemented("convert::to_docx"))
}

/// Convert a DOCX (Word) document to a PDF.
///
/// # Errors
///
/// Always returns [`PdfError::NotImplemented`]. This needs a DOCX
/// *reader and layout engine* (parse paragraphs/styles/tables, then paginate
/// and render them) — `PDFium` only reads and writes PDF, it has no DOCX
/// support at all. Same scope note as [`to_docx`].
pub fn from_docx(_docx_bytes: &[u8]) -> Result<Vec<u8>> {
    Err(PdfError::NotImplemented("convert::from_docx"))
}
