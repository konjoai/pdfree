//! PII detection and one-click redaction (Phase 6).
//!
//! Detection is regex-based (SSNs, emails, phone numbers, credit cards) —
//! deterministic and fully local, no LLM call needed for these common
//! structured PII kinds. Redaction reuses `pdfree_core::editor::replace_text`,
//! which mutates the matched text object's own content in place — so the
//! original PII text is actually overwritten in the document's content
//! stream, not just visually covered by a box on top of it.
//!
//! **Known scope boundary**: matched positions are the *containing text
//! run's* bounding box (`pdfree_core::editor::TextRun` has no sub-string
//! bounds), so `PiiSpan::{x,y,width,height}` locate "the run this PII is
//! in," not a glyph-precise box around just the matched substring — good
//! enough for a review UI to highlight the run, not for drawing a tight
//! redaction rectangle.

use crate::Result;
use pdfree_core::editor;
use regex::Regex;
use std::sync::OnceLock;

/// The kind of PII a [`PiiSpan`] matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiKind {
    Ssn,
    Email,
    Phone,
    CreditCard,
}

/// A detected span of personally-identifiable information.
#[derive(Debug, Clone, PartialEq)]
pub struct PiiSpan {
    /// 0-based page index.
    pub page: u16,
    pub kind: PiiKind,
    /// The exact matched text (e.g. `"123-45-6789"`).
    pub text: String,
    /// Bounding box of the *containing text run* — see module docs.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

fn ssn_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap())
}

fn email_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[[:alnum:].+_-]+@[[:alnum:].-]+\.[[:alpha:]]{2,}\b").unwrap())
}

fn phone_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(?:\+?1[-. ]?)?\(?\d{3}\)?[-. ]\d{3}[-. ]\d{4}\b").unwrap())
}

fn credit_card_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Must end on a digit, not a trailing separator — `(?:\d[ -]?){13,16}`
    // lets the final repetition consume a trailing space before `\b`, which
    // still satisfies the boundary (space is a non-word char) and silently
    // swallows one character of whatever follows the number.
    RE.get_or_init(|| Regex::new(r"\b\d(?:[ -]?\d){12,15}\b").unwrap())
}

/// Luhn checksum — filters the credit-card pattern's many false positives
/// (any 13-16 digit run, e.g. an invoice or tracking number) down to
/// sequences that are actually valid card numbers.
fn passes_luhn(digits: &str) -> bool {
    let digits: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() < 13 {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                d
            }
        })
        .sum();
    sum % 10 == 0
}

/// Find every PII match in a single string of text, in order of appearance.
fn find_matches(text: &str) -> Vec<(PiiKind, String)> {
    let mut matches: Vec<(usize, PiiKind, String)> = Vec::new();

    for m in ssn_pattern().find_iter(text) {
        matches.push((m.start(), PiiKind::Ssn, m.as_str().to_string()));
    }
    for m in email_pattern().find_iter(text) {
        matches.push((m.start(), PiiKind::Email, m.as_str().to_string()));
    }
    for m in phone_pattern().find_iter(text) {
        matches.push((m.start(), PiiKind::Phone, m.as_str().to_string()));
    }
    for m in credit_card_pattern().find_iter(text) {
        if passes_luhn(m.as_str()) {
            matches.push((m.start(), PiiKind::CreditCard, m.as_str().to_string()));
        }
    }

    matches.sort_by_key(|(start, ..)| *start);
    matches
        .into_iter()
        .map(|(_, kind, text)| (kind, text))
        .collect()
}

/// Detect PII across every page of a document by pattern-matching its text
/// runs.
pub fn detect_pii(pdf_bytes: &[u8]) -> Result<Vec<PiiSpan>> {
    let runs = editor::text_runs(pdf_bytes)?;
    let mut spans = Vec::new();
    for run in &runs {
        for (kind, text) in find_matches(&run.text) {
            spans.push(PiiSpan {
                page: run.page,
                kind,
                text,
                x: run.x,
                y: run.y,
                width: run.width,
                height: run.height,
            });
        }
    }
    Ok(spans)
}

