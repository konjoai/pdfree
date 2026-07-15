//! Rasterize PDF pages to images.
//!
//! Rendering goes through `PDFium` at an arbitrary DPI. PDF page geometry is
//! measured in points (1/72 inch), so a request for `dpi` scales the page by
//! `dpi / 72`.

use std::io::Cursor;

use image::ImageFormat;
use pdfium_render::prelude::*;

use crate::error::{PdfError, Result};

/// PDF user-space unit: 72 points per inch.
const POINTS_PER_INCH: f32 = 72.0;

/// Upper bound on a rendered edge, in pixels, to stop a pathological
/// DPI/page-size combination from allocating gigabytes.
const MAX_EDGE_PIXELS: f32 = 20_000.0;

/// How to rasterize a page.
#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    /// Target resolution in dots per inch. 150 is a good screen default; 300
    /// is print quality.
    pub dpi: f32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { dpi: 150.0 }
    }
}

impl RenderOptions {
    /// Convenience constructor.
    #[must_use]
    pub fn with_dpi(dpi: f32) -> Self {
        Self { dpi }
    }

    fn scale(self) -> f32 {
        self.dpi / POINTS_PER_INCH
    }
}

/// Render page `index` (0-based) of the given PDF bytes to PNG-encoded bytes.
///
/// This binds `PDFium`, loads the document from memory, rasterizes the one
/// page, and returns the PNG. Working from bytes (rather than a file path)
/// keeps the same code path usable on the web, where there is no filesystem.
///
/// # Errors
///
/// Returns [`PdfError::InvalidRenderTarget`] for a non-positive/non-finite
/// DPI or a render that would exceed the pixel-size guard,
/// [`PdfError::PageOutOfRange`] for a missing page index, and propagates
/// `PDFium` / image-encoding errors otherwise.
pub fn render_page_to_png(
    pdf_bytes: &[u8],
    index: u16,
    options: &RenderOptions,
) -> Result<Vec<u8>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    render_loaded_page_to_png(&document, index, options)
}

/// Same as [`render_page_to_png`], but works from an already-bound `PDFium`
/// document rather than binding and re-parsing `pdf_bytes` itself. Exists so
/// [`crate::pageview`] can gather a rendered page *and* its detected boxes
/// from a single bind + parse instead of two. See
/// [`crate::boxes::boxes_on_loaded_page`]'s doc comment for why this must
/// only ever be called with a freshly-bound `document`, never one reused
/// across a different prior document load.
pub(crate) fn render_loaded_page_to_png(
    document: &PdfDocument,
    index: u16,
    options: &RenderOptions,
) -> Result<Vec<u8>> {
    if !(options.dpi.is_finite() && options.dpi > 0.0) {
        return Err(PdfError::InvalidRenderTarget(format!(
            "dpi must be a positive, finite number (got {})",
            options.dpi
        )));
    }

    let pages = document.pages();

    let count = pages.len();
    if index >= count {
        return Err(PdfError::PageOutOfRange { index, count });
    }

    let page = pages.get(index)?;

    // Guard against absurd allocations before handing PDFium the config.
    let scale = options.scale();
    let width_px = page.width().value * scale;
    let height_px = page.height().value * scale;
    if width_px > MAX_EDGE_PIXELS || height_px > MAX_EDGE_PIXELS {
        return Err(PdfError::InvalidRenderTarget(format!(
            "rendered page would be {width_px:.0}x{height_px:.0}px at {} DPI, \
             which exceeds the {MAX_EDGE_PIXELS:.0}px limit",
            options.dpi
        )));
    }

    let config = PdfRenderConfig::new().scale_page_by_factor(scale);
    let image = page.render_with_config(&config)?.as_image();

    let mut png = Vec::new();
    image.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)?;
    Ok(png)
}

/// Page `index`'s size in PDF points (72/inch), without rendering it.
///
/// Cheap relative to [`render_page_to_png`] — it opens the document and reads
/// the page's declared media box, no rasterization involved. Callers that
/// need to fit a page to a viewport (see [`fit_to_page`]) should read the
/// size with this first, rather than rendering once to discover it.
///
/// # Errors
///
/// Returns [`PdfError::PageOutOfRange`] for a missing page index, and
/// propagates `PDFium` errors otherwise.
pub fn page_size_points(pdf_bytes: &[u8], index: u16) -> Result<(f32, f32)> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    let pages = document.pages();

    let count = pages.len();
    if index >= count {
        return Err(PdfError::PageOutOfRange { index, count });
    }

    let page = pages.get(index)?;
    Ok((page.width().value, page.height().value))
}

/// Pure math: given a page's size in PDF points and the available viewport
/// size in pixels, compute the [`RenderOptions`] (i.e. the DPI) that renders
/// the page as large as possible while still fitting entirely inside the
/// viewport on both axes.
///
/// Exists so every platform shell (macOS, web, Tauri, iOS) computes the
/// default "whole page visible" zoom identically, rather than each
/// back-computing its own fit math — see the Core UX Principles in
/// `CLAUDE.md` ("default view = whole page visible, always").
///
/// Degenerate inputs (a non-positive/non-finite page dimension or viewport
/// dimension) fall back to [`RenderOptions::default`] rather than producing
/// a non-finite or non-positive DPI.
#[must_use]
pub fn fit_to_page(
    page_width_pts: f32,
    page_height_pts: f32,
    viewport_width_px: f32,
    viewport_height_px: f32,
) -> RenderOptions {
    let valid = |v: f32| v.is_finite() && v > 0.0;
    if !(valid(page_width_pts)
        && valid(page_height_pts)
        && valid(viewport_width_px)
        && valid(viewport_height_px))
    {
        return RenderOptions::default();
    }

    let scale = (viewport_width_px / page_width_pts).min(viewport_height_px / page_height_pts);
    RenderOptions::with_dpi(scale * POINTS_PER_INCH)
}
