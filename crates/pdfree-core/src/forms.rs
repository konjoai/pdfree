//! `AcroForm` field detection and filling, plus text overlays for non-interactive
//! PDFs (Phase 1).
//!
//! Reading and filling go through `PDFium`'s form-fill environment, which is
//! initialized automatically the moment a document is opened (see
//! [`pdfium_render`]'s `PdfForm`). Writing is honestly scoped to what the
//! underlying binding actually supports: `PDFium` exposes setters for text
//! fields and checkboxes, but not for selecting an option in a dropdown or
//! list box — see [`FillValue`] and [`fill`] for the exact contract.

use std::collections::HashMap;

use pdfium_render::prelude::*;

use crate::error::{PdfError, Result};

/// A form field discovered in a document.
#[derive(Debug, Clone)]
pub struct FormField {
    /// The field's fully-qualified name, e.g. `"topmostSubform[0].Page1[0].f1_01[0]"`.
    pub name: String,
    /// What kind of widget this field is.
    pub kind: FieldKind,
    /// The field's current value as a display string, if any. `"true"`/`"false"`
    /// for checkboxes; the raw text for text fields; `None` for unset or
    /// unreadable fields.
    pub value: Option<String>,
}

/// The kind of an `AcroForm` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// A free-text entry field.
    Text,
    /// A single checkbox.
    Checkbox,
    /// One control in a radio button group.
    RadioButton,
    /// A combo box / dropdown selection field.
    Dropdown,
    /// A list box selection field.
    ListBox,
    /// A digital signature field.
    Signature,
    /// A push button (no persisted value).
    PushButton,
    /// A widget `PDFium` could not classify.
    Unknown,
}

impl FieldKind {
    fn from_pdfium(kind: PdfFormFieldType) -> Self {
        match kind {
            PdfFormFieldType::Text => FieldKind::Text,
            PdfFormFieldType::Checkbox => FieldKind::Checkbox,
            PdfFormFieldType::RadioButton => FieldKind::RadioButton,
            PdfFormFieldType::ComboBox => FieldKind::Dropdown,
            PdfFormFieldType::ListBox => FieldKind::ListBox,
            PdfFormFieldType::Signature => FieldKind::Signature,
            PdfFormFieldType::PushButton => FieldKind::PushButton,
            PdfFormFieldType::Unknown => FieldKind::Unknown,
        }
    }
}

/// A value to fill into a named interactive form field.
///
/// Scoped to what `PDFium`'s public binding can actually write: a text field's
/// string, or a checkbox's checked state. Dropdowns, list boxes, radio button
/// groups, and signature fields are readable via [`fields`] but not fillable
/// through this API yet — `pdfium-render` 0.8 exposes no public setter for
/// selecting an option, only for the two field kinds above.
#[derive(Debug, Clone)]
pub enum FillValue {
    /// Set a text field's value.
    Text(String),
    /// Check or clear a checkbox.
    Checkbox(bool),
}

/// Enumerate the interactive form fields in a document, with their current values.
///
/// # Errors
///
/// Returns an error if `PDFium` cannot be loaded or the bytes are not a
/// readable PDF (see [`PdfError`]).
pub fn fields(pdf_bytes: &[u8]) -> Result<Vec<FormField>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;

    let mut out = Vec::new();
    for page in document.pages().iter() {
        for annotation in page.annotations().iter() {
            if let Some(field) = annotation.as_form_field() {
                out.push(FormField {
                    name: field.name().unwrap_or_default(),
                    kind: FieldKind::from_pdfium(field.field_type()),
                    value: field_value(field),
                });
            }
        }
    }
    Ok(out)
}

