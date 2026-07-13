//! Label-aware fillable-field detection acceptance tests.
//!
//! Like the other `PDFium`-backed test files, these skip with a notice
//! (rather than fail) when the library isn't bundled, so a bare checkout
//! still builds green. Run `scripts/fetch-pdfium.sh` first to exercise them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfium_render::prelude::*;

use pdfree_core::error::PdfError;
use pdfree_core::fields::{fillable_fields, FieldSource};

fn pdfium_available() -> bool {
    pdfree_core::pdfium::bind().is_ok()
}

macro_rules! skip_without_pdfium {
    () => {
        if !pdfium_available() {
            eprintln!("skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable");
            return;
        }
    };
}

/// A one-page PDF with a stamped label and a stroked box just to its right —
/// the `Label: [ box ]` shape a flat (non-AcroForm) form draws.
fn pdf_with_labeled_box(label: &str, label_x: f32, box_left: f32, y: f32) -> Vec<u8> {
    let pdfium = pdfree_core::pdfium::bind().expect("bind pdfium");
    let mut document = pdfium.create_new_pdf().expect("create pdf");
    let mut page = document
        .pages_mut()
        .create_page_at_start(PdfPagePaperSize::Custom(
            PdfPoints::new(612.0),
            PdfPoints::new(792.0),
        ))
        .expect("create page");
    let font = document.fonts_mut().helvetica();
    page.objects_mut()
        .create_text_object(
            PdfPoints::new(label_x),
            PdfPoints::new(y),
            label,
            font,
            PdfPoints::new(10.0),
        )
        .expect("label text");
    // A box to the right of the label, on the same line.
    let rect = PdfRect::new(
        PdfPoints::new(y - 2.0),
        PdfPoints::new(box_left),
        PdfPoints::new(y + 14.0),
        PdfPoints::new(box_left + 160.0),
    );
    page.objects_mut()
        .create_path_object_rect(rect, Some(PdfColor::BLACK), Some(PdfPoints::new(1.0)), None)
        .expect("box");
    document.save_to_bytes().expect("save")
}

/// A one-page PDF with a stroked box and NO text anywhere near it — a
/// decorative/layout rectangle that must not be reported as a fillable field.
fn pdf_with_unlabeled_box() -> Vec<u8> {
    let pdfium = pdfree_core::pdfium::bind().expect("bind pdfium");
    let mut document = pdfium.create_new_pdf().expect("create pdf");
    let mut page = document
        .pages_mut()
        .create_page_at_start(PdfPagePaperSize::Custom(
            PdfPoints::new(612.0),
            PdfPoints::new(792.0),
        ))
        .expect("create page");
    let rect = PdfRect::new(
        PdfPoints::new(200.0),
        PdfPoints::new(100.0),
        PdfPoints::new(400.0),
        PdfPoints::new(300.0),
    );
    page.objects_mut()
        .create_path_object_rect(rect, Some(PdfColor::BLACK), Some(PdfPoints::new(1.0)), None)
        .expect("box");
    document.save_to_bytes().expect("save")
}

#[test]
fn keeps_a_labeled_box_and_carries_its_label() {
    skip_without_pdfium!();

    // Label "Full name:" ending near x=145, box starting at x=150.
    let bytes = pdf_with_labeled_box("Full name:", 100.0, 150.0, 300.0);
    let fields = fillable_fields(&bytes, 0).expect("fillable_fields");

    assert_eq!(
        fields.len(),
        1,
        "expected exactly the labeled box: {fields:?}"
    );
    let field = &fields[0];
    assert_eq!(field.source, FieldSource::Detected);
    assert_eq!(field.label.as_deref(), Some("Full name"));
}

#[test]
fn drops_a_box_with_no_label() {
    skip_without_pdfium!();

    let bytes = pdf_with_unlabeled_box();
    let fields = fillable_fields(&bytes, 0).expect("fillable_fields");

    assert!(
        fields.is_empty(),
        "an unlabeled decorative box must not be a field: {fields:?}"
    );
}

#[test]
fn routes_a_signature_labeled_line_to_the_sign_flow() {
    skip_without_pdfium!();

    let bytes = pdf_with_labeled_box("Signature:", 90.0, 150.0, 250.0);
    let fields = fillable_fields(&bytes, 0).expect("fillable_fields");

    assert_eq!(fields.len(), 1, "{fields:?}");
    assert_eq!(
        fields[0].signature_kind,
        pdfree_core::forms::SignatureFieldKind::Signature
    );
}

#[test]
fn irs_1040_reports_real_fields_within_the_page() {
    skip_without_pdfium!();

    // A real, multi-field government AcroForm: every reported field must be a
    // plausible on-page rect, and there should be a healthy number of them
    // (the AcroForm widgets alone guarantee this even before any detected
    // box). Not asserting an exact count — real forms are ragged.
    let bytes = include_bytes!("fixtures/irs_f1040.pdf");
    let fields = fillable_fields(bytes, 0).expect("fillable_fields");
    assert!(
        fields.len() >= 5,
        "far too few fields detected: {}",
        fields.len()
    );
    for f in &fields {
        assert!(f.width > 0.0 && f.height > 0.0, "degenerate field: {f:?}");
        assert!(
            f.width < 620.0 && f.height < 800.0,
            "field larger than page: {f:?}"
        );
    }
}

#[test]
fn rejects_an_out_of_range_page() {
    skip_without_pdfium!();

    let bytes = pdf_with_unlabeled_box();
    let err = fillable_fields(&bytes, 9).unwrap_err();
    assert!(matches!(
        err,
        PdfError::PageOutOfRange { index: 9, count: 1 }
    ));
}
