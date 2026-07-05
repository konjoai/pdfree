//! Phase 3 acceptance tests: merge, split, rotate, extract, reorder.
//!
//! Like `tests/render.rs`, these skip with a notice (rather than fail) when
//! `PDFium` isn't bundled, so a bare checkout still builds green. Run
//! `scripts/fetch-pdfium.sh` first to make them exercise `PDFium` for real.
//!
//! Test code may `unwrap`/`expect` freely (see `.github/copilot-instructions.md`)
//! — the production-code ban only applies to `pdfree-core`'s library surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::error::PdfError;
use pdfree_core::pages::{self, BatesOptions, Rotation, StampCorner};
use pdfree_core::{editor, renderer, Document, RenderOptions};

/// 2-page fixture ("`PDFree` - page one" / "page two").
const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");
/// 1-page real-world fixture (used only for its page count in merge tests).
const IRS_F1040: &[u8] = include_bytes!("fixtures/irs_f1040.pdf");

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
fn merges_documents_preserving_page_order_and_count() {
    skip_without_pdfium!();

    let f1040_pages = Document::from_bytes(IRS_F1040.to_vec(), None)
        .unwrap()
        .page_count();

    let merged = pages::merge(&[SAMPLE.to_vec(), IRS_F1040.to_vec()]).expect("merge");
    let doc = Document::from_bytes(merged, None).unwrap();
    assert_eq!(doc.page_count(), 2 + f1040_pages);
}

#[test]
fn merge_rejects_an_empty_document_list() {
    skip_without_pdfium!();

    let err = pages::merge(&[]).expect_err("empty document list is invalid");
    assert!(matches!(err, PdfError::InvalidPageRange(_)), "got {err:?}");
}

#[test]
fn splits_into_one_document_per_range() {
    skip_without_pdfium!();

    let pieces = pages::split(SAMPLE, &[(0, 0), (1, 1)]).expect("split");
    assert_eq!(pieces.len(), 2);
    for piece in &pieces {
        let doc = Document::from_bytes(piece.clone(), None).unwrap();
        assert_eq!(doc.page_count(), 1);
    }
}

#[test]
fn split_rejects_an_empty_range_list() {
    skip_without_pdfium!();

    let err = pages::split(SAMPLE, &[]).expect_err("empty range list is invalid");
    assert!(matches!(err, PdfError::InvalidPageRange(_)), "got {err:?}");
}

#[test]
fn split_rejects_an_inverted_range() {
    skip_without_pdfium!();

    let err = pages::split(SAMPLE, &[(1, 0)]).expect_err("start after end is invalid");
    assert!(matches!(err, PdfError::InvalidPageRange(_)), "got {err:?}");
}

