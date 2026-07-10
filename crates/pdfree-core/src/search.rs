//! In-document text search (Phase 4 quick win — "⌘F").
//!
//! Reuses [`crate::editor::text_runs`]'s existing font/position wiring rather
//! than walking `PDFium`'s text-page API a second time: a run is already the
//! right granularity for a search hit — a shell can highlight the run's
//! bounding box directly, the same way it already hit-tests clicks against
//! runs for [`crate::editor::text_run_at_point`].
//!
//! **Known scope boundary**: a match's bounds are the *whole run's* bounding
//! box, not a tight box around just the matched substring — `pdfree-core`
//! doesn't do character-offset-precise sub-run rects yet (the same boundary
//! [`crate::editor::replace_text`] documents). For a run containing the query
//! more than once, [`SearchMatch::occurrences`] reports the count so a shell
//! can at least show "(2)" next to the highlight rather than pretending
//! there was only one hit.

use crate::editor::{self, TextRun};
use crate::error::Result;

/// One run of text that contains the search query at least once.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    /// 0-based page index the match is on.
    pub page: u16,
    /// The full text of the run containing the match (not just the matched
    /// substring — see the module docs' scope boundary).
    pub text: String,
    /// How many times the query occurs within this run's text.
    pub occurrences: usize,
    /// Horizontal position of the run's bounding box, from the page's left edge.
    pub x: f32,
    /// Vertical position of the run's bounding box, from the page's bottom edge.
    pub y: f32,
    /// Width of the run's bounding box.
    pub width: f32,
    /// Height of the run's bounding box.
    pub height: f32,
}

impl SearchMatch {
    fn from_run(run: TextRun, occurrences: usize) -> Self {
        Self {
            page: run.page,
            text: run.text,
            occurrences,
            x: run.x,
            y: run.y,
            width: run.width,
            height: run.height,
        }
    }
}

/// Find every text run across the document that contains `query`, in page
/// order.
///
/// An empty `query` matches nothing (rather than every run). Matching is a
/// plain substring search — no regex, no fuzzy/whitespace-normalized
/// matching yet.
///
/// # Errors
///
/// Returns an error if `PDFium` cannot be loaded or the bytes are not a
/// readable PDF.
pub fn find_text(pdf_bytes: &[u8], query: &str, case_sensitive: bool) -> Result<Vec<SearchMatch>> {
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let needle = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    let runs = editor::text_runs(pdf_bytes)?;
    let mut out = Vec::new();
    for run in runs {
        let haystack = if case_sensitive {
            run.text.clone()
        } else {
            run.text.to_lowercase()
        };
        let occurrences = haystack.matches(needle.as_str()).count();
        if occurrences > 0 {
            out.push(SearchMatch::from_run(run, occurrences));
        }
    }
    Ok(out)
}
