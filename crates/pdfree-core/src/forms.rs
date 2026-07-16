//! `AcroForm` field detection and filling, plus text overlays for non-interactive
//! PDFs (Phase 1).
//!
//! Reading and filling go through `PDFium`'s form-fill environment, which is
//! initialized automatically the moment a document is opened (see
//! [`pdfium_render`]'s `PdfForm`). Writing is honestly scoped to what the
//! underlying binding actually supports: `PDFium` exposes setters for text
//! fields and checkboxes, but not for selecting an option in a dropdown,
//! list box, or radio group — see [`FillValue`] and [`fill`] for the exact
//! contract.
//!
//! **Radio button selection was investigated in depth and confirmed
//! unreachable, not just unimplemented.** `pdfium-render` 0.8.37's
//! `PdfFormRadioButtonField::set_checked()` looks like exactly the setter
//! this needs, and does compile — but verified against a real, freshly
//! authored radio-group fixture (`tests/fixtures/radio_sample.pdf`), calling
//! it is a no-op. Its actual implementation copies the widget's *current*
//! `/AS` (appearance-state) value up to the group's shared `/V`; it doesn't
//! set `/AS` to this widget's own "on" export value first, and there is no
//! public way to do that from outside the crate either (the same
//! `pub(crate)`-only `PdfFormFieldPrivate` trait — and the same missing
//! public annotation-handle accessor — that blocks the `/DA` font-size fix
//! below blocks this too). For a widget whose `/AS` starts at `"Off"` (every
//! option, until a real interactive click cycle has run), `set_checked()`
//! therefore just writes `"Off"` back — it can only ever *confirm* a
//! selection PDFium's own interactive click-handling already made, not
//! *establish* one from a headless byte-in/byte-out call. `FormField` still
//! exposes `radio_group_index` (each widget's position within its group, a
//! genuine read-side improvement — useful for grouping/displaying a radio
//! group's options), but [`FillValue`] has no `Radio` variant: shipping one
//! that silently doesn't work would be worse than not having it. Revisit
//! only if a future `pdfium-render` release exposes a public `/AS` setter or
//! widget export-value getter.
//!
//! **Known gap: no font-size control on text field fill.** `fill()` cannot
//! bake in a deterministic "fit once" font size for a text field, and this is
//! a confirmed limitation of `pdfium-render` 0.8.37, not an oversight to fix
//! later with more code: setting a field's rendered font size means writing
//! its widget's `/DA` (default appearance) string, and the only bindings that
//! can touch an annotation's dictionary keys (`FPDFAnnot_SetStringValue_str`
//! and friends, via `PdfFormFieldPrivate`) live in a `pub(crate)` module the
//! crate deliberately does not expose — there is no annotation handle or
//! dictionary-key setter reachable from outside `pdfium-render` for a
//! [`PdfFormField`]. Filled text is therefore sized entirely by `PDFium`'s own
//! form-render behavior at export time, which is the likely source of the
//! "text resizes/gets cut off on export" symptom. Revisit if a future
//! `pdfium-render` release exposes a public setter; there is no lower-risk
//! workaround available today short of vendoring/forking the binding.

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
    /// 0-based page index this field's widget is on.
    pub page: u16,
    /// Horizontal position of the field's widget rect, from the page's left edge.
    pub x: f32,
    /// Vertical position of the field's widget rect, from the page's bottom edge.
    pub y: f32,
    /// Width of the field's widget rect.
    pub width: f32,
    /// Height of the field's widget rect.
    pub height: f32,
    /// Whether this field should route to the sign flow instead of a plain
    /// text input, and if so, whether it's a full signature or (lighter-
    /// weight) initials — see [`SignatureFieldKind`].
    pub signature_kind: SignatureFieldKind,
    /// This widget's position within its radio button group, if `kind` is
    /// [`FieldKind::RadioButton`] — `None` for every other kind. Pass this
    /// back in [`FillValue::Radio`] to select this specific option; see the
    /// module doc comment for why index (not the option's value string) is
    /// the only reliable public handle `pdfium-render` gives us.
    pub radio_group_index: Option<u32>,
}

