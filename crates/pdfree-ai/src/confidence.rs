//! Confidence scoring via grounding checks (Phase 5 quick win).
//!
//! Before an AI-produced value (a RAG answer, a smart form-fill suggestion,
//! an extracted field) is shown to the user, check whether it's actually
//! grounded in the source document rather than hallucinated. This is a plain
//! text search against the document's own extracted text — no extra model
//! call, no network — so every AI feature in the Phase 5+ roadmap can run it
//! on every value it produces, not just as an occasional expensive QA pass.
//! It honors the same "no silent uploads, be honest about confidence" spirit
//! as the rest of `pdfree-ai`'s local-first design.

/// How well a value is supported by the source document's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grounding {
    /// The value appears verbatim (case- and whitespace-insensitive) as one
    /// contiguous run in the document.
    Exact,
    /// Every word in the value appears somewhere in the document, but not
    /// together as one contiguous run — weaker evidence than `Exact` (e.g.
    /// the words are true but were stitched together from different parts
    /// of the page).
    Partial,
    /// No meaningful overlap found; likely hallucinated or from outside the
    /// document.
    Ungrounded,
}

/// The result of checking one AI-produced value against a document's text.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundingCheck {
    /// How the value was (or wasn't) found in the document.
    pub grounding: Grounding,
    /// A confidence score in `0.0..=1.0`, derived deterministically from
    /// `grounding` (1.0 for `Exact`, 0.5 for `Partial`, 0.0 for
    /// `Ungrounded`) rather than a learned or model-reported score — this is
    /// meant to be simple enough to explain to a user, not a probability
    /// estimate.
    pub confidence: f32,
    /// A short excerpt of the document text surrounding the match, present
    /// only for `Grounding::Exact`.
    pub context: Option<String>,
}

/// How much surrounding text to include on each side of an exact match, in
/// characters, when building [`GroundingCheck::context`].
const EXCERPT_RADIUS_CHARS: usize = 40;

/// Check whether `value` is grounded in `document_text` — typically the
/// output of [`pdfree_core::convert::to_text`] for the document the value
/// was supposedly drawn from.
///
/// Matching is case-insensitive and whitespace-normalized (runs of
/// whitespace collapse to a single space) so line wrapping and extra
/// spacing in extracted PDF text don't cause a false negative. A blank
/// `value` is always `Ungrounded` — there's nothing to ground.
///
/// This is a pure function: it cannot fail, so unlike the rest of
/// `pdfree-ai` it returns [`GroundingCheck`] directly rather than
/// [`crate::Result`].
#[must_use]
pub fn ground_check(document_text: &str, value: &str) -> GroundingCheck {
    let needle = normalize(value.trim());
    if needle.is_empty() {
        return ungrounded();
    }

    let haystack = normalize(document_text);

    if let Some(byte_pos) = haystack.find(&needle) {
        return GroundingCheck {
            grounding: Grounding::Exact,
            confidence: 1.0,
            context: Some(excerpt(&haystack, byte_pos, needle.len())),
        };
    }

    let words: Vec<&str> = needle.split(' ').filter(|w| !w.is_empty()).collect();
    if !words.is_empty() && words.iter().all(|word| haystack.contains(word)) {
        return GroundingCheck {
            grounding: Grounding::Partial,
            confidence: 0.5,
            context: None,
        };
    }

    ungrounded()
}

fn ungrounded() -> GroundingCheck {
    GroundingCheck {
        grounding: Grounding::Ungrounded,
        confidence: 0.0,
        context: None,
    }
}

/// Lowercase and collapse whitespace runs to single spaces, so PDF text
/// extraction's line breaks and irregular spacing don't defeat matching.
fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A short slice of `haystack` centered on a byte match at
/// `[match_start, match_start + match_len)`, snapped to `char` boundaries so
/// it never panics on multi-byte UTF-8 (accented names, curly quotes, etc.,
/// are common in real-world documents).
fn excerpt(haystack: &str, match_start: usize, match_len: usize) -> String {
    let start = floor_char_boundary(haystack, match_start.saturating_sub(EXCERPT_RADIUS_CHARS));
    let end = ceil_char_boundary(
        haystack,
        (match_start + match_len + EXCERPT_RADIUS_CHARS).min(haystack.len()),
    );
    haystack[start..end].to_string()
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn exact_match_scores_full_confidence_with_context() {
        let doc = "The total amount due is $1,234.56 payable within 30 days.";
        let check = ground_check(doc, "$1,234.56");
        assert_eq!(check.grounding, Grounding::Exact);
        assert!((check.confidence - 1.0).abs() < f32::EPSILON);
        assert!(check.context.expect("context").contains("$1,234.56"));
    }

    #[test]
    fn matching_is_case_and_whitespace_insensitive() {
        let doc = "Signed by   JANE   Doe\non March 1st.";
        let check = ground_check(doc, "jane doe");
        assert_eq!(check.grounding, Grounding::Exact);
    }

    #[test]
    fn partial_match_scores_half_confidence() {
        // Both words appear in the document, but never contiguously.
        let doc = "Invoice number 42. Billed to: Acme Corp.";
        let check = ground_check(doc, "Acme 42");
        assert_eq!(check.grounding, Grounding::Partial);
        assert!((check.confidence - 0.5).abs() < f32::EPSILON);
        assert!(check.context.is_none());
    }

    #[test]
    fn unrelated_value_is_ungrounded() {
        let doc = "This document says nothing about the requested value.";
        let check = ground_check(doc, "Quantum Widget 9000");
        assert_eq!(check.grounding, Grounding::Ungrounded);
        assert!(check.confidence.abs() < f32::EPSILON);
    }

    #[test]
    fn a_blank_value_is_always_ungrounded() {
        let check = ground_check("some document text", "   ");
        assert_eq!(check.grounding, Grounding::Ungrounded);
    }

    #[test]
    fn excerpt_extraction_does_not_panic_on_multibyte_text() {
        let doc = "Café résumé: naïve façade — the client's name is Zoë Müller, signed.";
        let check = ground_check(doc, "Zoë Müller");
        assert_eq!(check.grounding, Grounding::Exact);
        assert!(check.context.expect("context").contains("zoë müller"));
    }
}
