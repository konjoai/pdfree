//! Page operations: merge, split, rotate, extract, reorder (Phase 3), plus
//! Bates-style sequential stamping (Phase 4 quick win).
//!
//! Every merge/split/extract operation here builds a fresh in-memory
//! `PDFium` document and copies pages into it via `PDFium`'s own page-import
//! machinery (`FPDF_ImportPages`), rather than manipulating page trees
//! directly — the same mechanism a tool like `pdftk` relies on, so page
//! resources (fonts, images) come along correctly instead of dangling.

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

/// Rotate one page *relative to its current orientation* (so repeated
/// clockwise-90 calls cycle a page through all four orientations), returning
/// the updated PDF as new bytes. `Rotation::None` is a no-op that leaves the
/// page's existing rotation untouched.
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
    // Add the requested turn to whatever rotation the page already carries,
    // modulo a full turn — `set_rotation` alone sets an *absolute* value, so
    // pressing "rotate right" repeatedly would keep re-setting the same 90°
    // instead of advancing 90° → 180° → 270° → 0°.
    let combined = quarter_turns(rotation_of(&target)) + quarter_turns(rotation.to_pdfium());
    target.set_rotation(rotation_from_quarter_turns(combined));

    Ok(document.save_to_bytes()?)
}

/// The page's current rotation, defaulting to upright if it can't be read.
fn rotation_of(page: &PdfPage) -> PdfPageRenderRotation {
    page.rotation().unwrap_or(PdfPageRenderRotation::None)
}

fn quarter_turns(r: PdfPageRenderRotation) -> u8 {
    match r {
        PdfPageRenderRotation::None => 0,
        PdfPageRenderRotation::Degrees90 => 1,
        PdfPageRenderRotation::Degrees180 => 2,
        PdfPageRenderRotation::Degrees270 => 3,
    }
}

fn rotation_from_quarter_turns(turns: u8) -> PdfPageRenderRotation {
    match turns % 4 {
        1 => PdfPageRenderRotation::Degrees90,
        2 => PdfPageRenderRotation::Degrees180,
        3 => PdfPageRenderRotation::Degrees270,
        _ => PdfPageRenderRotation::None,
    }
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

/// Which corner of the page a [`bates_number`] stamp is anchored to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StampCorner {
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
}

/// Options controlling [`bates_number`].
#[derive(Debug, Clone)]
pub struct BatesOptions {
    /// Text stamped before the sequence number, e.g. `"ACME-"`.
    pub prefix: String,
    /// Text stamped after the sequence number, e.g. `"-CONFIDENTIAL"`.
    pub suffix: String,
    /// The number stamped on page 0; each following page increments by one.
    pub start: u32,
    /// Zero-pad the number to at least this many digits (e.g. `6` ->
    /// `"000001"`). `0` means no padding.
    pub digits: u8,
    /// Which corner of the page to stamp.
    pub corner: StampCorner,
    /// Margin from the page edge, in PDF points.
    pub margin: f32,
    /// Font size, in PDF points. Must be a positive, finite number.
    pub font_size: f32,
}

impl Default for BatesOptions {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            suffix: String::new(),
            start: 1,
            digits: 6,
            corner: StampCorner::BottomRight,
            margin: 24.0,
            font_size: 9.0,
        }
    }
}

/// Stamp a sequential Bates-style number onto every page of a document —
/// `<prefix><zero-padded number><suffix>`, starting at `options.start` on
/// page 0 and incrementing by one per page — returning the updated PDF as
/// new bytes.
///
/// This is the legal/discovery "Bates numbering" convention, implemented as
/// a loop of stamped text objects (the same primitive
/// [`crate::forms::overlay_text`] uses) rather than a new module: each
/// page's label is measured after being placed at the left margin, so a
/// right-aligned corner shifts it left by its actual rendered width instead
/// of guessing at one.
///
/// # Errors
///
/// Returns [`PdfError::InvalidOverlay`] if `font_size` is not a positive,
/// finite number, and propagates `PDFium` / load errors otherwise.
pub fn bates_number(pdf_bytes: &[u8], options: &BatesOptions) -> Result<Vec<u8>> {
    if !(options.font_size.is_finite() && options.font_size > 0.0) {
        return Err(PdfError::InvalidOverlay(format!(
            "font_size must be a positive, finite number (got {})",
            options.font_size
        )));
    }

    let pdfium = crate::pdfium::bind()?;
    let mut document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    let font = document.fonts_mut().helvetica();
    let count = document.pages().len();
    let digits_width = usize::from(options.digits);

    for page_index in 0..count {
        let number = options.start.saturating_add(u32::from(page_index));
        let label = format!(
            "{}{number:0digits_width$}{}",
            options.prefix, options.suffix
        );

        let mut page = document.pages().get(page_index)?;
        let page_width = page.width().value;
        let page_height = page.height().value;

        let y = match options.corner {
            StampCorner::TopLeft | StampCorner::TopRight => {
                page_height - options.margin - options.font_size
            }
            StampCorner::BottomLeft | StampCorner::BottomRight => options.margin,
        };

        let mut stamp = page.objects_mut().create_text_object(
            PdfPoints::new(options.margin),
            PdfPoints::new(y),
            label.as_str(),
            font,
            PdfPoints::new(options.font_size),
        )?;

        if matches!(
            options.corner,
            StampCorner::TopRight | StampCorner::BottomRight
        ) {
            let label_width = stamp
                .as_text_object()
                .and_then(|text| text.bounds().ok())
                .map_or(0.0, |bounds| bounds.to_rect().width().value);
            let dx = (page_width - options.margin) - (options.margin + label_width);
            stamp.translate(PdfPoints::new(dx), PdfPoints::new(0.0))?;
        }
    }

    Ok(document.save_to_bytes()?)
}
