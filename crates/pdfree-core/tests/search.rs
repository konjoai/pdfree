//! Phase 4 quick-win acceptance tests: in-document text search.
//!
//! Like `tests/render.rs`, these skip with a notice (rather than fail) when
//! `PDFium` isn't bundled, so a bare checkout still builds green. Run
//! `scripts/fetch-pdfium.sh` first to make them exercise `PDFium` for real.
//!
//! Test code may `unwrap`/`expect` freely (see `.github/copilot-instructions.md`)
//! — the production-code ban only applies to `pdfree-core`'s library surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::search;

/// 2-page fixture: page 0 renders "`PDFree` - page one", page 1 renders
/// "`PDFree` - page two", both 24pt Helvetica.
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
fn finds_a_query_present_on_both_pages() {
    skip_without_pdfium!();

    let matches = search::find_text(SAMPLE, "PDFree", true).expect("find_text");
    assert_eq!(matches.len(), 2, "one run per page contains the query");
    assert!(matches.iter().any(|m| m.page == 0));
    assert!(matches.iter().any(|m| m.page == 1));
    assert!(matches.iter().all(|m| m.occurrences == 1));
    assert!(matches.iter().all(|m| m.width > 0.0 && m.height > 0.0));
}

#[test]
fn finds_a_query_present_on_only_one_page() {
    skip_without_pdfium!();

    let matches = search::find_text(SAMPLE, "page one", true).expect("find_text");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].page, 0);
    assert_eq!(matches[0].text, "PDFree - page one");
}

#[test]
fn case_insensitive_search_matches_regardless_of_case() {
    skip_without_pdfium!();

    let matches = search::find_text(SAMPLE, "PDFREE", false).expect("find_text");
    assert_eq!(matches.len(), 2);
}

#[test]
fn case_sensitive_search_rejects_a_case_mismatch() {
    skip_without_pdfium!();

    let matches = search::find_text(SAMPLE, "PDFREE", true).expect("find_text");
    assert!(matches.is_empty());
}

#[test]
fn a_query_with_no_matches_returns_an_empty_list() {
    skip_without_pdfium!();

    let matches = search::find_text(SAMPLE, "no such text anywhere", true).expect("find_text");
    assert!(matches.is_empty());
}

#[test]
fn an_empty_query_returns_an_empty_list_not_every_run() {
    skip_without_pdfium!();

    let matches = search::find_text(SAMPLE, "", true).expect("find_text");
    assert!(matches.is_empty());
}