/// Redact the given spans by overwriting each match's text with a
/// same-length placeholder, returning the updated document bytes. Typically
/// called with (a subset of) `detect_pii`'s output, after a user has
/// reviewed and confirmed which spans to redact.
///
/// The placeholder is `'X'`, not a block character (`█`) — the standard-14
/// PDF fonts `pdfree_core` writes text in (WinAnsiEncoding) have no glyph
/// outside the Latin-1 range, so a block char would either fail to encode
/// or silently vanish on read-back, defeating the point of a *visible*
/// redaction mark.
pub fn redact(pdf_bytes: &[u8], spans: &[PiiSpan]) -> Result<Vec<u8>> {
    let mut bytes = pdf_bytes.to_vec();
    for span in spans {
        let placeholder = "X".repeat(span.text.chars().count());
        bytes = editor::replace_text(&bytes, span.page, &span.text, &placeholder)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_an_ssn() {
        let matches = find_matches("Applicant SSN: 123-45-6789 on file.");
        assert_eq!(matches, vec![(PiiKind::Ssn, "123-45-6789".to_string())]);
    }

    #[test]
    fn detects_an_email() {
        let matches = find_matches("Contact jane.doe@example.com for details.");
        assert_eq!(
            matches,
            vec![(PiiKind::Email, "jane.doe@example.com".to_string())]
        );
    }

    #[test]
    fn detects_a_phone_number() {
        let matches = find_matches("Call 555-123-4567 to confirm.");
        assert_eq!(matches, vec![(PiiKind::Phone, "555-123-4567".to_string())]);
    }

    #[test]
    fn detects_a_valid_credit_card_but_not_an_arbitrary_digit_run() {
        // A real (test) Visa number that passes Luhn.
        let matches = find_matches("Card 4111 1111 1111 1111 charged.");
        assert_eq!(
            matches,
            vec![(PiiKind::CreditCard, "4111 1111 1111 1111".to_string())]
        );

        // Same digit count, fails Luhn — e.g. an invoice number, not a card.
        let no_match = find_matches("Invoice 1234 5678 9012 3456 due.");
        assert!(no_match.is_empty());
    }

    #[test]
    fn finds_multiple_kinds_in_order() {
        let matches = find_matches("SSN 123-45-6789, email a@b.com, phone 555-000-1111.");
        assert_eq!(
            matches,
            vec![
                (PiiKind::Ssn, "123-45-6789".to_string()),
                (PiiKind::Email, "a@b.com".to_string()),
                (PiiKind::Phone, "555-000-1111".to_string()),
            ]
        );
    }

    #[test]
    fn plain_text_has_no_false_positives() {
        let matches = find_matches("The quick brown fox jumps over the lazy dog.");
        assert!(matches.is_empty());
    }

    /// Real end-to-end pass through PDFium: stamp PII onto a real page via
    /// `forms::overlay_text` (which creates genuine editable text objects,
    /// not annotations), detect it, redact it, and confirm the original PII
    /// text no longer appears anywhere in the redacted document's extracted
    /// text. Skips (doesn't fail) when PDFium isn't bundled — same pattern
    /// as `pdfree-core`'s own `skip_without_pdfium!()` tests.
    #[test]
    fn detect_and_redact_round_trip_on_a_real_document() {
        if pdfree_core::pdfium::bind().is_err() {
            eprintln!("skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable");
            return;
        }

        let blank_page = white_page_png();
        let pdf = pdfree_core::convert::from_image(&blank_page, 72.0).unwrap();
        let overlay = pdfree_core::forms::TextOverlay {
            page: 0,
            x: 50.0,
            y: 700.0,
            text: "SSN: 123-45-6789 Email: jane@example.com".to_string(),
            font_size: 14.0,
        };
        let with_pii = pdfree_core::forms::overlay_text(&pdf, &[overlay]).unwrap();

        let spans = detect_pii(&with_pii).unwrap();
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().any(|s| s.kind == PiiKind::Ssn));
        assert!(spans.iter().any(|s| s.kind == PiiKind::Email));

        let redacted = redact(&with_pii, &spans).unwrap();
        let redacted_text = pdfree_core::convert::to_text(&redacted).unwrap();
        assert!(!redacted_text.contains("123-45-6789"));
        assert!(!redacted_text.contains("jane@example.com"));
        assert!(redacted_text.contains("XXXXXXXXXXX"), "{redacted_text:?}");
    }

    fn white_page_png() -> Vec<u8> {
        let img = image::RgbImage::from_pixel(612, 792, image::Rgb([255, 255, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }
}
