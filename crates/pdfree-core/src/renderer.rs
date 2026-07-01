//! Rasterize PDF pages to images.
//!
//! Rendering goes through PDFium at an arbitrary DPI. PDF page geometry is
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
    pub fn with_dpi(dpi: f32) -> Self {
        Self { dpi }
    }

    fn scale(&self) -> f32 {
        self.dpi / POINTS_PER_INCH
    }
}

/// Render page `index` (0-based) of the given PDF bytes to PNG-encoded bytes.
///
/// This binds PDFium, loads the document from memory, rasterizes the one page,
/// and returns the PNG. Working from bytes (rather than a file path) keeps the
/// same code path usable on the web, where there is no filesystem.
pub fn render_page_to_png(
    pdf_bytes: &[u8],
    index: u16,
    options: &RenderOptions,
) -> Result<Vec<u8>> {
    if !(options.dpi.is_finite() && options.dpi > 0.0) {
        return Err(PdfError::InvalidRenderTarget(format!(
            "dpi must be a positive, finite number (got {})",
            options.dpi
        )));
    }

    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
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
