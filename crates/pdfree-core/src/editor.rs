//! In-place text editing with font detection (Phase 3).
//!
//! "Detect the font, then replace in place" is really two operations: read
//! every text run on a page with its font and position (so a shell can hit-
//! test a click against the reported bounds and show what font is under the
//! cursor), and mutate an existing text object's own content — which
//! preserves its font automatically, since it's the same object with the
//! same font resource, not a freshly created one that would need font
//! matching at all.

use pdfium_render::prelude::*;

use crate::error::{PdfError, Result};

/// One run of text on a page, with its font and position.
///
/// A "run" is one `PDFium` text page-object — typically a contiguous span of
/// text sharing a single font resource, roughly a line or a styled phrase,
/// not necessarily a whole paragraph.
#[derive(Debug, Clone)]
pub struct TextRun {
    /// 0-based page index this run is on.
    pub page: u16,
    /// The run's text content.
    pub text: String,
    /// The name of the font applied to this run.
    pub font_name: String,
    /// The font size in PDF points.
    pub font_size: f32,
    /// Horizontal position of the run's bounding box, from the page's left edge.
    pub x: f32,
    /// Vertical position of the run's bounding box, from the page's bottom edge.
    pub y: f32,
    /// Width of the run's bounding box.
    pub width: f32,
    /// Height of the run's bounding box.
    pub height: f32,
}

/// Enumerate every text run on every page of a document, with font and
/// position — the "detect font" half of the feature. A shell hit-tests a
/// click against each run's bounds to find what's under the cursor.
///
/// # Errors
///
/// Returns an error if `PDFium` cannot be loaded or the bytes are not a
/// readable PDF.
pub fn text_runs(pdf_bytes: &[u8]) -> Result<Vec<TextRun>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;

    let mut out = Vec::new();
    for (page_index, page) in document.pages().iter().enumerate() {
        for object in page.objects().iter() {
            let Some(text_object) = object.as_text_object() else {
                continue;
            };
            let bounds = text_object
                .bounds()
                .map(|q| q.to_rect())
                .unwrap_or(PdfRect::ZERO);
            let font_name = text_object.font().name();
            out.push(TextRun {
                // Page counts are u16 throughout this crate; page_index is
                // bounded by document.pages().len(), so this never truncates.
                #[allow(clippy::cast_possible_truncation)]
                page: page_index as u16,
                text: text_object.text(),
                font_name,
                font_size: text_object.unscaled_font_size().value,
                x: bounds.left().value,
                y: bounds.bottom().value,
                width: bounds.width().value,
                height: bounds.height().value,
            });
        }
    }
    Ok(out)
}

/// Find the text run at a page-space point, if any — a convenience over
/// [`text_runs`] for a shell that already has a click position in PDF points
/// rather than a full list of run bounds to hit-test itself.
///
/// # Errors
///
/// Returns [`PdfError::PageOutOfRange`] if `page` doesn't exist, and
/// propagates `PDFium` / load errors otherwise.
pub fn text_run_at_point(pdf_bytes: &[u8], page: u16, x: f32, y: f32) -> Result<Option<TextRun>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    let count = document.pages().len();
    if page >= count {
        return Err(PdfError::PageOutOfRange { index: page, count });
    }

    let loaded = document.pages().get(page)?;
    let page_text = loaded.text()?;
    let chars = page_text.chars();
    let Some(character) = chars.get_char_at_point(PdfPoints::new(x), PdfPoints::new(y)) else {
        return Ok(None);
    };
    let text_object = character.text_object()?;
    let bounds = text_object
        .bounds()
        .map(|q| q.to_rect())
        .unwrap_or(PdfRect::ZERO);
    let font_name = text_object.font().name();

    Ok(Some(TextRun {
        page,
        text: text_object.text(),
        font_name,
        font_size: text_object.unscaled_font_size().value,
        x: bounds.left().value,
        y: bounds.bottom().value,
        width: bounds.width().value,
        height: bounds.height().value,
    }))
}

/// Replace every occurrence of `find` with `replace`, within every text run
/// on `page` that contains at least one occurrence, returning the updated
/// PDF as new bytes.
///
/// Mutates the matching text object's own content in place, so its font is
/// preserved exactly — no font-matching heuristic is involved. If a run
/// contains `find` more than once, every occurrence in that run is replaced
/// together (there is no "replace just this one instance" — pdfree-core
/// doesn't do character-offset-precise partial edits within a run yet).
///
/// # Errors
///
/// Returns [`PdfError::PageOutOfRange`] if `page` doesn't exist,
/// [`PdfError::TextNotFound`] if no run on the page contains `find`, and
/// propagates `PDFium` / load errors otherwise.
pub fn replace_text(pdf_bytes: &[u8], page: u16, find: &str, replace: &str) -> Result<Vec<u8>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    let count = document.pages().len();
    if page >= count {
        return Err(PdfError::PageOutOfRange { index: page, count });
    }

    let mut loaded = document.pages().get(page)?;
    let mut found = false;
    for mut object in loaded.objects().iter() {
        let Some(text_object) = object.as_text_object_mut() else {
            continue;
        };
        let original = text_object.text();
        if !original.contains(find) {
            continue;
        }
        found = true;
        text_object.set_text(original.replace(find, replace))?;
    }

    if !found {
        return Err(PdfError::TextNotFound {
            page,
            find: find.to_string(),
        });
    }

    // Unlike adding/removing an object, mutating an existing text object's
    // content doesn't automatically regenerate the page's content stream —
    // confirmed empirically: without this, set_text() succeeds but the
    // change never makes it into save_to_bytes()'s output.
    loaded.regenerate_content()?;

    Ok(document.save_to_bytes()?)
}
