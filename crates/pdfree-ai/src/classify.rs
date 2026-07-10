//! Document classification (Phase 6).
//!
//! Classifies a document into a fixed label set by prompting a [`Provider`]
//! over its extracted text — not embeddings. Same tradeoff as `rag.rs`: no
//! local embedding model to download or run, so this stays fully on-device
//! (with Ollama) with zero extra setup. Vectro-backed semantic search and
//! whole-library organization are a separate, larger feature not pursued
//! here — worth revisiting only if usage ever spans a whole library rather
//! than one document at a time (see `rag.rs`'s equivalent note).

use crate::provider::Provider;
use crate::{AiError, Result};

/// The fixed label set `classify` chooses from. Kept small and closed so a
/// caller can build UI (icons, filters, folders) against a known set,
/// rather than an open-ended string the model could return anything for.
pub const LABELS: &[&str] = &[
    "contract", "invoice", "tax_form", "receipt", "letter", "form", "resume", "report", "other",
];

/// Only the first chunk of text is needed to identify a document's type —
/// keeps the prompt small and fast even for long documents.
const MAX_CLASSIFY_WORDS: usize = 1500;

/// Classify a document into one of [`LABELS`] using its extracted text.
pub fn classify(pdf_bytes: &[u8], provider: &dyn Provider) -> Result<String> {
    let text = pdfree_core::convert::to_text(pdf_bytes)?;
    if text.split_whitespace().next().is_none() {
        return Err(AiError::Provider(
            "document contains no extractable text".to_string(),
        ));
    }

    let excerpt = text
        .split_whitespace()
        .take(MAX_CLASSIFY_WORDS)
        .collect::<Vec<_>>()
        .join(" ");

    let prompt = format!(
        "Classify this document into exactly one of these categories: {}.\n\n\
         Respond with ONLY the category label, nothing else — no punctuation, \
         no explanation.\n\nDocument text:\n{excerpt}\n\nCategory:",
        LABELS.join(", ")
    );

    let response = provider.complete(&prompt)?;
    Ok(parse_label(&response))
}

/// Extract a label from the model's raw response — case-insensitive
/// substring match against [`LABELS`], tolerating extra whitespace or
/// punctuation the model adds despite being asked not to. Falls back to
/// `"other"` if nothing matches, so a hallucinated label never reaches a
/// caller's UI as an unrecognized value.
fn parse_label(response: &str) -> String {
    let normalized = response.trim().to_lowercase();
    for &label in LABELS {
        if normalized.contains(label) {
            return label.to_string();
        }
    }
    "other".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Residency;

    struct FixedProvider {
        response: String,
    }

    impl Provider for FixedProvider {
        fn name(&self) -> &str {
            "fixed-test-provider"
        }
        fn residency(&self) -> Residency {
            Residency::Local
        }
        fn complete(&self, _prompt: &str) -> crate::Result<String> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn parses_an_exact_label() {
        assert_eq!(parse_label("invoice"), "invoice");
    }

    #[test]
    fn parses_a_label_with_surrounding_prose() {
        assert_eq!(
            parse_label("This document is a tax_form, specifically a 1040."),
            "tax_form"
        );
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(parse_label("CONTRACT"), "contract");
    }

    #[test]
    fn distinguishes_tax_form_from_the_shorter_form_label() {
        assert_eq!(parse_label("tax_form"), "tax_form");
        assert_eq!(parse_label("form"), "form");
    }

    #[test]
    fn unrecognized_response_falls_back_to_other() {
        assert_eq!(parse_label("I cannot determine the category."), "other");
    }

    #[test]
    fn empty_document_is_a_provider_error_not_a_model_call() {
        // A blank PDF has no extractable text, so classify() should fail
        // fast rather than spend a request on it.
        let blank = image::RgbImage::from_pixel(64, 64, image::Rgb([255, 255, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(blank)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let Ok(pdf) = pdfree_core::convert::from_image(&bytes, 72.0) else {
            eprintln!("skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable");
            return;
        };
        let provider = FixedProvider {
            response: "this should never be read".to_string(),
        };
        let result = classify(&pdf, &provider);
        assert!(matches!(result, Err(AiError::Provider(_))));
    }

    /// Real end-to-end pass against a real local Ollama model. Doesn't
    /// assert on the exact label (small local models don't always agree
    /// with "tax_form" vs. "form") — just that the round trip works and
    /// returns something from the known label set. Skips (doesn't fail)
    /// when either PDFium or Ollama is unavailable in this environment.
    #[test]
    fn classifies_a_real_document_with_a_real_model() {
        use crate::provider::OllamaProvider;

        let fixture = include_bytes!("../../pdfree-core/tests/fixtures/irs_f1040.pdf");
        if pdfree_core::pdfium::bind().is_err() {
            eprintln!("skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable");
            return;
        }

        let provider = OllamaProvider::new("qwen3:8b");
        match classify(fixture, &provider) {
            Ok(label) => assert!(
                LABELS.contains(&label.as_str()),
                "unexpected label: {label}"
            ),
            Err(e) => eprintln!("skipping: provider unavailable ({e})"),
        }
    }
}
