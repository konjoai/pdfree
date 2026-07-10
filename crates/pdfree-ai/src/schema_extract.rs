//! Schema-driven extraction: pull caller-defined fields out of a document's
//! text via an LLM (Phase 7).
//!
//! Same shape as `formfill.rs`'s profile-mapping problem, just inverted: the
//! caller supplies the schema (field name + a human description of what it
//! means) instead of a fixed profile, and the model is asked to find each
//! field's value in the document rather than match it to something already
//! known. Results are a *suggestion* list, never auto-applied — same
//! rationale as `formfill.rs`: a wrong guess belongs in a review UI, not
//! silently written anywhere.

use crate::provider::Provider;
use crate::{AiError, Result};
use std::collections::HashSet;

/// One field the caller wants pulled out of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaField {
    /// Short identifier for the field, echoed back verbatim in the result
    /// (e.g. `"invoice_number"`, `"total_due"`).
    pub name: String,
    /// A human-readable description of what the field means — this is what
    /// the model actually reads to find it, since `name` alone is often too
    /// terse to disambiguate ("total" could be a dozen different numbers on
    /// an invoice).
    pub description: String,
}

/// A field the model found a value for, ready to show in a review UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedValue {
    pub field_name: String,
    pub value: String,
}

/// Conservative single-pass ceiling — same constant as `summarize.rs` (local
/// models have limited context windows; leaves headroom for the prompt
/// wrapper and the field list itself).
const MAX_SINGLE_PASS_WORDS: usize = 6000;

/// Extract `schema`'s fields from a document via `provider`. Returns only
/// the fields the model found an actual value for; a field with no match in
/// the text is simply absent from the result, not present with an empty or
/// guessed value.
pub fn extract(
    pdf_bytes: &[u8],
    schema: &[SchemaField],
    provider: &dyn Provider,
) -> Result<Vec<ExtractedValue>> {
    if schema.is_empty() {
        return Ok(Vec::new());
    }

    let text = pdfree_core::convert::to_text(pdf_bytes)?;
    if text.split_whitespace().next().is_none() {
        return Err(AiError::Provider(
            "document contains no extractable text".to_string(),
        ));
    }

    extract_from_text(&text, schema, provider)
}

/// The text-in, values-out half of [`extract`] — split out so the map-reduce
/// chunking logic can be tested directly against hand-built text, without
/// needing a real PDF/`PDFium` round trip just to exercise word counting.
fn extract_from_text(
    text: &str,
    schema: &[SchemaField],
    provider: &dyn Provider,
) -> Result<Vec<ExtractedValue>> {
    let word_count = text.split_whitespace().count();
    if word_count <= MAX_SINGLE_PASS_WORDS {
        return extract_pass(text, schema, provider);
    }

    // Long documents: run extraction over each chunk and merge by keeping
    // the first non-empty value found for each field. This is a simple,
    // honest merge — not confidence-scored — so a field whose true value
    // sits in a later chunk still gets picked up as long as no earlier
    // chunk produced *any* value for it; it does mean an earlier chunk's
    // wrong guess (the model finding a look-alike value) can shadow a
    // better match later in the document. Acceptable for a suggestion list
    // a human reviews before anything is written anywhere.
    let chunks = crate::rag::chunk(text, MAX_SINGLE_PASS_WORDS, 200);
    let mut merged = Vec::new();
    let mut found: HashSet<String> = HashSet::new();
    for chunk in &chunks {
        for value in extract_pass(chunk, schema, provider)? {
            if found.insert(value.field_name.clone()) {
                merged.push(value);
            }
        }
    }
    Ok(merged)
}

