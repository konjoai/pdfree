//! AcroForm field detection and filling (Phase 1).
//!
//! Fill interactive form fields (text, checkbox, dropdown) and overlay text
//! boxes onto non-interactive PDFs. Not implemented in Phase 0.

use crate::error::{PdfError, Result};

/// A form field discovered in a document.
#[derive(Debug, Clone)]
pub struct FormField {
    pub name: String,
    pub kind: FieldKind,
    pub value: Option<String>,
}

/// The kind of an AcroForm field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Checkbox,
    RadioButton,
    Dropdown,
    ListBox,
    Signature,
    Unknown,
}

/// Enumerate the interactive form fields in a document.
pub fn fields(_pdf_bytes: &[u8]) -> Result<Vec<FormField>> {
    Err(PdfError::NotImplemented("forms::fields"))
}
