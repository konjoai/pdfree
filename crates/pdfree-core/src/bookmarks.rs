//! Document outline / bookmarks panel (Phase 4 quick win).
//!
//! `pdfium-render` already binds `PDFium`'s bookmark tree (`PdfBookmarks`,
//! `PdfBookmark`) — this module just walks it into a plain, `Send`-able tree
//! a shell can render directly as an outline sidebar, resolving each
//! bookmark's destination to a page index up front so the shell doesn't need
//! to touch `PDFium` types itself.
//!
//! A bookmark with no destination (an action `PDFium` doesn't resolve to a
//! page, or no destination at all) reports `page: None` rather than being
//! dropped — a shell can still show its title, just without a jump target.

use pdfium_render::prelude::*;

use crate::error::Result;

/// Defensive caps against a pathological or maliciously crafted bookmark
/// tree (very deep nesting, or a cycle) blowing the stack or producing an
/// unbounded result — mirrors the `MAX_EDGE_PIXELS` guard in `renderer.rs`.
const MAX_BOOKMARK_DEPTH: usize = 64;
const MAX_BOOKMARKS: usize = 10_000;

/// One node in a document's outline tree.
#[derive(Debug, Clone)]
pub struct Bookmark {
    /// The bookmark's display title.
    pub title: String,
    /// The 0-based page index this bookmark jumps to, if `PDFium` can
    /// resolve one.
    pub page: Option<u16>,
    /// Nested bookmarks, in document order.
    pub children: Vec<Bookmark>,
}

/// Read the document's outline (bookmark tree), if it has one.
///
/// Returns an empty `Vec` for a document with no bookmarks — this is the
/// common case (most PDFs have no outline) and is not an error.
///
/// # Errors
///
/// Returns an error if `PDFium` cannot be loaded or the bytes are not a
/// readable PDF.
pub fn outline(pdf_bytes: &[u8]) -> Result<Vec<Bookmark>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;

    let mut budget = MAX_BOOKMARKS;
    Ok(build_siblings(document.bookmarks().root(), 0, &mut budget))
}

/// Walk a chain of siblings (starting at `node`) and their children,
/// depth- and count-bounded.
fn build_siblings(
    node: Option<PdfBookmark<'_>>,
    depth: usize,
    budget: &mut usize,
) -> Vec<Bookmark> {
    let mut out = Vec::new();
    let mut current = node;

    while let Some(bookmark) = current {
        if *budget == 0 || depth >= MAX_BOOKMARK_DEPTH {
            break;
        }
        *budget -= 1;

        let title = bookmark.title().unwrap_or_default();
        let page = bookmark
            .destination()
            .and_then(|destination| destination.page_index().ok());
        let children = build_siblings(bookmark.first_child(), depth + 1, budget);

        out.push(Bookmark {
            title,
            page,
            children,
        });

        current = bookmark.next_sibling();
    }

    out
}
