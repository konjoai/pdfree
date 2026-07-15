//! Acceptance tests for [`pdfree_core::pageview::page_view`] — the combined
//! render + box-detection call that replaced two separate bind-and-reparse
//! round trips (see the module's own doc comment for why that mattered).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::error::PdfError;
use pdfree_core::pageview::page_view;

const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");
const IRS_1040: &[u8] = include_bytes!("fixtures/irs_f1040.pdf");

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
fn renders_the_same_png_as_render_page_to_png() {
    skip_without_pdfium!();

    let view = page_view(SAMPLE, 0, 150.0).expect("page_view");
    assert_eq!(&view.png[..8], b"\x89PNG\r\n\x1a\n", "output is a PNG");

    let expected = pdfree_core::renderer::render_page_to_png(
        SAMPLE,
        0,
        &pdfree_core::renderer::RenderOptions::with_dpi(150.0),
    )
    .expect("render_page_to_png");
    assert_eq!(
        view.png, expected,
        "same bytes as the standalone render call"
    );
}

#[test]
fn detects_the_same_boxes_as_boxes_on_page() {
    skip_without_pdfium!();

    let view = page_view(IRS_1040, 0, 150.0).expect("page_view");
    let expected = pdfree_core::boxes::boxes_on_page(IRS_1040, 0).expect("boxes_on_page");

    assert_eq!(
        view.boxes.len(),
        expected.len(),
        "same box count as the standalone scan"
    );
    for (a, b) in view.boxes.iter().zip(expected.iter()) {
        assert_eq!(a, b, "same boxes as the standalone scan");
    }
}

#[test]
fn rejects_an_out_of_range_page() {
    skip_without_pdfium!();

    let err = page_view(SAMPLE, 9, 150.0).expect_err("page 9 does not exist");
    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn rejects_an_invalid_dpi() {
    skip_without_pdfium!();

    let err = page_view(SAMPLE, 0, 0.0).expect_err("zero dpi is invalid");
    assert!(
        matches!(err, PdfError::InvalidRenderTarget(_)),
        "got {err:?}"
    );
}
