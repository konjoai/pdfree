//! Document auto-summary (Phase 5).
//!
//! Extracts text via `pdfree-core`, then asks a [`Provider`] to summarize
//! it. Documents too large for one prompt fall back to map-reduce:
//! summarize each chunk independently, then summarize the summaries.

use crate::provider::Provider;
use crate::{AiError, Result};

/// Conservative single-pass ceiling. Local models in particular have
/// limited context windows; staying well under it also leaves headroom for
/// the prompt wrapper and the model's own response.
const MAX_SINGLE_PASS_WORDS: usize = 6000;

/// Summarize a PDF document.
pub fn summarize(pdf_bytes: &[u8], provider: &dyn Provider) -> Result<String> {
    let text = pdfree_core::convert::to_text(pdf_bytes)?;
    if text.split_whitespace().next().is_none() {
        return Err(AiError::Provider(
            "document contains no extractable text".to_string(),
        ));
    }

    summarize_text(&text, provider)
}

/// Summarize arbitrary extracted text, map-reducing if it's too long for a
/// single pass.
fn summarize_text(text: &str, provider: &dyn Provider) -> Result<String> {
    let word_count = text.split_whitespace().count();
    if word_count <= MAX_SINGLE_PASS_WORDS {
        return summarize_pass(text, provider);
    }

    // No overlap needed between chunks here — unlike rag::retrieve, nothing
    // depends on boundary continuity, just coverage.
    let chunks = crate::rag::chunk(text, MAX_SINGLE_PASS_WORDS, 0);
    let mut partial_summaries = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        partial_summaries.push(summarize_pass(chunk, provider)?);
    }

    let combined = partial_summaries.join("\n\n");
    summarize_pass(&combined, provider)
}

fn summarize_pass(text: &str, provider: &dyn Provider) -> Result<String> {
    let prompt = format!(
        "Summarize the following document text in a concise paragraph, \
         covering the main points a reader would need. Do not add \
         information that isn't present in the text.\n\nText:\n{text}\n\nSummary:"
    );
    provider.complete(&prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{OllamaProvider, Residency};
    use std::cell::RefCell;

    /// Records how many times `complete` was called instead of hitting a
    /// real model — lets the map-reduce branching be tested deterministically
    /// and fast, without a network dependency.
    struct CountingProvider {
        calls: RefCell<u32>,
    }

    impl CountingProvider {
        fn new() -> Self {
            Self {
                calls: RefCell::new(0),
            }
        }
    }

    impl Provider for CountingProvider {
        fn name(&self) -> &str {
            "counting-test-provider"
        }

        fn residency(&self) -> Residency {
            Residency::Local
        }

        fn complete(&self, _prompt: &str) -> crate::Result<String> {
            let mut calls = self.calls.borrow_mut();
            *calls += 1;
            Ok(format!("summary-{calls}"))
        }
    }

    fn words(count: usize) -> String {
        (0..count)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn short_text_is_a_single_pass() {
        let provider = CountingProvider::new();
        let result = summarize_text(&words(10), &provider).unwrap();
        assert_eq!(result, "summary-1");
        assert_eq!(*provider.calls.borrow(), 1);
    }

    #[test]
    fn long_text_map_reduces_across_multiple_passes() {
        let provider = CountingProvider::new();
        // Comfortably over the single-pass ceiling, so this must chunk.
        let long_text = words(MAX_SINGLE_PASS_WORDS * 3);
        summarize_text(&long_text, &provider).unwrap();
        // At least 3 chunk passes + 1 reduce pass.
        assert!(
            *provider.calls.borrow() >= 4,
            "calls: {}",
            provider.calls.borrow()
        );
    }

    /// Real end-to-end pass: extract text from a genuine IRS form via
    /// PDFium and summarize it with a real local Ollama model. Skips
    /// (doesn't fail) when either dependency is unavailable in this
    /// environment — same pattern as pdfree-core's own
    /// `skip_without_pdfium!()` tests.
    #[test]
    fn summarize_produces_real_text_from_a_real_document() {
        let pdf_bytes = include_bytes!("../../pdfree-core/tests/fixtures/irs_f1040.pdf");
        if pdfree_core::pdfium::bind().is_err() {
            eprintln!("skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable");
            return;
        }

        let provider = OllamaProvider::new("qwen3:8b");
        match summarize(pdf_bytes, &provider) {
            Ok(text) => assert!(!text.trim().is_empty()),
            Err(e) => eprintln!("skipping: provider unavailable ({e})"),
        }
    }
}
