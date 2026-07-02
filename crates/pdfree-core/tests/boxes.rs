//! Phase 4 acceptance tests: box/cell boundary detection for click-to-fill.
//!
//! Like the other Phase 2/3 test files, these skip with a notice (rather
//! than fail) when `PDFium` isn't bundled, so a bare checkout still builds
//! green. Run `scripts/fetch-pdfium.sh` first to make them exercise `PDFium`
//! for real.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfium_render::prelude::*;

use pdfree_core::boxes::{box_at_point, boxes_on_page};
use pdfree_core::error::PdfError;

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

/// Build a one-page, blank-letter-size PDF with a single stroked rectangle
/// drawn at the given PDF-point coordinates.
fn pdf_with_rect(rect: PdfRect) -> Vec<u8> {
    let pdfium = pdfree_core::pdfium::bind().expect("bind pdfium");
    let mut document = pdfium.create_new_pdf().expect("create pdf");
    let mut page = document
        .pages_mut()
        .create_page_at_start(PdfPagePaperSize::Custom(
            PdfPoints::new(612.0),
            PdfPoints::new(792.0),
        ))
        .expect("create page");
    page.objects_mut()
        .create_path_object_rect(rect, Some(PdfColor::BLACK), Some(PdfPoints::new(1.0)), None)
        .expect("create rect");
    document.save_to_bytes().expect("save")
}

#[test]
fn finds_a_single_drawn_rectangle_containing_the_point() {
    skip_without_pdfium!();

    // PdfRect::new(bottom, left, top, right).
    let rect = PdfRect::new(
        PdfPoints::new(200.0),
        PdfPoints::new(100.0),
        PdfPoints::new(400.0),
        PdfPoints::new(260.0),
    );
    let bytes = pdf_with_rect(rect);

    let found = box_at_point(&bytes, 0, 150.0, 230.0)
        .expect("box_at_point")
        .expect("a box should be found");

    // The rect is 160x200 (100..260, 200..400); a stroked path's bounds()
    // pad out by roughly half the stroke width on each side, so allow a few
    // points of slack rather than asserting an exact match.
    assert_eq!(found.page, 0);
    assert!((found.x - 100.0).abs() < 3.0, "x = {}", found.x);
    assert!((found.y - 200.0).abs() < 3.0, "y = {}", found.y);
    assert!((found.width - 160.0).abs() < 3.0, "width = {}", found.width);
    assert!(
        (found.height - 200.0).abs() < 3.0,
        "height = {}",
        found.height
    );
}

#[test]
fn returns_none_when_no_box_encloses_the_point() {
    skip_without_pdfium!();

    // PdfRect::new(bottom, left, top, right).
    let rect = PdfRect::new(
        PdfPoints::new(200.0),
        PdfPoints::new(100.0),
        PdfPoints::new(400.0),
        PdfPoints::new(260.0),
    );
    let bytes = pdf_with_rect(rect);

    // Nowhere near the drawn rectangle.
    let found = box_at_point(&bytes, 0, 10.0, 10.0).expect("box_at_point");
    assert!(found.is_none());
}

/// Build a one-page PDF with a 3-column, 2-row ruled table: two horizontal
/// lines each spanning the full table width, and three vertical lines each
/// spanning the full table height — the "ruled grid" shape real forms draw
/// (e.g. a name/middle-initial/last-name row), as opposed to one rect per
/// cell.
fn pdf_with_ruled_grid() -> Vec<u8> {
    let pdfium = pdfree_core::pdfium::bind().expect("bind pdfium");
    let mut document = pdfium.create_new_pdf().expect("create pdf");
    let mut page = document
        .pages_mut()
        .create_page_at_start(PdfPagePaperSize::Custom(
            PdfPoints::new(612.0),
            PdfPoints::new(792.0),
        ))
        .expect("create page");

    let color = PdfColor::BLACK;
    let width = PdfPoints::new(1.0);
    // Columns at x = 100, 200, 300, 400; rows at y = 500, 550, 600.
    for &y in &[500.0, 550.0, 600.0] {
        page.objects_mut()
            .create_path_object_line(
                PdfPoints::new(100.0),
                PdfPoints::new(y),
                PdfPoints::new(400.0),
                PdfPoints::new(y),
                color,
                width,
            )
            .expect("horizontal ruling");
    }
    for &x in &[100.0, 200.0, 300.0, 400.0] {
        page.objects_mut()
            .create_path_object_line(
                PdfPoints::new(x),
                PdfPoints::new(500.0),
                PdfPoints::new(x),
                PdfPoints::new(600.0),
                color,
                width,
            )
            .expect("vertical ruling");
    }

    document.save_to_bytes().expect("save")
}

#[test]
fn snaps_to_exactly_one_cell_of_a_ruled_grid_without_overshooting() {
    skip_without_pdfium!();

    let bytes = pdf_with_ruled_grid();

    // Click inside the middle cell of the bottom row: x in (200, 300), y in (500, 550).
    let found = box_at_point(&bytes, 0, 250.0, 525.0)
        .expect("box_at_point")
        .expect("a cell should be found");

    assert!((found.x - 200.0).abs() < 2.0, "x = {}", found.x);
    assert!((found.y - 500.0).abs() < 2.0, "y = {}", found.y);
    assert!(
        (found.width - 100.0).abs() < 2.0,
        "width = {} (overshot into a neighboring cell?)",
        found.width
    );
    assert!(
        (found.height - 50.0).abs() < 2.0,
        "height = {} (overshot into a neighboring row?)",
        found.height
    );
}

#[test]
fn boxes_on_page_finds_every_cell_of_a_ruled_grid() {
    skip_without_pdfium!();

    let bytes = pdf_with_ruled_grid();
    let found = boxes_on_page(&bytes, 0).expect("boxes_on_page");

    // A 3-column x 2-row grid has 6 cells.
    assert_eq!(found.len(), 6, "found: {:?}", found);
    for cell in &found {
        assert!(
            (cell.width - 100.0).abs() < 2.0,
            "cell width = {}",
            cell.width
        );
        assert!(
            (cell.height - 50.0).abs() < 2.0,
            "cell height = {}",
            cell.height
        );
    }
}

#[test]
fn rejects_an_out_of_range_page() {
    skip_without_pdfium!();

    let rect = PdfRect::new(
        PdfPoints::new(0.0),
        PdfPoints::new(0.0),
        PdfPoints::new(10.0),
        PdfPoints::new(10.0),
    );
    let bytes = pdf_with_rect(rect);

    let err = box_at_point(&bytes, 5, 0.0, 0.0).unwrap_err();
    assert!(matches!(
        err,
        PdfError::PageOutOfRange { index: 5, count: 1 }
    ));
}
