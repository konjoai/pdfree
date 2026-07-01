//! Structured extraction: fields, tables, contract analysis (Phase 6).
//!
//! Combines specialized extractors (pdfplumber-style) with LLM validation —
//! not LLM alone. Vectro provides structured output.

use crate::{AiError, Result};

/// Extract tables from a document as rows of cells.
pub fn extract_tables(_pdf_bytes: &[u8]) -> Result<Vec<Vec<Vec<String>>>> {
    Err(AiError::NotImplemented("extract::extract_tables"))
}
