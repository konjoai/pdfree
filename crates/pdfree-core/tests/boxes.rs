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

/// A one-page PDF with a single bare horizontal line (a "fill on this line"
/// underline) and, optionally, an enclosing stroked rectangle around it.
fn pdf_with_underline(x0: f32, x1: f32, y: f32, enclosing: Option<PdfRect>) -> Vec<u8> {
    let pdfium = pdfree_core::pdfium::bind().expect("bind pdfium");
    let mut document = pdfium.create_new_pdf().expect("create pdf");
    let mut page = document
        .pages_mut()
        .create_page_at_start(PdfPagePaperSize::Custom(
            PdfPoints::new(612.0),
            PdfPoints::new(792.0),
        ))
        .expect("create page");
    if let Some(rect) = enclosing {
        page.objects_mut()
            .create_path_object_rect(rect, Some(PdfColor::BLACK), Some(PdfPoints::new(1.0)), None)
            .expect("enclosing rect");
    }
    page.objects_mut()
        .create_path_object_line(
            PdfPoints::new(x0),
            PdfPoints::new(y),
            PdfPoints::new(x1),
            PdfPoints::new(y),
            PdfColor::BLACK,
            PdfPoints::new(1.0),
        )
        .expect("underline");
    document.save_to_bytes().expect("save")
}

#[test]
fn detects_a_fill_in_underline_with_no_box_around_it() {
    skip_without_pdfium!();

    // A lone horizontal line at y=300 from x=100 to x=400 — the shape of
    // `Name: ______`, which the cell/rect tiers can't see at all.
    let bytes = pdf_with_underline(100.0, 400.0, 300.0, None);
    let found = boxes_on_page(&bytes, 0).expect("boxes_on_page");

    assert_eq!(
        found.len(),
        1,
        "expected exactly one underline field: {found:?}"
    );
    let field = found[0];
    assert!((field.x - 100.0).abs() < 2.0, "x = {}", field.x);
    assert!((field.width - 300.0).abs() < 2.0, "width = {}", field.width);
    // The affordance sits *on* the line (its bottom edge at the line's y) and
    // is about one text line tall.
    assert!((field.y - 300.0).abs() < 2.0, "y = {}", field.y);
    assert!(
        field.height > 6.0 && field.height <= 17.0,
        "height = {}",
        field.height
    );
}

#[test]
fn prefers_the_inner_field_over_an_enclosing_region() {
    skip_without_pdfium!();

    // A big bordered region (100..500 x 250..450) with a fill line inside it
    // — the shell should highlight the line, not the whole region box.
    let region = PdfRect::new(
        PdfPoints::new(250.0),
        PdfPoints::new(100.0),
        PdfPoints::new(450.0),
        PdfPoints::new(500.0),
    );
    let bytes = pdf_with_underline(140.0, 460.0, 320.0, Some(region));
    let found = boxes_on_page(&bytes, 0).expect("boxes_on_page");

    // Exactly the inner fill line survives; the 400x200 region is dropped.
    assert_eq!(found.len(), 1, "expected only the inner field: {found:?}");
    let field = found[0];
    assert!((field.x - 140.0).abs() < 2.0, "x = {}", field.x);
    assert!((field.width - 320.0).abs() < 2.0, "width = {}", field.width);
    assert!(
        field.height <= 17.0,
        "should be a slim fill field, got {}",
        field.height
    );
}

#[test]
fn irs_1040_still_scans_without_regression() {
    skip_without_pdfium!();

    // Smoke test against a real, multi-field government form: detection must
    // return a healthy set of on-page fields and never a box larger than the
    // page. (Not asserting an exact count — real forms are ragged.)
    let bytes = include_bytes!("fixtures/irs_f1040.pdf");
    let found = boxes_on_page(bytes, 0).expect("boxes_on_page");
    assert!(
        found.len() >= 5,
        "far too few fields detected: {}",
        found.len()
    );
    for b in &found {
        assert!(b.width > 0.0 && b.height > 0.0, "degenerate box: {b:?}");
        assert!(
            b.width < 620.0 && b.height < 800.0,
            "box larger than page: {b:?}"
        );
    }
}

/// A one-page PDF with a stroked rectangle framing a small solid-fill image
/// well inside its bounds — standing in for a logo/seal graphic bordered by
/// a decorative rectangle. Built in its own function (matching
/// `pdf_with_rect`/`pdf_with_ruled_grid`/`pdf_with_underline` above) so its
/// own `pdfium::bind()` fully drops before the test calls `boxes_on_page`,
/// which binds again — two live bindings in one process is a confirmed hang
/// (see `pages.rs`'s "never call `bind()` twice within one call chain" note).
fn pdf_with_rect_framing_an_image(rect: PdfRect) -> Vec<u8> {
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
    let image = image::RgbImage::from_pixel(40, 40, image::Rgb([10, 10, 10])).into();
    page.objects_mut()
        .create_image_object(
            PdfPoints::new(140.0),
            PdfPoints::new(260.0),
            &image,
            Some(PdfPoints::new(80.0)),
            Some(PdfPoints::new(80.0)),
        )
        .expect("create image");
    document.save_to_bytes().expect("save")
}

#[test]
fn rejects_a_lone_rectangle_that_already_contains_an_image() {
    skip_without_pdfium!();

    // A logo/seal is commonly framed by a stroked rectangle border — that
    // border shouldn't be offered as a fillable field just because it's a
    // lone rectangle in the right size range (Tier 3).
    let rect = PdfRect::new(
        PdfPoints::new(200.0),
        PdfPoints::new(100.0),
        PdfPoints::new(400.0),
        PdfPoints::new(260.0),
    );
    let bytes = pdf_with_rect_framing_an_image(rect);

    let found = boxes_on_page(&bytes, 0).expect("boxes_on_page");
    assert!(
        found.is_empty(),
        "a rectangle framing an image shouldn't be treated as fillable: {found:?}"
    );
}

/// A one-page PDF with a bare underline and a name already printed in the
/// gap above it — see `pdf_with_rect_framing_an_image` above for why this is
/// its own function rather than inlined in the test.
fn pdf_with_underline_and_printed_value() -> Vec<u8> {
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
        .create_path_object_line(
            PdfPoints::new(100.0),
            PdfPoints::new(300.0),
            PdfPoints::new(400.0),
            PdfPoints::new(300.0),
            PdfColor::BLACK,
            PdfPoints::new(1.0),
        )
        .expect("underline");
    let font = document.fonts_mut().helvetica();
    page.objects_mut()
        .create_text_object(
            PdfPoints::new(110.0),
            PdfPoints::new(304.0),
            "Jane Doe",
            font,
            PdfPoints::new(12.0),
        )
        .expect("create text");
    document.save_to_bytes().expect("save")
}

#[test]
fn rejects_an_underline_with_a_value_already_printed_above_it() {
    skip_without_pdfium!();

    // Same shape as `detects_a_fill_in_underline_with_no_box_around_it`, but
    // with a name already printed in the gap above the line — this blank has
    // already been filled in, so it shouldn't be re-offered as fillable.
    let bytes = pdf_with_underline_and_printed_value();

    let found = boxes_on_page(&bytes, 0).expect("boxes_on_page");
    assert!(
        found.is_empty(),
        "an underline with a value already written above it shouldn't be treated as fillable: {found:?}"
    );
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
