//! Combined page load: render + box detection from a single bind + parse.
//!
//! Loading or navigating to a page used to mean two entirely separate
//! top-level calls — [`crate::renderer::render_page_to_png`] and
//! [`crate::boxes::boxes_on_page`] — each independently binding `PDFium` and
//! re-parsing the *entire* document from `pdf_bytes` from scratch.
//! `boxes_on_page`'s vector-graphics scan is documented as by far the
//! heaviest per-page `pdfree-core` call, so paying its full bind + parse cost
//! twice (once for itself, once again for the unrelated render call) on every
//! single page view was the actual root cause of "the app is slow to open
//! even a 1-page PDF" and "page navigation is slow" — not the page's own
//! genuine size or complexity. [`page_view`] does one bind, one parse, then
//! both operations against that one loaded document.
//!
//! This does *not* cache or reuse a `PDFium` binding across separate calls —
//! see [`crate::pdfium::bind`]'s docs for why that's unsafe with the current
//! `pdfium-render` version. Each call to [`page_view`] is its own complete,
//! self-contained bind-parse-use-drop cycle, exactly like every other
//! `pdfree-core` function; it just does two things instead of one within
//! that one cycle.

use crate::boxes::{boxes_on_loaded_page, DetectedBox};
use crate::error::Result;
use crate::renderer::{render_loaded_page_to_png, RenderOptions};

/// Everything a shell needs to display one page: its rendered image and
/// every fillable box detected on it.
#[derive(Debug, Clone)]
pub struct PageView {
    /// PNG-encoded page render at the requested DPI.
    pub png: Vec<u8>,
    /// Every fillable box (drawn rectangle or ruled-line cell) on this page
    /// — same result [`crate::boxes::boxes_on_page`] would give.
    pub boxes: Vec<DetectedBox>,
}

/// Render page `index` and detect its fillable boxes in one bind + parse.
///
/// # Errors
///
/// Returns [`crate::error::PdfError::PageOutOfRange`] if `index` doesn't
/// exist, [`crate::error::PdfError::InvalidRenderTarget`] for a bad DPI or a
/// render that would exceed the pixel-size guard, and propagates `PDFium` /
/// image-encoding errors otherwise.
pub fn page_view(pdf_bytes: &[u8], index: u16, dpi: f32) -> Result<PageView> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;

    let png = render_loaded_page_to_png(&document, index, &RenderOptions::with_dpi(dpi))?;
    let boxes = boxes_on_loaded_page(&document, index)?;

    Ok(PageView { png, boxes })
}
