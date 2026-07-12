//! Label-aware fillable-field detection (Core UX Principle #2, refined).
//!
//! The original scan (see [`crate::boxes`]) reconstructs every rectangle and
//! ruled-line cell on a page from vector graphics alone. On real forms that
//! over-detects: decorative rules, table borders, and layout boxes that a
//! human would never try to type into all come back as "fields", while a
//! plain `AcroForm` text field with no drawn box around it is missed entirely
//! because the geometric scan never looks at the form dictionary.
//!
//! This module produces the list a shell should actually highlight, in a
//! single document parse, by combining two signals:
//!
//! 1. **`AcroForm` widgets** — genuine, author-declared interactive fields.
//!    These are always included (they are fields by definition, and dropping
//!    them is exactly the "missed a fillable field" bug); each is paired with
//!    a nearby on-page label for display where one can be found.
//! 2. **Detected boxes/lines** — the geometric scan, but kept *only* when a
//!    human-readable text label sits next to (or above) the box, and only
//!    when it doesn't duplicate an `AcroForm` widget already found. A drawn
//!    box with no label next to it is not treated as a field at all — that's
//!    the "highlights things that aren't fields" bug. The label requirement
//!    is what makes the detector intelligent about flat/scanned forms that
//!    have no `AcroForm` at all.
//!
//! A detected box whose *label* reads like a signature/initials line ("Sign
//! here", "Initials", …) is classified into the sign flow the same way an
//! `AcroForm` signature field is, so a flat form's signature line routes to
//! signing rather than a text caret (Core UX Principle #3).

use pdfium_render::prelude::*;

use crate::boxes::{self, DetectedBox};
use crate::error::{PdfError, Result};
use crate::forms::{FieldKind, SignatureFieldKind};

/// Where a [`FillableField`] came from — a real interactive widget, or a box
/// reconstructed from the page's vector graphics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldSource {
    /// A genuine, author-declared `AcroForm` widget.
    AcroForm,
    /// A box/line reconstructed from vector graphics, kept because it has a
    /// nearby text label.
    Detected,
}

/// One field a shell should present an input affordance for, in PDF points
/// (72/inch, origin at the page's bottom-left corner).
#[derive(Debug, Clone)]
pub struct FillableField {
    /// 0-based page index this field is on.
    pub page: u16,
    /// Horizontal position from the page's left edge.
    pub x: f32,
    /// Vertical position from the page's bottom edge.
    pub y: f32,
    /// Field width.
    pub width: f32,
    /// Field height.
    pub height: f32,
    /// The human-readable label found next to (or above) the field, if any.
    /// Always present for a [`FieldSource::Detected`] field (it's the reason
    /// the field was kept); best-effort for an `AcroForm` widget.
    pub label: Option<String>,
    /// The `AcroForm` field's fully-qualified name, when this came from a
    /// real widget; `None` for a detected box.
    pub field_name: Option<String>,
    /// Whether this field routes to the sign flow, and if so which weight.
    pub signature_kind: SignatureFieldKind,
    /// Where this field came from.
    pub source: FieldSource,
}

/// The largest gap, in PDF points, allowed between a label's right edge and a
/// field's left edge for the label to count as that field's left-hand label
/// (the `Name: ______` pattern). Roughly one inch — generous enough for
/// right-aligned label columns, tight enough that an unrelated word further
/// left isn't grabbed (vertical alignment is also required).
const MAX_LEFT_GAP: f32 = 60.0;

/// The largest gap, in PDF points, allowed between a label's bottom edge and
/// a field's top edge for the label to count as a column-header label sitting
/// above the field.
const MAX_ABOVE_GAP: f32 = 22.0;

/// Slack, in PDF points, absorbing sub-point jitter when testing whether a
/// label and field overlap on the cross axis.
const ALIGN_PAD: f32 = 3.0;

