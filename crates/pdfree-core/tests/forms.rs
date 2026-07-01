//! Phase 1 acceptance tests: detect and fill `AcroForm` fields, and overlay
//! text onto a non-interactive PDF.
//!
//! Like `tests/render.rs`, these skip with a notice (rather than fail) when
//! `PDFium` isn't bundled, so a bare checkout still builds green. Run
//! `scripts/fetch-pdfium.sh` first to make them exercise `PDFium` for real.

use pdfree_core::error::PdfError;
use pdfree_core::forms::{self, FieldKind, FillValue, TextOverlay};
use pdfree_core::{Document, RenderOptions};

const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");
const FORM_SAMPLE: &[u8] = include_bytes!("fixtures/form_sample.pdf");
/// The real, unmodified IRS Form 1040 (fetched from irs.gov), per the project
/// convention of testing against real-world PDFs, not just synthetic fixtures.
const IRS_F1040: &[u8] = include_bytes!("fixtures/irs_f1040.pdf");

fn pdfium_available() -> bool {
    pdfree_core::pdfium::bind().is_ok()
}

macro_rules! skip_without_pdfium {
    () => {
        if !pdfium_available() {
            eprintln!(
                "skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable"
            );
            return;
        }
    };
}

#[test]
fn discovers_form_fields_with_kinds_and_initial_values() {
    skip_without_pdfium!();

    let found = forms::fields(FORM_SAMPLE).expect("enumerate fields");
    assert_eq!(found.len(), 2, "fixture has a text field and a checkbox");

    let name_field = found
        .iter()
        .find(|f| f.name == "FullName")
        .expect("FullName field present");
    assert_eq!(name_field.kind, FieldKind::Text);

    let checkbox_field = found
        .iter()
        .find(|f| f.name == "AgreeToTerms")
        .expect("AgreeToTerms field present");
    assert_eq!(checkbox_field.kind, FieldKind::Checkbox);
    assert_eq!(checkbox_field.value.as_deref(), Some("false"));
}

#[test]
fn fills_text_and_checkbox_fields_and_persists_the_values() {
    skip_without_pdfium!();

    let filled = forms::fill(
        FORM_SAMPLE,
        &[
            (
                "FullName".to_string(),
                FillValue::Text("Wesley Scholl".to_string()),
            ),
            ("AgreeToTerms".to_string(), FillValue::Checkbox(true)),
        ],
    )
    .expect("fill fields");

    let after = forms::fields(&filled).expect("re-read filled fields");

    let name = after.iter().find(|f| f.name == "FullName").unwrap();
    assert_eq!(name.value.as_deref(), Some("Wesley Scholl"));

    let checkbox = after.iter().find(|f| f.name == "AgreeToTerms").unwrap();
    assert_eq!(checkbox.value.as_deref(), Some("true"));
}

#[test]
fn fill_rejects_an_unknown_field_name() {
    skip_without_pdfium!();

    let err = forms::fill(
        FORM_SAMPLE,
        &[("DoesNotExist".to_string(), FillValue::Text("x".to_string()))],
    )
    .expect_err("unknown field name must error, not silently no-op");

    assert!(
        matches!(&err, PdfError::UnknownFormField(name) if name == "DoesNotExist"),
        "got {err:?}"
    );
}

#[test]
fn fill_rejects_a_value_kind_the_field_cannot_accept() {
    skip_without_pdfium!();

    // AgreeToTerms is a checkbox; filling it with a text value must fail
    // honestly rather than silently coercing or dropping the request.
    let err = forms::fill(
        FORM_SAMPLE,
        &[(
            "AgreeToTerms".to_string(),
            FillValue::Text("yes".to_string()),
        )],
    )
    .expect_err("wrong value kind for this field must error");

    assert!(
        matches!(
            &err,
            PdfError::UnsupportedFieldFill { name, kind }
                if name == "AgreeToTerms" && *kind == FieldKind::Checkbox
        ),
        "got {err:?}"
    );
}

