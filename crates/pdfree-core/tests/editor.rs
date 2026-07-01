//! Phase 3 acceptance tests: font detection and in-place text replacement.
//!
//! Like `tests/render.rs`, these skip with a notice (rather than fail) when
//! `PDFium` isn't bundled, so a bare checkout still builds green. Run
//! `scripts/fetch-pdfium.sh` first to make them exercise `PDFium` for real.
//!
//! Test code may `unwrap`/`expect` freely (see `.github/copilot-instructions.md`)
//! — the production-code ban only applies to `pdfree-core`'s library surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::editor;
use pdfree_core::error::PdfError;
use pdfree_core::{Document, RenderOptions};

/// 2-page fixture; page 0 renders "`PDFree` - page one" in 24pt Helvetica
/// starting at roughly (72, 700) in PDF points.
const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");

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
fn enumerates_text_runs_with_font_and_bounds() {
    skip_without_pdfium!();

    let runs = editor::text_runs(SAMPLE).expect("text_runs");
    assert_eq!(runs.len(), 2, "one heading per page");

    let page0 = runs.iter().find(|r| r.page == 0).expect("page 0 run");
    assert_eq!(page0.text, "PDFree - page one");
    assert_eq!(page0.font_name, "Helvetica");
    assert!((page0.font_size - 24.0).abs() < 0.5);
    assert!(page0.width > 0.0 && page0.height > 0.0);
}

#[test]
fn finds_the_text_run_under_a_point() {
    skip_without_pdfium!();

    // The heading sits at roughly (72, 700)-(280, 722) in PDF points.
    let hit = editor::text_run_at_point(SAMPLE, 0, 80.0, 705.0).expect("hit test");
    let run = hit.expect("a run under the point");
    assert_eq!(run.text, "PDFree - page one");
    assert_eq!(run.font_name, "Helvetica");
}

#[test]
fn point_with_no_text_returns_none() {
    skip_without_pdfium!();

    let miss = editor::text_run_at_point(SAMPLE, 0, 400.0, 100.0).expect("hit test");
    assert!(miss.is_none());
}

#[test]
fn text_run_at_point_rejects_an_out_of_range_page() {
    skip_without_pdfium!();

    let err = editor::text_run_at_point(SAMPLE, 9, 0.0, 0.0).expect_err("page 9 does not exist");
    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn replaces_text_in_place_preserving_the_font() {
    skip_without_pdfium!();

    let edited = editor::replace_text(SAMPLE, 0, "page one", "chapter one").expect("replace_text");

    let runs = editor::text_runs(&edited).expect("text_runs after edit");
    let page0 = runs.iter().find(|r| r.page == 0).expect("page 0 run");
    assert_eq!(page0.text, "PDFree - chapter one");
    assert_eq!(
        page0.font_name, "Helvetica",
        "font must be unchanged — same object"
    );

    // Page 1 must be untouched.
    let page1 = runs.iter().find(|r| r.page == 1).expect("page 1 run");
    assert_eq!(page1.text, "PDFree - page two");

    // And the edit must actually render.
    let before = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let after = Document::from_bytes(edited, None).unwrap();
    let png_before = before
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    let png_after = after
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    assert_ne!(png_before, png_after, "the text edit must render");
}

#[test]
fn replace_text_rejects_an_out_of_range_page() {
    skip_without_pdfium!();

    let err = editor::replace_text(SAMPLE, 9, "x", "y").expect_err("page 9 does not exist");
    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn replace_text_rejects_a_search_string_with_no_matches() {
    skip_without_pdfium!();

    let err = editor::replace_text(SAMPLE, 0, "no-such-text-on-this-page", "x")
        .expect_err("search text is not on the page");
    assert!(
        matches!(err, PdfError::TextNotFound { page: 0, .. }),
        "got {err:?}"
    );
}
