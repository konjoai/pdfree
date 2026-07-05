//! Phase 4 quick-win acceptance tests: document outline / bookmarks panel.
//!
//! Like `tests/render.rs`, these skip with a notice (rather than fail) when
//! `PDFium` isn't bundled, so a bare checkout still builds green. Run
//! `scripts/fetch-pdfium.sh` first to make them exercise `PDFium` for real.
//!
//! Test code may `unwrap`/`expect` freely (see `.github/copilot-instructions.md`)
//! — the production-code ban only applies to `pdfree-core`'s library surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::bookmarks;

/// 2-page fixture with no outline at all.
const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");

/// 3-page fixture (generated via `pypdf`) with a two-level outline:
/// "Chapter 1" (page 0) -> "Section 1.1" (page 1), then "Chapter 2" (page 2)
/// as a second top-level sibling.
const OUTLINE_SAMPLE: &[u8] = include_bytes!("fixtures/outline_sample.pdf");

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
fn a_document_with_no_outline_returns_an_empty_list() {
    skip_without_pdfium!();

    let tree = bookmarks::outline(SAMPLE).expect("outline");
    assert!(tree.is_empty());
}

#[test]
fn reads_top_level_titles_in_document_order() {
    skip_without_pdfium!();

    let tree = bookmarks::outline(OUTLINE_SAMPLE).expect("outline");
    assert_eq!(tree.len(), 2, "two top-level bookmarks");
    assert_eq!(tree[0].title, "Chapter 1");
    assert_eq!(tree[1].title, "Chapter 2");
}

#[test]
fn resolves_each_bookmark_to_its_destination_page() {
    skip_without_pdfium!();

    let tree = bookmarks::outline(OUTLINE_SAMPLE).expect("outline");
    assert_eq!(tree[0].page, Some(0));
    assert_eq!(tree[1].page, Some(2));
}

#[test]
fn nests_child_bookmarks_under_their_parent() {
    skip_without_pdfium!();

    let tree = bookmarks::outline(OUTLINE_SAMPLE).expect("outline");
    let chapter_1 = &tree[0];
    assert_eq!(chapter_1.children.len(), 1);
    assert_eq!(chapter_1.children[0].title, "Section 1.1");
    assert_eq!(chapter_1.children[0].page, Some(1));

    let chapter_2 = &tree[1];
    assert!(chapter_2.children.is_empty());
}
