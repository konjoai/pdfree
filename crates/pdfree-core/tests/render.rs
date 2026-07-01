//! Phase 0 acceptance tests: open a real PDF and render a page to PNG.
//!
//! These exercise the full `PDFium` path. When the `PDFium` shared library is not
//! bundled (see `docs/pdfium-bundling.md` / `scripts/fetch-pdfium.sh`), the
//! rendering tests print a skip notice and pass, so a checkout without the
//! binary still builds green. Run `scripts/fetch-pdfium.sh` first to make them
//! render for real.
//!
//! Test code may `unwrap`/`expect` freely (see `.github/copilot-instructions.md`)
//! — the production-code ban only applies to `pdfree-core`'s library surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::error::PdfError;
use pdfree_core::{Document, RenderOptions};

const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");

/// True when `PDFium` can be loaded, so render tests should actually run.
fn pdfium_available() -> bool {
    pdfree_core::pdfium::bind().is_ok()
}

macro_rules! skip_without_pdfium {
    () => {
        if !pdfium_available() {
            eprintln!(
                "skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable"
            );
            return;
        }
    };
}

#[test]
fn opens_document_and_reads_page_count() {
    skip_without_pdfium!();

    let doc = Document::from_bytes(SAMPLE.to_vec(), None).expect("open sample");
    assert_eq!(doc.page_count(), 2, "sample fixture has two pages");
}

#[test]
fn renders_first_page_to_png() {
    skip_without_pdfium!();

    let doc = Document::from_bytes(SAMPLE.to_vec(), None).expect("open sample");
    let png = doc
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .expect("render page 0");

    // Valid PNG signature.
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "output is a PNG");

    // A US Letter page (612x792 pt) at 150 DPI is ~1275x1650 px.
    let image = image::load_from_memory(&png).expect("decode png");
    assert_eq!(image.width(), 1275, "612pt @ 150dpi");
    assert_eq!(image.height(), 1650, "792pt @ 150dpi");
}

#[test]
fn dpi_controls_output_resolution() {
    skip_without_pdfium!();

    let doc = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let low = doc.render_page(0, &RenderOptions::with_dpi(72.0)).unwrap();
    let high = doc.render_page(0, &RenderOptions::with_dpi(300.0)).unwrap();

    let low = image::load_from_memory(&low).unwrap();
    let high = image::load_from_memory(&high).unwrap();
    assert_eq!(low.width(), 612, "72dpi == 1x point size");
    assert_eq!(high.width(), 2550, "300dpi == 4.16x");
}

#[test]
fn render_rejects_out_of_range_page() {
    skip_without_pdfium!();

    let doc = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let err = doc
        .render_page(9, &RenderOptions::default())
        .expect_err("page 9 does not exist");
    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn render_rejects_invalid_dpi() {
    // Validated before PDFium is ever touched, so this runs without the library.
    let err = pdfree_core::renderer::render_page_to_png(SAMPLE, 0, &RenderOptions::with_dpi(0.0))
        .expect_err("zero dpi is invalid");
    assert!(
        matches!(err, PdfError::InvalidRenderTarget(_)),
        "got {err:?}"
    );
}