#[test]
fn split_rejects_an_out_of_range_end() {
    skip_without_pdfium!();

    let err = pages::split(SAMPLE, &[(0, 9)]).expect_err("page 9 does not exist");
    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn rotates_a_page_and_it_renders_differently() {
    skip_without_pdfium!();

    let rotated = pages::rotate(SAMPLE, 0, Rotation::Clockwise90).expect("rotate");

    let before = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let after = Document::from_bytes(rotated, None).unwrap();
    let png_before = before
        .render_page(0, &RenderOptions::with_dpi(72.0))
        .unwrap();
    let png_after = after
        .render_page(0, &RenderOptions::with_dpi(72.0))
        .unwrap();
    assert_ne!(
        png_before, png_after,
        "a 90-degree rotation must change the rendered page"
    );
}

#[test]
fn rotate_rejects_an_out_of_range_page() {
    skip_without_pdfium!();

    let err = pages::rotate(SAMPLE, 9, Rotation::None).expect_err("page 9 does not exist");
    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn extract_pulls_pages_in_the_exact_order_given() {
    skip_without_pdfium!();

    // sample.pdf's two pages render differently ("page one" vs "page two"),
    // so extracting [1, 0] and checking pixel content directly proves order
    // is preserved, not just page count.
    let reversed = pages::extract(SAMPLE, &[1, 0]).expect("extract");
    let reversed_doc = Document::from_bytes(reversed, None).unwrap();
    assert_eq!(reversed_doc.page_count(), 2);

    let original = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let orig_page0 = original
        .render_page(0, &RenderOptions::with_dpi(72.0))
        .unwrap();
    let orig_page1 = original
        .render_page(1, &RenderOptions::with_dpi(72.0))
        .unwrap();
    let new_page0 = reversed_doc
        .render_page(0, &RenderOptions::with_dpi(72.0))
        .unwrap();

    assert_eq!(
        new_page0, orig_page1,
        "extract([1,0])'s first page must be the original's second"
    );
    assert_ne!(
        new_page0, orig_page0,
        "and must not be the original's first"
    );
}

#[test]
fn extract_rejects_an_empty_page_list() {
    skip_without_pdfium!();

    let err = pages::extract(SAMPLE, &[]).expect_err("empty page list is invalid");
    assert!(matches!(err, PdfError::InvalidPageRange(_)), "got {err:?}");
}

#[test]
fn extract_rejects_an_out_of_range_index() {
    skip_without_pdfium!();

    let err = pages::extract(SAMPLE, &[9]).expect_err("page 9 does not exist");
    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn reorders_all_pages() {
    skip_without_pdfium!();

    let reordered = pages::reorder(SAMPLE, &[1, 0]).expect("reorder");
    let doc = Document::from_bytes(reordered, None).unwrap();
    assert_eq!(doc.page_count(), 2);
}

#[test]
fn reorder_rejects_the_wrong_number_of_pages() {
    skip_without_pdfium!();

    let err = pages::reorder(SAMPLE, &[0]).expect_err("only one index for a 2-page document");
    assert!(matches!(err, PdfError::InvalidPageOrder(_)), "got {err:?}");
}

#[test]
fn reorder_rejects_a_duplicate_index() {
    skip_without_pdfium!();

    let err = pages::reorder(SAMPLE, &[0, 0]).expect_err("0 repeated is not a permutation");
    assert!(matches!(err, PdfError::InvalidPageOrder(_)), "got {err:?}");
}

#[test]
fn reorder_rejects_an_out_of_range_index() {
    skip_without_pdfium!();

    let err = pages::reorder(SAMPLE, &[0, 9]).expect_err("page 9 does not exist");
    assert!(matches!(err, PdfError::InvalidPageOrder(_)), "got {err:?}");
}

#[test]
fn bates_numbers_every_page_sequentially_with_prefix_and_padding() {
    skip_without_pdfium!();

    let options = BatesOptions {
        prefix: "ACME-".to_string(),
        digits: 4,
        ..BatesOptions::default()
    };
    let stamped = pages::bates_number(SAMPLE, &options).expect("bates_number");

    let runs = editor::text_runs(&stamped).expect("text_runs");
    assert!(
        runs.iter().any(|r| r.page == 0 && r.text == "ACME-0001"),
        "page 0 stamp missing; runs: {runs:?}"
    );
    assert!(
        runs.iter().any(|r| r.page == 1 && r.text == "ACME-0002"),
        "page 1 stamp missing; runs: {runs:?}"
    );
}

#[test]
fn bates_number_defaults_have_no_prefix_or_suffix_and_six_digit_padding() {
    skip_without_pdfium!();

    let stamped = pages::bates_number(SAMPLE, &BatesOptions::default()).expect("bates_number");

    let runs = editor::text_runs(&stamped).expect("text_runs");
    assert!(runs.iter().any(|r| r.page == 0 && r.text == "000001"));
    assert!(runs.iter().any(|r| r.page == 1 && r.text == "000002"));
}

#[test]
fn bates_number_rejects_a_non_positive_font_size() {
    skip_without_pdfium!();

    let options = BatesOptions {
        font_size: 0.0,
        ..BatesOptions::default()
    };
    let err = pages::bates_number(SAMPLE, &options).expect_err("font_size 0 is invalid");
    assert!(matches!(err, PdfError::InvalidOverlay(_)), "got {err:?}");
}

#[test]
fn bates_number_right_aligned_corner_lands_its_right_edge_at_the_margin() {
    skip_without_pdfium!();

    let options = BatesOptions {
        corner: StampCorner::BottomRight,
        margin: 20.0,
        font_size: 12.0,
        ..BatesOptions::default()
    };
    let stamped = pages::bates_number(SAMPLE, &options).expect("bates_number");

    let runs = editor::text_runs(&stamped).expect("text_runs");
    let stamp = runs
        .iter()
        .find(|r| r.page == 0 && r.text == "000001")
        .expect("stamp on page 0");

    let (page_width, _) = renderer::page_size_points(SAMPLE, 0).expect("page_size_points");
    let expected_right_edge = page_width - options.margin;
    assert!(
        (stamp.x + stamp.width - expected_right_edge).abs() < 2.0,
        "right edge should sit at the margin: x={} width={} page_width={}",
        stamp.x,
        stamp.width,
        page_width
    );
}