/// Detect every fillable field on `page` a shell should highlight, in PDF
/// points, in a single document parse.
///
/// See the module docs for the exact rule. In short: every `AcroForm` widget
/// on the page, plus every detected box/line that has a text label next to it
/// and doesn't duplicate one of those widgets.
///
/// # Errors
///
/// Returns [`PdfError::PageOutOfRange`] if `page` doesn't exist, and
/// propagates `PDFium` / load errors otherwise.
pub fn fillable_fields(pdf_bytes: &[u8], page: u16) -> Result<Vec<FillableField>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    let count = document.pages().len();
    if page >= count {
        return Err(PdfError::PageOutOfRange { index: page, count });
    }

    let loaded = document.pages().get(page)?;
    let runs = label_runs(&loaded);

    let mut fields: Vec<FillableField> = Vec::new();

    // 1. AcroForm widgets — always kept (they're declared fields), with a
    //    best-effort label attached for display.
    for annotation in loaded.annotations().iter() {
        let Some(field) = annotation.as_form_field() else {
            continue;
        };
        let kind = FieldKind::from_pdfium(field.field_type());
        if !kind_is_fillable(kind) {
            continue;
        }
        let bounds = annotation.bounds().unwrap_or(PdfRect::ZERO);
        let (x, y, w, h) = (
            bounds.left().value,
            bounds.bottom().value,
            bounds.width().value,
            bounds.height().value,
        );
        if w <= 0.0 || h <= 0.0 {
            continue;
        }
        let name = field.name().unwrap_or_default();
        let label = best_label(x, y, w, h, &runs);
        let signature_kind = classify_signature(kind, &name, label.as_deref());
        fields.push(FillableField {
            page,
            x,
            y,
            width: w,
            height: h,
            label: label.or_else(|| non_empty(&name)),
            field_name: Some(name),
            signature_kind,
            source: FieldSource::AcroForm,
        });
    }

    let acroform_count = fields.len();

    // 2. Detected boxes — kept only when labeled and not duplicating a widget.
    for b in boxes::detect_boxes(&loaded, page) {
        if fields[..acroform_count]
            .iter()
            .any(|f| covers(f.x, f.y, f.width, f.height, &b))
        {
            continue;
        }
        let Some(label) = best_label(b.x, b.y, b.width, b.height, &runs) else {
            continue;
        };
        let signature_kind = SignatureFieldKind::classify(&label, FieldKind::Text);
        fields.push(FillableField {
            page,
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
            label: Some(label),
            field_name: None,
            signature_kind,
            source: FieldSource::Detected,
        });
    }

    Ok(fields)
}

/// Which `AcroForm` field kinds are worth presenting an input affordance for.
/// Push buttons carry no fillable value and `Unknown` widgets can't be routed
/// anywhere sensible, so both are excluded.
fn kind_is_fillable(kind: FieldKind) -> bool {
    matches!(
        kind,
        FieldKind::Text
            | FieldKind::Checkbox
            | FieldKind::RadioButton
            | FieldKind::Dropdown
            | FieldKind::ListBox
            | FieldKind::Signature
    )
}

/// Classify a widget's sign-flow routing from its declared kind and name, and
/// — if neither says "signature" — from its on-page label, so a cryptically
/// named text field (`f1_07`) sitting under a "Signature" label still routes
/// to signing.
fn classify_signature(kind: FieldKind, name: &str, label: Option<&str>) -> SignatureFieldKind {
    let by_name = SignatureFieldKind::classify(name, kind);
    if by_name != SignatureFieldKind::None {
        return by_name;
    }
    match label {
        Some(label) => SignatureFieldKind::classify(label, FieldKind::Text),
        None => SignatureFieldKind::None,
    }
}

