//! Phase 2 acceptance tests: highlight/underline/strikeout/note annotations.
//!
//! Like `tests/render.rs`, these skip with a notice (rather than fail) when
//! `PDFium` isn't bundled, so a bare checkout still builds green. Run
//! `scripts/fetch-pdfium.sh` first to make them exercise `PDFium` for real.
//!
//! Test code may `unwrap`/`expect` freely (see `.github/copilot-instructions.md`)
//! — the production-code ban only applies to `pdfree-core`'s library surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::annotations::{self, Annotation, AnnotationKind, Color, Point};
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
        points: Vec::new(),
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
                points: Vec::new(),
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
            points: Vec::new(),
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

fn shape(kind: AnnotationKind, x: f32, y: f32, width: f32, height: f32) -> Annotation {
    Annotation {
        page: 0,
        kind,
        x,
        y,
        width,
        height,
        color: None,
        note: None,
        points: Vec::new(),
    }
}

fn line_like(kind: AnnotationKind, points: Vec<Point>) -> Annotation {
    Annotation {
        page: 0,
        kind,
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
        color: None,
        note: None,
        points,
    }
}

/// Unlike highlight/underline/strikeout, shapes and ink draw real vector
/// path objects (see the module doc comment) — so they must actually change
/// rendered pixels, not just round-trip through `list()`.
#[test]
fn rectangle_renders_visibly_and_lists_back_as_shape() {
    skip_without_pdfium!();

    let annotated = annotations::annotate(
        SAMPLE,
        &[shape(AnnotationKind::Rectangle, 72.0, 600.0, 100.0, 50.0)],
    )
    .expect("annotate rectangle");

    let before = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let after = Document::from_bytes(annotated.clone(), None).unwrap();
    let png_before = before
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    let png_after = after
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    assert_ne!(png_before, png_after, "the rectangle must render");

    let found = annotations::list(&annotated).expect("list after annotate");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, AnnotationKind::Shape);
}

#[test]
fn circle_renders_visibly() {
    skip_without_pdfium!();

    let annotated = annotations::annotate(
        SAMPLE,
        &[shape(AnnotationKind::Circle, 72.0, 600.0, 100.0, 100.0)],
    )
    .expect("annotate circle");

    let before = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let after = Document::from_bytes(annotated, None).unwrap();
    let png_before = before
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    let png_after = after
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    assert_ne!(png_before, png_after, "the circle must render");
}

#[test]
fn line_renders_visibly_and_lists_back_as_shape() {
    skip_without_pdfium!();

    let annotated = annotations::annotate(
        SAMPLE,
        &[line_like(
            AnnotationKind::Line,
            vec![Point::new(72.0, 600.0), Point::new(200.0, 650.0)],
        )],
    )
    .expect("annotate line");

    let before = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let after = Document::from_bytes(annotated.clone(), None).unwrap();
    let png_before = before
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    let png_after = after
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    assert_ne!(png_before, png_after, "the line must render");

    let found = annotations::list(&annotated).expect("list after annotate");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, AnnotationKind::Shape);
}

#[test]
fn arrow_renders_visibly_and_covers_a_larger_area_than_a_bare_line() {
    skip_without_pdfium!();

    let points = vec![Point::new(72.0, 600.0), Point::new(200.0, 650.0)];

    let line_bytes =
        annotations::annotate(SAMPLE, &[line_like(AnnotationKind::Line, points.clone())])
            .expect("annotate line");
    let arrow_bytes = annotations::annotate(SAMPLE, &[line_like(AnnotationKind::Arrow, points)])
        .expect("annotate arrow");

    let line_found = annotations::list(&line_bytes).expect("list line");
    let arrow_found = annotations::list(&arrow_bytes).expect("list arrow");

    // The arrowhead extends the bounding box beyond the bare line/line's,
    // confirming the triangle path object was actually added, not just the
    // shaft (a real geometric check, not merely "some bytes differ").
    let line_area = line_found[0].width * line_found[0].height;
    let arrow_area = arrow_found[0].width * arrow_found[0].height;
    assert!(
        arrow_area > line_area,
        "arrow bounding box ({arrow_area}) should exceed the bare line's ({line_area})"
    );

    let before = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let after = Document::from_bytes(arrow_bytes, None).unwrap();
    let png_before = before
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    let png_after = after
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    assert_ne!(png_before, png_after, "the arrow must render");
}

#[test]
fn ink_freehand_stroke_renders_visibly_and_lists_back_as_ink() {
    skip_without_pdfium!();

    let stroke = vec![
        Point::new(72.0, 600.0),
        Point::new(90.0, 620.0),
        Point::new(110.0, 590.0),
        Point::new(130.0, 610.0),
    ];
    let annotated = annotations::annotate(SAMPLE, &[line_like(AnnotationKind::Ink, stroke)])
        .expect("annotate ink");

    let before = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let after = Document::from_bytes(annotated.clone(), None).unwrap();
    let png_before = before
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    let png_after = after
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    assert_ne!(png_before, png_after, "the freehand stroke must render");

    let found = annotations::list(&annotated).expect("list after annotate");
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].kind,
        AnnotationKind::Ink,
        "ink is a real, distinct PdfPageAnnotationType — unlike the Stamp-backed shapes, \
         it round-trips through list() as itself"
    );
}

#[test]
fn line_rejects_a_point_count_other_than_two() {
    skip_without_pdfium!();

    let err = annotations::annotate(
        SAMPLE,
        &[line_like(AnnotationKind::Line, vec![Point::new(0.0, 0.0)])],
    )
    .expect_err("a line needs exactly 2 points");
    assert!(matches!(err, PdfError::InvalidAnnotation(_)), "got {err:?}");
}

#[test]
fn ink_rejects_fewer_than_two_points() {
    skip_without_pdfium!();

    let err = annotations::annotate(
        SAMPLE,
        &[line_like(AnnotationKind::Ink, vec![Point::new(0.0, 0.0)])],
    )
    .expect_err("ink needs at least 2 points");
    assert!(matches!(err, PdfError::InvalidAnnotation(_)), "got {err:?}");
}

#[test]
fn annotate_rejects_shape_as_a_direct_input() {
    skip_without_pdfium!();

    let err = annotations::annotate(
        SAMPLE,
        &[shape(AnnotationKind::Shape, 72.0, 600.0, 50.0, 50.0)],
    )
    .expect_err("Shape is a list()-only value, not a valid annotate() input");
    assert!(matches!(err, PdfError::InvalidAnnotation(_)), "got {err:?}");
}
