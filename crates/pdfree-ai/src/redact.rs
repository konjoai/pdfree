//! PII detection and one-click redaction (Phase 6). Squash compliance DNA.

use crate::{AiError, Result};

/// A span of detected personally-identifiable information.
#[derive(Debug, Clone)]
pub struct PiiSpan {
    pub page: u16,
    pub kind: String,
    pub text: String,
}

/// Detect PII (names, SSNs, addresses, phone, banking) in a document.
pub fn detect_pii(_pdf_bytes: &[u8]) -> Result<Vec<PiiSpan>> {
    Err(AiError::NotImplemented("redact::detect_pii"))
}