/// A text run reduced to just what label-matching needs. Kept separate from
/// [`crate::editor::TextRun`] so the matching logic below is a pure function
/// over plain geometry, unit-testable without `PDFium`.
#[derive(Debug, Clone)]
struct LabelRun {
    text: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// Collect every non-blank text run on a loaded page as a label candidate.
fn label_runs(loaded: &PdfPage<'_>) -> Vec<LabelRun> {
    let mut runs = Vec::new();
    for object in loaded.objects().iter() {
        let Some(text_object) = object.as_text_object() else {
            continue;
        };
        let text = text_object.text();
        if text.trim().is_empty() {
            continue;
        }
        let bounds = text_object
            .bounds()
            .map(|q| q.to_rect())
            .unwrap_or(PdfRect::ZERO);
        runs.push(LabelRun {
            text,
            x: bounds.left().value,
            y: bounds.bottom().value,
            width: bounds.width().value,
            height: bounds.height().value,
        });
    }
    runs
}

/// Find the closest qualifying label for a field box, if any: a text run
/// immediately to the field's left on the same line (`Name: ____`), or one
/// sitting just above it (a column header). Returns the label text, trimmed
/// of a trailing colon. A run whose center falls *inside* the box is treated
/// as the field's own content, never a label.
fn best_label(bx: f32, by: f32, bw: f32, bh: f32, runs: &[LabelRun]) -> Option<String> {
    let mut best: Option<(f32, &str)> = None;
    for run in runs {
        if !has_alnum(&run.text) {
            continue;
        }
        let (rx, ry, rw, rh) = (run.x, run.y, run.width, run.height);
        // A run centered inside the box is filled content, not a label.
        let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
        if cx > bx + ALIGN_PAD
            && cx < bx + bw - ALIGN_PAD
            && cy > by + ALIGN_PAD
            && cy < by + bh - ALIGN_PAD
        {
            continue;
        }

        let mut distance: Option<f32> = None;

        // Left label: run ends at/just-before the box's left edge and shares
        // its vertical band.
        let run_right = rx + rw;
        let left_gap = bx - run_right;
        if (-ALIGN_PAD..=MAX_LEFT_GAP).contains(&left_gap)
            && bands_overlap(by, by + bh, ry, ry + rh)
        {
            distance = Some(min_opt(distance, left_gap.max(0.0)));
        }

        // Above label: run sits just above the box's top edge and overlaps it
        // horizontally.
        let box_top = by + bh;
        let above_gap = ry - box_top;
        if (-ALIGN_PAD..=MAX_ABOVE_GAP).contains(&above_gap)
            && bands_overlap(bx, bx + bw, rx, rx + rw)
        {
            distance = Some(min_opt(distance, above_gap.max(0.0)));
        }

        if let Some(distance) = distance {
            if best.map_or(true, |(d, _)| distance < d) {
                best = Some((distance, run.text.trim()));
            }
        }
    }
    best.map(|(_, text)| clean_label(text))
}

fn min_opt(current: Option<f32>, candidate: f32) -> f32 {
    match current {
        Some(c) => c.min(candidate),
        None => candidate,
    }
}

/// Whether two 1-D bands `[a0, a1]` and `[b0, b1]` overlap within [`ALIGN_PAD`].
fn bands_overlap(a0: f32, a1: f32, b0: f32, b1: f32) -> bool {
    a0.min(a1) - ALIGN_PAD <= b0.max(b1) && b0.min(b1) - ALIGN_PAD <= a1.max(a0)
}

/// Whether an `AcroForm` field rect substantially covers a detected box (so
/// the box is a duplicate of the real widget and should be dropped). True
/// when the box's center is inside the field rect, or the two overlap over
/// more than half the box's area.
fn covers(fx: f32, fy: f32, fw: f32, fh: f32, b: &DetectedBox) -> bool {
    let (bcx, bcy) = (b.x + b.width / 2.0, b.y + b.height / 2.0);
    let center_inside = bcx >= fx - ALIGN_PAD
        && bcx <= fx + fw + ALIGN_PAD
        && bcy >= fy - ALIGN_PAD
        && bcy <= fy + fh + ALIGN_PAD;
    if center_inside {
        return true;
    }
    let x_overlap = (fx + fw).min(b.x + b.width) - fx.max(b.x);
    let y_overlap = (fy + fh).min(b.y + b.height) - fy.max(b.y);
    if x_overlap <= 0.0 || y_overlap <= 0.0 {
        return false;
    }
    let box_area = (b.width * b.height).max(1.0);
    (x_overlap * y_overlap) / box_area > 0.5
}

fn has_alnum(text: &str) -> bool {
    text.chars().any(|c| c.is_alphanumeric())
}

fn non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Tidy a raw label for display: trim, then drop a single trailing colon and
/// any space before it (`"First name :"` -> `"First name"`).
fn clean_label(text: &str) -> String {
    let trimmed = text.trim();
    trimmed
        .strip_suffix(':')
        .map_or(trimmed, str::trim_end)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, x: f32, y: f32, w: f32, h: f32) -> LabelRun {
        LabelRun {
            text: text.to_string(),
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn finds_a_label_immediately_to_the_left() {
        // "Name:" run ending at x=98, box starts at x=102 on the same line.
        let runs = vec![run("Name:", 60.0, 300.0, 38.0, 10.0)];
        let label = best_label(102.0, 299.0, 200.0, 14.0, &runs);
        assert_eq!(label.as_deref(), Some("Name"));
    }

    #[test]
    fn finds_a_column_header_label_above_the_box() {
        // Header sitting 6pt above a cell, horizontally overlapping it.
        let runs = vec![run("Amount", 200.0, 512.0, 40.0, 9.0)];
        let label = best_label(200.0, 480.0, 90.0, 24.0, &runs);
        assert_eq!(label.as_deref(), Some("Amount"));
    }

    #[test]
    fn rejects_a_box_with_no_nearby_text() {
        let runs = vec![run("Far away", 10.0, 50.0, 40.0, 10.0)];
        assert!(best_label(300.0, 400.0, 100.0, 16.0, &runs).is_none());
    }

    #[test]
    fn rejects_a_left_run_that_is_too_far_away() {
        // Run ends at x=20; box starts at x=300 — well past MAX_LEFT_GAP.
        let runs = vec![run("Section", 0.0, 300.0, 20.0, 10.0)];
        assert!(best_label(300.0, 299.0, 100.0, 14.0, &runs).is_none());
    }

    #[test]
    fn rejects_a_left_run_on_a_different_line() {
        // Correct horizontal gap, but the run's band is far below the box.
        let runs = vec![run("Name:", 60.0, 100.0, 38.0, 10.0)];
        assert!(best_label(102.0, 300.0, 200.0, 14.0, &runs).is_none());
    }

    #[test]
    fn ignores_a_run_that_is_the_boxs_own_content() {
        // A run centered well inside the box is filled text, not a label.
        let runs = vec![run("already filled", 120.0, 302.0, 80.0, 10.0)];
        assert!(best_label(100.0, 300.0, 200.0, 16.0, &runs).is_none());
    }

    #[test]
    fn requires_alphanumeric_content_in_the_label() {
        // Punctuation-only run to the left should not qualify.
        let runs = vec![run("----", 60.0, 300.0, 38.0, 10.0)];
        assert!(best_label(102.0, 299.0, 200.0, 14.0, &runs).is_none());
    }

    #[test]
    fn prefers_the_closer_of_two_candidate_labels() {
        let runs = vec![
            run("Far:", 20.0, 300.0, 30.0, 10.0),  // gap 52
            run("Near:", 90.0, 300.0, 30.0, 10.0), // gap 2 (right edge 120)
        ];
        // Box starts at x=122.
        let label = best_label(122.0, 299.0, 100.0, 14.0, &runs);
        assert_eq!(label.as_deref(), Some("Near"));
    }

    #[test]
    fn kind_fillable_excludes_push_buttons_and_unknown() {
        assert!(kind_is_fillable(FieldKind::Text));
        assert!(kind_is_fillable(FieldKind::Checkbox));
        assert!(kind_is_fillable(FieldKind::Signature));
        assert!(!kind_is_fillable(FieldKind::PushButton));
        assert!(!kind_is_fillable(FieldKind::Unknown));
    }

    #[test]
    fn covers_detects_a_widget_over_a_detected_box() {
        let b = DetectedBox {
            page: 0,
            x: 100.0,
            y: 300.0,
            width: 80.0,
            height: 14.0,
        };
        // Widget rect essentially on top of the box.
        assert!(covers(101.0, 301.0, 78.0, 12.0, &b));
        // A widget far away doesn't cover it.
        assert!(!covers(400.0, 100.0, 20.0, 10.0, &b));
    }

    #[test]
    fn classify_signature_falls_back_to_the_label() {
        // Cryptic name, but a "Signature" label routes it to signing.
        assert_eq!(
            classify_signature(FieldKind::Text, "f1_07", Some("Signature")),
            SignatureFieldKind::Signature
        );
        // Name already says initials.
        assert_eq!(
            classify_signature(FieldKind::Text, "spouse_initials", None),
            SignatureFieldKind::Initials
        );
        // Ordinary field stays None.
        assert_eq!(
            classify_signature(FieldKind::Text, "city", Some("City")),
            SignatureFieldKind::None
        );
    }

    #[test]
    fn clean_label_strips_a_trailing_colon() {
        assert_eq!(clean_label("First name :"), "First name");
        assert_eq!(clean_label("  Total  "), "Total");
    }
}
