//! OCR with LLM cleanup (Phase 5).
//!
//! Tesseract (or Apple Vision on Apple platforms) reads scans; an LLM repairs
//! garbled characters and restores formatting.

use crate::{AiError, Result};

/// Extract text from a scanned page image.
pub fn recognize(_page_png: &[u8]) -> Result<String> {
    Err(AiError::NotImplemented("ocr::recognize"))
}