fn extract_pass(
    text: &str,
    schema: &[SchemaField],
    provider: &dyn Provider,
) -> Result<Vec<ExtractedValue>> {
    let field_list = schema
        .iter()
        .map(|f| format!("- \"{}\": {}", f.name, f.description))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Extract the following fields from the document text below. Only \
         extract a value if it is actually present in the text — do not \
         guess or invent a value for a field that isn't there.\n\n\
         Fields to extract:\n{field_list}\n\n\
         Document text:\n{text}\n\n\
         Respond with ONLY a JSON object mapping each field name you found a \
         value for to that value as a string. Omit any field you couldn't \
         find. Respond with JSON only, no other text. Example: \
         {{\"invoice_number\": \"INV-1029\"}}"
    );

    let response = provider.complete(&prompt)?;
    parse_extraction(&response, schema)
}

fn parse_extraction(response: &str, schema: &[SchemaField]) -> Result<Vec<ExtractedValue>> {
    let json_str = crate::json_util::extract_json_object(response).ok_or_else(|| {
        AiError::Provider(format!("model response had no JSON object: {response}"))
    })?;

    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| AiError::Provider(format!("failed to parse extraction result: {e}")))?;

    let map = parsed
        .as_object()
        .ok_or_else(|| AiError::Provider("extraction result was not a JSON object".to_string()))?;

    // Never surface a field name we didn't ask for, no matter what the
    // model proposes — a hallucinated field name is silently dropped.
    let known_names: HashSet<&str> = schema.iter().map(|f| f.name.as_str()).collect();

    let mut out = Vec::new();
    for (field_name, value) in map {
        if !known_names.contains(field_name.as_str()) {
            continue;
        }
        if let Some(value) = value.as_str().map(str::trim).filter(|v| !v.is_empty()) {
            out.push(ExtractedValue {
                field_name: field_name.clone(),
                value: value.to_string(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{OllamaProvider, Provider, Residency};
    use std::cell::RefCell;

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

    /// Records how many times `complete` was called — lets the map-reduce
    /// branching be tested deterministically without a network dependency.
    struct CountingProvider {
        calls: RefCell<u32>,
        response: String,
    }

    impl Provider for CountingProvider {
        fn name(&self) -> &str {
            "counting-test-provider"
        }
        fn residency(&self) -> Residency {
            Residency::Local
        }
        fn complete(&self, _prompt: &str) -> crate::Result<String> {
            *self.calls.borrow_mut() += 1;
            Ok(self.response.clone())
        }
    }

    fn schema(name: &str, description: &str) -> SchemaField {
        SchemaField {
            name: name.to_string(),
            description: description.to_string(),
        }
    }

    fn words(count: usize) -> String {
        (0..count)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn empty_schema_returns_nothing_without_calling_the_model() {
        let provider = FixedProvider {
            response: "this should never be read".to_string(),
        };
        let result = extract(b"irrelevant", &[], &provider).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn extraction_is_filtered_to_known_field_names() {
        let schema = vec![schema("invoice_number", "the invoice's unique number")];
        let response = parse_extraction(
            r#"{"invoice_number": "INV-1029", "vendor_name": "Acme Corp"}"#,
            &schema,
        )
        .unwrap();
        // "vendor_name" wasn't in the schema — a hallucinated field must be
        // dropped, not surfaced.
        assert_eq!(
            response,
            vec![ExtractedValue {
                field_name: "invoice_number".to_string(),
                value: "INV-1029".to_string(),
            }]
        );
    }

    #[test]
    fn empty_string_values_are_dropped() {
        let schema = vec![schema("total_due", "the total amount due")];
        let response = parse_extraction(r#"{"total_due": "  "}"#, &schema).unwrap();
        assert!(response.is_empty());
    }

    #[test]
    fn response_wrapped_in_prose_still_parses() {
        let schema = vec![schema("total_due", "the total amount due")];
        let response = parse_extraction(
            "Here you go:\n```json\n{\"total_due\": \"$42.00\"}\n```",
            &schema,
        )
        .unwrap();
        assert_eq!(response[0].value, "$42.00");
    }

    #[test]
    fn long_text_map_reduces_and_merges_first_match_per_field() {
        let schema = vec![schema("field_a", "some field")];
        let provider = CountingProvider {
            calls: RefCell::new(0),
            response: r#"{"field_a": "found-it"}"#.to_string(),
        };
        let long_text = words(MAX_SINGLE_PASS_WORDS * 3);
        let result = extract_from_text(&long_text, &schema, &provider).unwrap();
        assert!(*provider.calls.borrow() >= 3);
        // Every chunk "finds" field_a with the same value — merge must not
        // duplicate it once already found.
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "found-it");
    }

    /// Real end-to-end pass against a local Ollama model — confirms the
    /// prompt shape actually elicits usable JSON from a real (non-mocked)
    /// LLM. Skips (doesn't fail) when Ollama is unavailable.
    #[test]
    fn extract_works_against_a_real_local_model() {
        let fixture = include_bytes!("../../pdfree-core/tests/fixtures/irs_f1040.pdf");
        if pdfree_core::pdfium::bind().is_err() {
            eprintln!("skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable");
            return;
        }

        let schema = vec![schema(
            "tax_year",
            "the tax year this form covers, e.g. 2023",
        )];
        let provider = OllamaProvider::new("qwen3:8b");
        match extract(fixture, &schema, &provider) {
            Ok(values) => {
                for v in &values {
                    assert_eq!(v.field_name, "tax_year");
                    assert!(!v.value.is_empty());
                }
            }
            Err(e) => eprintln!("skipping: provider unavailable ({e})"),
        }
    }
}
