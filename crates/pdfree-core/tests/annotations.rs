//! Phase 2 acceptance tests: highlight/underline/strikeout/note annotations.
//!
//! Like `tests/render.rs`, these skip with a notice (rather than fail) when
//! `PDFium` isn't bundled, so a bare checkout still builds green. Run
//! `scripts/fetch-pdfium.sh` first to make them exercise `PDFium` for real.
//!
//! Test code may `unwrap`/`expect` freely (see `.github/copilot-instructions.md`)
//! — the production-code ban only applies to `pdfree-core`'s library surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::annotations::{self, Annotation, AnnotationKind, Color};
use pdfree_core::error::PdfError;
use pdfree_core::{Document, RenderOptions};

const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");

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

fn markup(kind: AnnotationKind, color: Option<Color>, note: Option<&str>) -> Annotation {
    Annotation {
        page: 0,
        kind,
        x: 72.0,
        y: 600.0,
        width: 200.0,
        height: 20.0,
        color,
        note: note.map(str::to_string),
    }
}

#[test]
fn adds_and_reads_back_all_four_annotation_kinds() {
    skip_without_pdfium!();

    assert!(
        annotations::list(SAMPLE)
            .expect("list on a bare PDF")
            .is_empty(),
        "sample.pdf has no annotations to start with"
    );

    let annotated = annotations::annotate(
        SAMPLE,
        &[
            markup(AnnotationKind::Highlight, None, Some("important")),
            markup(AnnotationKind::Underline, Some(Color::new(0, 120, 0)), None),
            markup(AnnotationKind::StrikeOut, None, None),
            Annotation {
                page: 0,
                kind: AnnotationKind::Note,
                x: 400.0,
                y: 700.0,
                width: 24.0,
                height: 24.0,
                color: None,
                note: Some("reviewer comment".to_string()),
            },
        ],
    )
    .expect("annotate");

    let found = annotations::list(&annotated).expect("list after annotate");
    assert_eq!(found.len(), 4);

    let highlight = found
        .iter()
        .find(|a| a.kind == AnnotationKind::Highlight)
        .expect("highlight present");
    assert_eq!(highlight.note.as_deref(), Some("important"));
    assert_eq!(
        highlight.color,
        Some(Color::new(255, 235, 59)),
        "default highlight color"
    );
    assert!((highlight.x - 72.0).abs() < 0.5);
    assert!((highlight.width - 200.0).abs() < 0.5);

    let underline = found
        .iter()
        .find(|a| a.kind == AnnotationKind::Underline)
        .expect("underline present");
    assert_eq!(
        underline.color,
        Some(Color::new(0, 120, 0)),
        "custom color persisted"
    );

    let strikeout = found
        .iter()
        .find(|a| a.kind == AnnotationKind::StrikeOut)
        .expect("strikeout present");
    assert_eq!(
        strikeout.color,
        Some(Color::new(220, 38, 38)),
        "default strikeout color"
    );

    let note = found
        .iter()
        .find(|a| a.kind == AnnotationKind::Note)
        .expect("note present");
    assert_eq!(note.note.as_deref(), Some("reviewer comment"));
}

#[test]
fn sticky_note_renders_visibly() {
    skip_without_pdfium!();

    // Unlike the three markup kinds (see the module docs' known PDFium
    // rendering gap), a Note annotation gets PDFium's built-in icon
    // appearance, so it must actually change the rendered pixels.
    let annotated = annotations::annotate(
        SAMPLE,
        &[Annotation {
            page: 0,
            kind: AnnotationKind::Note,
            x: 400.0,
            y: 700.0,
            width: 24.0,
            height: 24.0,
            color: None,
            note: Some("hello".to_string()),
        }],
    )
    .expect("annotate");

    let before = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let after = Document::from_bytes(annotated, None).unwrap();

    let png_before = before
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    let png_after = after
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    assert_ne!(png_before, png_after, "the sticky note icon must render");
}

#[test]
fn annotate_rejects_an_out_of_range_page() {
    skip_without_pdfium!();

    let mut bad = markup(AnnotationKind::Highlight, None, None);
    bad.page = 9;
    let err = annotations::annotate(SAMPLE, &[bad]).expect_err("page 9 does not exist");
    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn annotate_rejects_a_non_positive_size() {
    skip_without_pdfium!();

    let mut bad = markup(AnnotationKind::Highlight, None, None);
    bad.width = 0.0;
    let err = annotations::annotate(SAMPLE, &[bad]).expect_err("zero width is invalid");
    assert!(matches!(err, PdfError::InvalidAnnotation(_)), "got {err:?}");
}