/// Whether a field is a signature/initials field a shell should route to the
/// sign flow (Core UX Principles: "signature/initials fields are special-
/// cased" — never a plain text input) — and if so, which of the two.
///
/// The PDF spec has no distinct "initials" field type — `PDFium` only reports
/// [`FieldKind::Signature`] for a true digital-signature widget, and most
/// real-world non-crypto e-sign forms use an ordinary text field for both
/// "sign here" and "initial here" lines. So this is a name-based heuristic
/// layered on top of the real field kind, computed once here (rather than in
/// every shell) so macOS/web/Tauri/iOS classify a field identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureFieldKind {
    /// An ordinary field — fill it as text.
    None,
    /// A full signature.
    Signature,
    /// Initials — the shell should offer a lighter-weight signer UI.
    Initials,
}

impl SignatureFieldKind {
    /// Classify a field from its `AcroForm` kind and its name/tooltip.
    /// "Initials" is checked before the broader "sign" match since an
    /// initials field's name may not otherwise contain "sign" at all, while
    /// checking order the other way could never distinguish the two.
    #[must_use]
    pub fn classify(name: &str, kind: FieldKind) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("initial") {
            SignatureFieldKind::Initials
        } else if kind == FieldKind::Signature || lower.contains("sign") {
            SignatureFieldKind::Signature
        } else {
            SignatureFieldKind::None
        }
    }
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
    pub(crate) fn from_pdfium(kind: PdfFormFieldType) -> Self {
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
/// string, or a checkbox's checked state. Dropdowns, list boxes, and radio
/// button groups are readable via [`fields`] but not fillable through this
/// API — see the module doc comment for why radio selection specifically was
/// investigated and confirmed unreachable, not merely unimplemented.
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
    for (page_index, page) in document.pages().iter().enumerate() {
        for annotation in page.annotations().iter() {
            if let Some(field) = annotation.as_form_field() {
                let bounds = annotation.bounds().unwrap_or(PdfRect::ZERO);
                let name = field.name().unwrap_or_default();
                let kind = FieldKind::from_pdfium(field.field_type());
                let radio_group_index = match &field {
                    PdfFormField::RadioButton(f) => Some(f.index_in_group()),
                    _ => None,
                };
                out.push(FormField {
                    signature_kind: SignatureFieldKind::classify(&name, kind),
                    value: field_value(field),
                    name,
                    kind,
                    // Page counts are u16 throughout this crate; page_index is
                    // bounded by document.pages().len(), so this never truncates.
                    #[allow(clippy::cast_possible_truncation)]
                    page: page_index as u16,
                    x: bounds.left().value,
                    y: bounds.bottom().value,
                    width: bounds.width().value,
                    height: bounds.height().value,
                    radio_group_index,
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

#[cfg(test)]
mod tests {
    use super::{FieldKind, SignatureFieldKind};

    #[test]
    fn classifies_a_true_signature_field_kind_regardless_of_name() {
        assert_eq!(
            SignatureFieldKind::classify("topmostSubform[0].sig[0]", FieldKind::Signature),
            SignatureFieldKind::Signature
        );
        // Even a name that wouldn't otherwise match "sign"/"initial".
        assert_eq!(
            SignatureFieldKind::classify("widget_47", FieldKind::Signature),
            SignatureFieldKind::Signature
        );
    }

    #[test]
    fn classifies_a_text_field_named_like_a_signature_line() {
        for name in [
            "Your signature",
            "signature_1",
            "SIGN_HERE",
            "Please_Sign.Line",
        ] {
            assert_eq!(
                SignatureFieldKind::classify(name, FieldKind::Text),
                SignatureFieldKind::Signature,
                "expected {name} to classify as Signature"
            );
        }
    }

    #[test]
    fn classifies_a_text_field_named_like_initials_as_initials_not_signature() {
        for name in ["Initial here", "spouse_initials", "INITIALS_1"] {
            assert_eq!(
                SignatureFieldKind::classify(name, FieldKind::Text),
                SignatureFieldKind::Initials,
                "expected {name} to classify as Initials"
            );
        }
    }

    #[test]
    fn classifies_an_ordinary_field_as_none() {
        for name in ["FullName", "city", "zip_code", ""] {
            assert_eq!(
                SignatureFieldKind::classify(name, FieldKind::Text),
                SignatureFieldKind::None,
                "expected {name} to classify as None"
            );
        }
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(
            SignatureFieldKind::classify("SiGnAtUrE_LiNe", FieldKind::Text),
            SignatureFieldKind::Signature
        );
        assert_eq!(
            SignatureFieldKind::classify("InItIaLs", FieldKind::Text),
            SignatureFieldKind::Initials
        );
    }
}
