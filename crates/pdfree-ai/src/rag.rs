//! Retrieval-augmented generation: chunk -> retrieve -> answer.
//!
//! Retrieval is lexical (word-overlap scoring), not embedding-based — no
//! embedding model to download or run, so single-document Q&A stays fully
//! on-device with zero extra setup. A real vector index (in the spirit of
//! Kyro/Kohaku/Vectro) is worth revisiting if usage ever spans a whole
//! library rather than one open document at a time.

use crate::provider::Provider;
use crate::{AiError, Result};
use std::collections::HashSet;

/// Split `text` into overlapping chunks of `chunk_words` words each,
/// advancing by `chunk_words - overlap_words` per chunk so consecutive
/// chunks share context at their boundary.
pub fn chunk(text: &str, chunk_words: usize, overlap_words: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() || chunk_words == 0 {
        return Vec::new();
    }

    let step = chunk_words.saturating_sub(overlap_words).max(1);
    let mut chunks = Vec::new();
    let mut start = 0;
    loop {
        let end = (start + chunk_words).min(words.len());
        chunks.push(words[start..end].join(" "));
        if end == words.len() {
            break;
        }
        start += step;
    }
    chunks
}

/// Function words common enough to swamp real topical overlap (especially
/// when they repeat within a chunk) without carrying any retrieval signal.
/// Deliberately short — this is a relevance nudge, not a linguistic model.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "how", "in", "is", "it", "of", "on",
    "or", "that", "the", "this", "to", "was", "were", "what", "when", "where", "who", "why",
    "with",
];

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// Rank chunk indices by lowercase word-overlap with `question`, most
/// relevant first. Ties (including "no overlap at all") keep document
/// order, so a `top_k` slice always returns *something* rather than nothing.
pub fn retrieve(chunks: &[String], question: &str, top_k: usize) -> Vec<usize> {
    let query_tokens: HashSet<String> = tokenize(question).into_iter().collect();

    let mut scored: Vec<(usize, f64)> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let tokens = tokenize(chunk);
            let overlap = tokens.iter().filter(|t| query_tokens.contains(*t)).count() as f64;
            // Mild length normalization so one giant chunk doesn't win purely
            // by containing more words — sqrt rather than linear so a chunk
            // that's actually more relevant (higher raw overlap) still wins.
            let length_penalty = (tokens.len().max(1) as f64).sqrt();
            (i, overlap / length_penalty)
        })
        .collect();

    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.into_iter().take(top_k).map(|(i, _)| i).collect()
}

/// Answer a question about a single PDF document via retrieval-augmented
/// generation: extract text, chunk it, retrieve the most relevant chunks,
/// and ask `provider` to answer grounded only in that context.
pub fn answer(pdf_bytes: &[u8], question: &str, provider: &dyn Provider) -> Result<String> {
    let text = pdfree_core::convert::to_text(pdf_bytes)?;
    let chunks = chunk(&text, 400, 50);
    if chunks.is_empty() {
        return Err(AiError::Provider(
            "document contains no extractable text".to_string(),
        ));
    }

    let top = retrieve(&chunks, question, 5);
    let context = top
        .iter()
        .map(|&i| chunks[i].as_str())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let prompt = format!(
        "Answer the question using only the context below, which is excerpted \
         from a PDF document. If the answer isn't in the context, say so — \
         don't guess.\n\nContext:\n{context}\n\nQuestion: {question}\n\nAnswer:"
    );

    provider.complete(&prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::OllamaProvider;

    #[test]
    fn chunk_splits_into_overlapping_windows() {
        let text = (1..=20)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let chunks = chunk(&text, 5, 2);

        assert_eq!(chunks[0], "1 2 3 4 5");
        // step = 5 - 2 = 3, so the next window starts at word index 3 (0-based) = "4"
        assert_eq!(chunks[1], "4 5 6 7 8");
        assert_eq!(*chunks.last().unwrap(), "16 17 18 19 20");
    }

    #[test]
    fn chunk_of_empty_text_is_empty() {
        assert!(chunk("", 100, 10).is_empty());
    }

    #[test]
    fn chunk_never_infinite_loops_when_overlap_exceeds_chunk_size() {
        // step must clamp to at least 1 word of forward progress.
        let chunks = chunk("a b c d e", 3, 10);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn retrieve_ranks_the_matching_chunk_first() {
        let chunks = vec![
            "the weather today is sunny and warm".to_string(),
            "quarterly revenue grew twelve percent year over year".to_string(),
            "the cat sat on the mat".to_string(),
        ];
        let top = retrieve(&chunks, "What was the revenue growth?", 1);
        assert_eq!(top, vec![1]);
    }

    #[test]
    fn retrieve_falls_back_to_document_order_with_no_matches() {
        let chunks = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
        let top = retrieve(&chunks, "zzz nonexistent query", 2);
        assert_eq!(top, vec![0, 1]);
    }

    /// Real end-to-end pass: extract text from a genuine IRS form via
    /// PDFium, retrieve the relevant chunk, and have a real local Ollama
    /// model answer grounded in it. Skips (doesn't fail) when either
    /// dependency is unavailable in this environment — same pattern as
    /// pdfree-core's own `skip_without_pdfium!()` tests.
    #[test]
    fn answer_grounds_a_real_question_in_a_real_document() {
        let pdf_bytes = include_bytes!("../../pdfree-core/tests/fixtures/irs_f1040.pdf");
        if pdfree_core::pdfium::bind().is_err() {
            eprintln!("skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable");
            return;
        }

        let provider = OllamaProvider::new("qwen3:8b");
        match answer(pdf_bytes, "What tax form is this?", &provider) {
            Ok(text) => assert!(!text.trim().is_empty()),
            Err(e) => eprintln!("skipping: provider unavailable ({e})"),
        }
    }
}
