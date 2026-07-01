//! Phase 3 acceptance tests: text extraction and image-to-PDF conversion.
//!
//! Like `tests/render.rs`, these skip with a notice (rather than fail) when
//! `PDFium` isn't bundled, so a bare checkout still builds green. Run
//! `scripts/fetch-pdfium.sh` first to make them exercise `PDFium` for real.
//!
//! Test code may `unwrap`/`expect` freely (see `.github/copilot-instructions.md`)
//! — the production-code ban only applies to `pdfree-core`'s library surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::convert;
use pdfree_core::error::PdfError;
use pdfree_core::Document;

const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");
const SIGNATURE_PNG: &[u8] = include_bytes!("fixtures/signature.png");

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
fn extracts_text_from_every_page() {
    skip_without_pdfium!();

    let text = convert::to_text(SAMPLE).expect("to_text");
    assert!(text.contains("PDFree - page one"));
    assert!(text.contains("PDFree - page two"));
}

#[test]
fn builds_a_single_page_pdf_from_an_image_at_exact_size() {
    skip_without_pdfium!();

    let dpi = 96.0;
    let pdf = convert::from_image(SIGNATURE_PNG, dpi).expect("from_image");

    let doc = Document::from_bytes(pdf, None).unwrap();
    assert_eq!(doc.page_count(), 1);

    // The source PNG is 200x80px; at 96 DPI that's exactly 200x80px again
    // when rendered back out at the same DPI (72pt/96dpi round-trips to
    // whole pixels), proving the page was sized to the image, not to a
    // fixed paper size.
    let png = doc
        .render_page(0, &pdfree_core::RenderOptions::with_dpi(dpi))
        .unwrap();
    let rendered = image::load_from_memory(&png).unwrap();
    let original = image::load_from_memory(SIGNATURE_PNG).unwrap();
    assert_eq!(rendered.width(), original.width());
    assert_eq!(rendered.height(), original.height());
}

#[test]
fn from_image_rejects_a_non_positive_dpi() {
    skip_without_pdfium!();

    let err = convert::from_image(SIGNATURE_PNG, 0.0).expect_err("zero dpi is invalid");
    assert!(
        matches!(err, PdfError::InvalidRenderTarget(_)),
        "got {err:?}"
    );
}

#[test]
fn from_image_rejects_invalid_image_bytes() {
    skip_without_pdfium!();

    let err = convert::from_image(b"not an image", 96.0)
        .expect_err("garbage bytes must not silently no-op");
    assert!(matches!(err, PdfError::Image(_)), "got {err:?}");
}

#[test]
fn docx_conversion_is_honestly_not_implemented() {
    let to_err = convert::to_docx(SAMPLE).expect_err("not implemented");
    assert!(
        matches!(to_err, PdfError::NotImplemented(_)),
        "got {to_err:?}"
    );

    let from_err = convert::from_docx(b"").expect_err("not implemented");
    assert!(
        matches!(from_err, PdfError::NotImplemented(_)),
        "got {from_err:?}"
    );
}