/// Fill named interactive form fields, returning the updated PDF as new bytes.
///
/// Every name in `values` must match a field actually present in the
/// document and must pair with a [`FillValue`] that field kind accepts;
/// otherwise this returns an error rather than silently dropping the fill
/// request. Fields present in the document but not named in `values` are
/// left untouched.
///
/// # Errors
///
/// Returns [`PdfError::UnknownFormField`] if a name in `values` matches no
/// field in the document, [`PdfError::UnsupportedFieldFill`] if a field's
/// kind cannot accept the paired [`FillValue`], and propagates `PDFium` /
/// load errors otherwise.
pub fn fill(pdf_bytes: &[u8], values: &[(String, FillValue)]) -> Result<Vec<u8>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;

    let mut remaining: HashMap<&str, &FillValue> = values
        .iter()
        .map(|(name, value)| (name.as_str(), value))
        .collect();

    for page in document.pages().iter() {
        for mut annotation in page.annotations().iter() {
            let Some(field) = annotation.as_form_field_mut() else {
                continue;
            };
            let Some(name) = field.name() else {
                continue;
            };
            let Some(value) = remaining.remove(name.as_str()) else {
                continue;
            };
            apply_fill(field, &name, value)?;
        }
    }

    if let Some(name) = remaining.into_keys().next() {
        return Err(PdfError::UnknownFormField(name.to_string()));
    }

    Ok(document.save_to_bytes()?)
}

/// Read the current value of a form field as a display string.
fn field_value(field: &PdfFormField) -> Option<String> {
    match field {
        PdfFormField::Text(f) => f.value(),
        PdfFormField::Checkbox(f) => Some(bool_str(f.is_checked().unwrap_or(false)).to_string()),
        PdfFormField::RadioButton(f) => {
            if f.is_checked().unwrap_or(false) {
                f.group_value()
            } else {
                None
            }
        }
        PdfFormField::ComboBox(f) => f.value(),
        PdfFormField::ListBox(f) => f.value(),
        PdfFormField::PushButton(_) | PdfFormField::Signature(_) | PdfFormField::Unknown(_) => None,
    }
}

fn bool_str(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

/// Apply one fill value to one field, or fail honestly if this field kind
/// can't accept it.
fn apply_fill(field: &mut PdfFormField, name: &str, value: &FillValue) -> Result<()> {
    match (field, value) {
        (PdfFormField::Text(f), FillValue::Text(text)) => {
            f.set_value(text)?;
            Ok(())
        }
        (PdfFormField::Checkbox(f), FillValue::Checkbox(checked)) => {
            f.set_checked(*checked)?;
            Ok(())
        }
        (field, _) => Err(PdfError::UnsupportedFieldFill {
            name: name.to_string(),
            kind: FieldKind::from_pdfium(field.field_type()),
        }),
    }
}

/// A block of text to stamp onto a page at a fixed position, in PDF points
/// (72 per inch, origin at the page's bottom-left corner).
///
/// This is the "overlay text boxes on non-interactive PDFs" path: for a PDF
/// with no `AcroForm` at all (a plain scanned form, a template with no fillable
/// fields), stamp the answer directly onto the page instead.
#[derive(Debug, Clone)]
pub struct TextOverlay {
    /// 0-based page index to stamp onto.
    pub page: u16,
    /// Horizontal position in PDF points from the page's left edge.
    pub x: f32,
    /// Vertical position in PDF points from the page's bottom edge.
    pub y: f32,
    /// The text to stamp.
    pub text: String,
    /// Font size in PDF points. Must be a positive, finite number.
    pub font_size: f32,
}

/// Stamp one or more [`TextOverlay`]s onto a document, returning the updated
/// PDF as new bytes.
///
/// # Errors
///
/// Returns [`PdfError::PageOutOfRange`] if an overlay names a page that
/// doesn't exist, [`PdfError::InvalidOverlay`] if `font_size` is not a
/// positive finite number, and propagates `PDFium` / load errors otherwise.
pub fn overlay_text(pdf_bytes: &[u8], overlays: &[TextOverlay]) -> Result<Vec<u8>> {
    let pdfium = crate::pdfium::bind()?;
    let mut document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    let font = document.fonts_mut().helvetica();
    let count = document.pages().len();

    for overlay in overlays {
        if overlay.page >= count {
            return Err(PdfError::PageOutOfRange {
                index: overlay.page,
                count,
            });
        }
        if !(overlay.font_size.is_finite() && overlay.font_size > 0.0) {
            return Err(PdfError::InvalidOverlay(format!(
                "font_size must be a positive, finite number (got {})",
                overlay.font_size
            )));
        }

        let mut page = document.pages().get(overlay.page)?;
        page.objects_mut().create_text_object(
            PdfPoints::new(overlay.x),
            PdfPoints::new(overlay.y),
            overlay.text.as_str(),
            font,
            PdfPoints::new(overlay.font_size),
        )?;
    }

    Ok(document.save_to_bytes()?)
}
