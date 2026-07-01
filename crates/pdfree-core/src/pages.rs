//! Page operations: merge, split, rotate, extract, reorder (Phase 3).
//!
//! Every operation here builds a fresh in-memory `PDFium` document and copies
//! pages into it via `PDFium`'s own page-import machinery (`FPDF_ImportPages`),
//! rather than manipulating page trees directly — the same mechanism a tool
//! like `pdftk` relies on, so page resources (fonts, images) come along
//! correctly instead of dangling.

use pdfium_render::prelude::*;

use crate::error::{PdfError, Result};

/// How far to rotate a page, clockwise, from its current orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    /// No rotation.
    None,
    /// Rotate 90 degrees clockwise.
    Clockwise90,
    /// Rotate 180 degrees.
    Clockwise180,
    /// Rotate 270 degrees clockwise (90 counter-clockwise).
    Clockwise270,
}

impl Rotation {
    fn to_pdfium(self) -> PdfPageRenderRotation {
        match self {
            Rotation::None => PdfPageRenderRotation::None,
            Rotation::Clockwise90 => PdfPageRenderRotation::Degrees90,
            Rotation::Clockwise180 => PdfPageRenderRotation::Degrees180,
            Rotation::Clockwise270 => PdfPageRenderRotation::Degrees270,
        }
    }
}

/// Merge several PDFs (as byte buffers), in order, into a single document.
///
/// # Errors
///
/// Returns [`PdfError::InvalidPageRange`] if `documents` is empty, and
/// propagates `PDFium` / load errors otherwise.
pub fn merge(documents: &[Vec<u8>]) -> Result<Vec<u8>> {
    if documents.is_empty() {
        return Err(PdfError::InvalidPageRange(
            "merge requires at least one document".to_string(),
        ));
    }

    let pdfium = crate::pdfium::bind()?;
    let mut merged = pdfium.create_new_pdf()?;

    for bytes in documents {
        let source = pdfium.load_pdf_from_byte_slice(bytes, None)?;
        merged.pages_mut().append(&source)?;
    }

    Ok(merged.save_to_bytes()?)
}

/// Split a PDF into one document per page range. Each range is an inclusive,
/// 0-based `(start, end)` pair.
///
/// # Errors
///
/// Returns [`PdfError::InvalidPageRange`] if `ranges` is empty or any range
/// is inverted (`start > end`), [`PdfError::PageOutOfRange`] if a range
/// extends past the document's last page, and propagates `PDFium` / load
/// errors otherwise.
pub fn split(pdf_bytes: &[u8], ranges: &[(u16, u16)]) -> Result<Vec<Vec<u8>>> {
    if ranges.is_empty() {
        return Err(PdfError::InvalidPageRange(
            "split requires at least one page range".to_string(),
        ));
    }

    let pdfium = crate::pdfium::bind()?;
    let source = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    let count = source.pages().len();

    let mut out = Vec::with_capacity(ranges.len());
    for &(start, end) in ranges {
        if start > end {
            return Err(PdfError::InvalidPageRange(format!(
                "range start {start} is after end {end}"
            )));
        }
        if end >= count {
            return Err(PdfError::PageOutOfRange { index: end, count });
        }

        let mut piece = pdfium.create_new_pdf()?;
        piece
            .pages_mut()
            .copy_page_range_from_document(&source, start..=end, 0)?;
        out.push(piece.save_to_bytes()?);
    }

    Ok(out)
}

/// Rotate one page, returning the updated PDF as new bytes.
///
/// # Errors
///
/// Returns [`PdfError::PageOutOfRange`] if `page` doesn't exist, and
/// propagates `PDFium` / load errors otherwise.
pub fn rotate(pdf_bytes: &[u8], page: u16, rotation: Rotation) -> Result<Vec<u8>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    let count = document.pages().len();
    if page >= count {
        return Err(PdfError::PageOutOfRange { index: page, count });
    }

    let mut target = document.pages().get(page)?;
    target.set_rotation(rotation.to_pdfium());

    Ok(document.save_to_bytes()?)
}

/// Extract the given 0-based page indices, in the given order, into a new
/// document. Indices may repeat or be given in any order — this is also how
/// [`reorder`] is implemented.
///
/// # Errors
///
/// Returns [`PdfError::InvalidPageRange`] if `pages` is empty,
/// [`PdfError::PageOutOfRange`] if an index doesn't exist, and propagates
/// `PDFium` / load errors otherwise.
pub fn extract(pdf_bytes: &[u8], pages: &[u16]) -> Result<Vec<u8>> {
    let pdfium = crate::pdfium::bind()?;
    extract_with(&pdfium, pdf_bytes, pages)
}

/// Shared implementation for [`extract`] and [`reorder`].
///
/// Takes an already-bound [`Pdfium`] rather than binding its own: `PDFium`
/// does not tolerate two independent bindings to the shared library being
/// live at once in the same process (confirmed empirically — nesting two
/// [`crate::pdfium::bind`] calls hangs), so every call chain within this
/// crate binds exactly once at its public entry point and threads the
/// binding through.
fn extract_with(pdfium: &Pdfium, pdf_bytes: &[u8], pages: &[u16]) -> Result<Vec<u8>> {
    if pages.is_empty() {
        return Err(PdfError::InvalidPageRange(
            "extract requires at least one page".to_string(),
        ));
    }

    let source = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    let count = source.pages().len();

    for &index in pages {
        if index >= count {
            return Err(PdfError::PageOutOfRange { index, count });
        }
    }

    // FPDF_ImportPages takes a 1-based, comma-separated page list and
    // imports pages in exactly the order given, so this one primitive
    // handles both "pull these pages out" and "put them in this order".
    let page_list = pages
        .iter()
        .map(|index| (index + 1).to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut extracted = pdfium.create_new_pdf()?;
    extracted
        .pages_mut()
        .copy_pages_from_document(&source, &page_list, 0)?;

    Ok(extracted.save_to_bytes()?)
}

/// Reorder every page in a document to match `new_order`, a permutation of
/// `0..page_count`.
///
/// # Errors
///
/// Returns [`PdfError::InvalidPageOrder`] if `new_order` isn't exactly a
/// permutation of the document's existing pages (wrong length, an
/// out-of-range index, or a duplicate), and propagates `PDFium` / load
/// errors otherwise.
pub fn reorder(pdf_bytes: &[u8], new_order: &[u16]) -> Result<Vec<u8>> {
    let pdfium = crate::pdfium::bind()?;
    let count = pdfium
        .load_pdf_from_byte_slice(pdf_bytes, None)?
        .pages()
        .len();

    if new_order.len() != usize::from(count) {
        return Err(PdfError::InvalidPageOrder(format!(
            "expected {count} page index(es), got {}",
            new_order.len()
        )));
    }

    let mut seen = vec![false; usize::from(count)];
    for &index in new_order {
        let Some(slot) = seen.get_mut(usize::from(index)) else {
            return Err(PdfError::InvalidPageOrder(format!(
                "page index {index} is out of range (document has {count} page(s))"
            )));
        };
        if *slot {
            return Err(PdfError::InvalidPageOrder(format!(
                "page index {index} appears more than once"
            )));
        }
        *slot = true;
    }

    extract_with(&pdfium, pdf_bytes, new_order)
}