#[test]
fn overlays_text_onto_a_non_interactive_pdf() {
    skip_without_pdfium!();

    // sample.pdf (from tests/render.rs) has no AcroForm at all.
    assert!(forms::fields(SAMPLE).expect("enumerate fields").is_empty());

    let overlaid = forms::overlay_text(
        SAMPLE,
        &[TextOverlay {
            page: 0,
            x: 72.0,
            y: 600.0,
            text: "Overlay: Hello PDFree".to_string(),
            font_size: 14.0,
        }],
    )
    .expect("overlay text");

    assert!(overlaid.len() > SAMPLE.len(), "overlay adds content");

    // The overlay must actually render: decode the resulting page and check
    // pixels changed near the stamped position (not just that bytes grew).
    let doc = Document::from_bytes(overlaid, None).expect("open overlaid doc");
    let before = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();

    let png_before = before
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    let png_after = doc.render_page(0, &RenderOptions::with_dpi(150.0)).unwrap();
    assert_ne!(
        png_before, png_after,
        "overlay must change the rendered page"
    );
}

#[test]
fn overlay_rejects_an_out_of_range_page() {
    skip_without_pdfium!();

    let err = forms::overlay_text(
        SAMPLE,
        &[TextOverlay {
            page: 9,
            x: 0.0,
            y: 0.0,
            text: "x".to_string(),
            font_size: 12.0,
        }],
    )
    .expect_err("page 9 does not exist");

    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn fills_a_real_irs_form_1040_and_field_values_persist() {
    skip_without_pdfium!();

    // The 1040 is a real-world AcroForm: 199 fields with generated names
    // (f1_01[0], c1_1[0], …) inside repeating subform containers, unlike the
    // hand-built fixture above. First and last name plus a checkbox is
    // representative of the "smart form fill" use case from the project spec.
    const FIRST_NAME: &str = "topmostSubform[0].Page1[0].f1_01[0]";
    const LAST_NAME: &str = "topmostSubform[0].Page1[0].f1_02[0]";
    const DIGITAL_ASSETS_YES: &str = "topmostSubform[0].Page1[0].c1_1[0]";

    let found = forms::fields(IRS_F1040).expect("enumerate real-world fields");
    assert!(
        found.len() > 100,
        "expected the 1040's full field set, got {}",
        found.len()
    );
    assert!(found
        .iter()
        .any(|f| f.name == FIRST_NAME && f.kind == FieldKind::Text));
    assert!(found
        .iter()
        .any(|f| f.name == DIGITAL_ASSETS_YES && f.kind == FieldKind::Checkbox));

    let filled = forms::fill(
        IRS_F1040,
        &[
            (
                FIRST_NAME.to_string(),
                FillValue::Text("Wesley".to_string()),
            ),
            (LAST_NAME.to_string(), FillValue::Text("Scholl".to_string())),
            (DIGITAL_ASSETS_YES.to_string(), FillValue::Checkbox(true)),
        ],
    )
    .expect("fill real-world fields");

    let after = forms::fields(&filled).expect("re-read filled real-world fields");
    assert_eq!(
        after
            .iter()
            .find(|f| f.name == FIRST_NAME)
            .unwrap()
            .value
            .as_deref(),
        Some("Wesley")
    );
    assert_eq!(
        after
            .iter()
            .find(|f| f.name == LAST_NAME)
            .unwrap()
            .value
            .as_deref(),
        Some("Scholl")
    );
    assert_eq!(
        after
            .iter()
            .find(|f| f.name == DIGITAL_ASSETS_YES)
            .unwrap()
            .value
            .as_deref(),
        Some("true")
    );

    // The document must still open and render after being mutated.
    let doc = Document::from_bytes(filled, None).expect("open filled real-world doc");
    doc.render_page(0, &RenderOptions::with_dpi(72.0))
        .expect("render filled real-world doc");
}

#[test]
fn overlay_rejects_a_non_positive_font_size() {
    skip_without_pdfium!();

    let err = forms::overlay_text(
        SAMPLE,
        &[TextOverlay {
            page: 0,
            x: 0.0,
            y: 0.0,
            text: "x".to_string(),
            font_size: 0.0,
        }],
    )
    .expect_err("zero font_size is invalid");

    assert!(matches!(err, PdfError::InvalidOverlay(_)), "got {err:?}");
}
