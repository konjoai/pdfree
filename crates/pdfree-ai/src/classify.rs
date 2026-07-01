//! Document classification (Phase 6). Vectro embeddings.

use crate::{AiError, Result};

/// Classify a document (contract, invoice, tax form, receipt, ...).
pub fn classify(_pdf_bytes: &[u8]) -> Result<String> {
    Err(AiError::NotImplemented("classify::classify"))
}
