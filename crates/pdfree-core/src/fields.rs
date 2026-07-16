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
/// immediately to the field's left on the same line (`Name: ____`), one
/// sitting just above it (a column header), or one immediately to its right
/// (`☐ Yes` — the standard checkbox-then-label layout, common enough on real
/// forms that skipping it silently drops every such checkbox from a flat
/// form with no backing `AcroForm` widget). Returns the label text, trimmed
/// of a trailing colon. A run whose center falls *inside* the box is treated
/// as the field's own content, never a label.
fn best_label(bx: f32, by: f32, bw: f32, bh: f32, runs: &[LabelRun]) -> Option<String> {
    let mut best: Option<(f32, &str)> = None;
    for run in runs {
        if !has_alnum(&run.text) {
            continue;
        }
        // A run centered inside the box is filled content, not a label.
        if run_is_box_content(run, bx, by, bw, bh) {
            continue;
        }
        // The run's distance to the box if it qualifies as a left-hand
        // label, a right-hand label, or a column header above — the
        // nearest of the three if more than one applies.
        let distance = [
            left_label_gap(run, bx, by, bh),
            right_label_gap(run, bx, by, bw, bh),
            above_label_gap(run, bx, by, bw, bh),
        ]
        .into_iter()
        .flatten()
        .reduce(f32::min);

        if let Some(distance) = distance {
            if best.map_or(true, |(d, _)| distance < d) {
                best = Some((distance, run.text.trim()));
            }
        }
    }
    best.map(|(_, text)| clean_label(text))
}

/// Whether `run`'s center point sits inside the box's interior (shrunk by
/// [`ALIGN_PAD`] on every side) — i.e. the run is the box's own filled
/// content rather than a label sitting next to it.
fn run_is_box_content(run: &LabelRun, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    let cx = run.x + run.width / 2.0;
    let cy = run.y + run.height / 2.0;
    cx > bx + ALIGN_PAD
        && cx < bx + bw - ALIGN_PAD
        && cy > by + ALIGN_PAD
        && cy < by + bh - ALIGN_PAD
}

/// The gap (in points, `>= 0`) from `run`'s right edge to the box's left edge
/// if `run` qualifies as a left-hand label — it ends within [`MAX_LEFT_GAP`]
/// of (and no more than [`ALIGN_PAD`] past) the box's left edge and shares its
/// vertical band. `None` if it doesn't qualify.
fn left_label_gap(run: &LabelRun, bx: f32, by: f32, bh: f32) -> Option<f32> {
    let left_gap = bx - (run.x + run.width);
    if (-ALIGN_PAD..=MAX_LEFT_GAP).contains(&left_gap)
        && bands_overlap(by, by + bh, run.y, run.y + run.height)
    {
        Some(left_gap.max(0.0))
    } else {
        None
    }
}

/// The gap (in points, `>= 0`) from the box's right edge to `run`'s left edge
/// if `run` qualifies as a right-hand label — the mirror image of
/// [`left_label_gap`], for the `☐ Yes` checkbox-then-label pattern. `None` if
/// it doesn't qualify.
fn right_label_gap(run: &LabelRun, bx: f32, by: f32, bw: f32, bh: f32) -> Option<f32> {
    let right_gap = run.x - (bx + bw);
    if (-ALIGN_PAD..=MAX_LEFT_GAP).contains(&right_gap)
        && bands_overlap(by, by + bh, run.y, run.y + run.height)
    {
        Some(right_gap.max(0.0))
    } else {
        None
    }
}

/// The gap (in points, `>= 0`) from the box's top edge up to `run` if `run`
/// qualifies as a column header sitting above the box — it's within
/// [`MAX_ABOVE_GAP`] of (and no more than [`ALIGN_PAD`] below) the box's top
/// edge and overlaps it horizontally. `None` if it doesn't qualify.
fn above_label_gap(run: &LabelRun, bx: f32, by: f32, bw: f32, bh: f32) -> Option<f32> {
    let above_gap = run.y - (by + bh);
    if (-ALIGN_PAD..=MAX_ABOVE_GAP).contains(&above_gap)
        && bands_overlap(bx, bx + bw, run.x, run.x + run.width)
    {
        Some(above_gap.max(0.0))
    } else {
        None
    }
}

/// Whether two 1-D bands `[a0, a1]` and `[b0, b1]` overlap within [`ALIGN_PAD`].
fn bands_overlap(a0: f32, a1: f32, b0: f32, b1: f32) -> bool {
    a0.min(a1) - ALIGN_PAD <= b0.max(b1) && b0.min(b1) - ALIGN_PAD <= a1.max(a0)
}

/// Whether an `AcroForm` field rect substantially covers a detected box (so
/// the box is a duplicate of the real widget and should be dropped). True
/// when the two rects overlap by more than half the *smaller* rect's area —
/// which dedupes a widget and a same-size drawn box around it, and a small
/// widget nested inside a larger detected box (or vice versa), while leaving a
/// mere corner clip alone. Using the smaller rect's area (not the box's)
/// keeps the criterion symmetric and reachable in both directions, so it's
/// fully determined by the overlap geometry.
fn covers(fx: f32, fy: f32, fw: f32, fh: f32, b: &DetectedBox) -> bool {
    let x_overlap = ((fx + fw).min(b.x + b.width) - fx.max(b.x)).max(0.0);
    let y_overlap = ((fy + fh).min(b.y + b.height) - fy.max(b.y)).max(0.0);
    let smaller_area = (fw * fh).min(b.width * b.height).max(1.0);
    (x_overlap * y_overlap) / smaller_area > 0.5
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
    fn on_a_tie_the_first_qualifying_label_wins() {
        // Both runs qualify as left labels (box left edge x=100, band
        // [300,314]); the nearer one wins, and on an exact tie the incumbent
        // (first seen) is kept — pinning the strict `<` tie-break.
        let far_then_near = vec![
            run("Far", 0.0, 300.0, 40.0, 10.0),   // right 40, gap 60
            run("Near", 50.0, 300.0, 45.0, 10.0), // right 95, gap 5
        ];
        assert_eq!(
            best_label(100.0, 300.0, 100.0, 14.0, &far_then_near).as_deref(),
            Some("Near")
        );
        let tied = vec![
            run("First", 50.0, 300.0, 45.0, 10.0), // right 95, gap 5
            run("Second", 50.0, 305.0, 45.0, 9.0), // right 95, gap 5
        ];
        assert_eq!(
            best_label(100.0, 300.0, 100.0, 14.0, &tied).as_deref(),
            Some("First")
        );
    }

    #[test]
    fn kind_fillable_excludes_push_buttons_and_unknown() {
        assert!(kind_is_fillable(FieldKind::Text));
        assert!(kind_is_fillable(FieldKind::Checkbox));
        assert!(kind_is_fillable(FieldKind::Signature));
        assert!(!kind_is_fillable(FieldKind::PushButton));
        assert!(!kind_is_fillable(FieldKind::Unknown));
    }

    fn bx(x: f32, y: f32, w: f32, h: f32) -> DetectedBox {
        DetectedBox {
            page: 0,
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn covers_detects_a_widget_over_a_detected_box() {
        let b = bx(100.0, 300.0, 80.0, 14.0);
        // Widget rect essentially on top of the box.
        assert!(covers(101.0, 301.0, 78.0, 12.0, &b));
        // A widget far away doesn't cover it.
        assert!(!covers(400.0, 100.0, 20.0, 10.0, &b));
    }

    #[test]
    fn covers_true_when_rects_substantially_coincide() {
        // Same-size box drawn around a widget.
        assert!(covers(1.0, 1.0, 10.0, 10.0, &bx(0.0, 0.0, 10.0, 10.0)));
        // A small widget fully nested in a larger detected box, and the
        // reverse (small box nested in a larger widget) — both dedupe via the
        // smaller rect's area.
        assert!(covers(3.0, 3.0, 4.0, 4.0, &bx(0.0, 0.0, 10.0, 10.0)));
        assert!(covers(0.0, 0.0, 20.0, 20.0, &bx(6.0, 6.0, 4.0, 4.0)));
    }

    #[test]
    fn covers_false_for_a_mere_corner_clip() {
        // Overlap is well under half the smaller rect.
        assert!(!covers(8.0, 8.0, 10.0, 10.0, &bx(0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn covers_false_when_disjoint_or_edge_touching() {
        // Disjoint in x, disjoint in y, and touching exactly at an edge
        // (zero overlap) all count as not covered.
        assert!(!covers(50.0, 0.0, 10.0, 10.0, &bx(0.0, 0.0, 10.0, 10.0)));
        assert!(!covers(0.0, 50.0, 10.0, 10.0, &bx(0.0, 0.0, 10.0, 10.0)));
        assert!(!covers(10.0, 0.0, 10.0, 10.0, &bx(0.0, 0.0, 10.0, 10.0)));
        assert!(!covers(0.0, 10.0, 10.0, 10.0, &bx(0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn covers_is_decided_strictly_above_half_the_smaller_area() {
        // Exactly half the smaller rect overlaps -> not covered (strict `>`).
        assert!(!covers(0.0, 0.0, 10.0, 10.0, &bx(5.0, 0.0, 10.0, 10.0)));
        // Just over half -> covered.
        assert!(covers(0.0, 0.0, 10.0, 10.0, &bx(4.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn covers_uses_the_smaller_rects_area_as_the_denominator() {
        // Overlap is exactly half of the field's area (the smaller rect):
        // not covered. If the area were computed as width+height instead of
        // width*height, the denominator would shrink and this would flip.
        assert!(!covers(0.0, 0.0, 10.0, 6.0, &bx(5.0, 0.0, 10.0, 10.0)));
        // Same, with the *box* as the smaller rect.
        assert!(!covers(5.0, 0.0, 10.0, 10.0, &bx(0.0, 0.0, 10.0, 6.0)));
    }

    #[test]
    fn bands_overlap_respects_the_alignment_pad() {
        // Bands 3pt apart still overlap thanks to ALIGN_PAD (kills the pad
        // being added instead of subtracted), tested both orderings.
        assert!(bands_overlap(0.0, 10.0, 13.0, 20.0));
        assert!(bands_overlap(13.0, 20.0, 0.0, 10.0));
        // Clearly overlapping and clearly disjoint.
        assert!(bands_overlap(0.0, 10.0, 5.0, 15.0));
        assert!(!bands_overlap(0.0, 10.0, 50.0, 60.0));
        // Disjoint bands where dividing by the pad (instead of subtracting)
        // would wrongly report overlap, tested both orderings.
        assert!(!bands_overlap(100.0, 110.0, 0.0, 40.0));
        assert!(!bands_overlap(0.0, 40.0, 100.0, 110.0));
    }

    #[test]
    fn run_is_box_content_only_when_center_is_strictly_inside() {
        // Box (100,200) sized 100x50 → interior center range x in (103,197),
        // y in (203,247). A run's center is (x + w/2, y + h/2).
        let content =
            |x, y| run_is_box_content(&run("x", x, y, 10.0, 10.0), 100.0, 200.0, 100.0, 50.0);
        // Center clearly inside.
        assert!(content(145.0, 215.0)); // center (150, 220)
                                        // Center exactly on each padded boundary → excluded (strict compares).
        assert!(!content(98.0, 215.0)); // cx = 103 == bx + PAD
        assert!(!content(192.0, 215.0)); // cx = 197 == bx + bw - PAD
        assert!(!content(145.0, 198.0)); // cy = 203 == by + PAD
        assert!(!content(145.0, 242.0)); // cy = 247 == by + bh - PAD
                                         // Far outside on both axes.
        assert!(!content(0.0, 0.0));
    }

    #[test]
    fn left_label_gap_qualifies_a_run_just_left_and_aligned() {
        // Box left edge x=100, band y in [300,314].
        let lg = |x, y, w, h| left_label_gap(&run("L", x, y, w, h), 100.0, 300.0, 14.0);
        // Ends 10pt left of the box, same line → gap 10.
        assert_eq!(lg(50.0, 300.0, 40.0, 10.0), Some(10.0));
        // Ends right at the box's left edge (gap 0), and slightly past it (to
        // −ALIGN_PAD) still qualifies, clamped to 0.
        assert_eq!(lg(50.0, 300.0, 50.0, 10.0), Some(0.0));
        assert_eq!(lg(50.0, 300.0, 53.0, 10.0), Some(0.0)); // left_gap = −3 (== −PAD)
                                                            // Past the box by more than the pad → not a left label.
        assert_eq!(lg(50.0, 300.0, 55.0, 10.0), None); // left_gap = −5
                                                       // Exactly at the max reach vs one point beyond it.
        assert_eq!(lg(0.0, 300.0, 40.0, 10.0), Some(60.0)); // left_gap = 60
        assert_eq!(lg(0.0, 300.0, 39.0, 10.0), None); // left_gap = 61
                                                      // Right distance, wrong line (vertical bands don't overlap) — several
                                                      // positions above/below the box's band, which together pin the
                                                      // band-extent arithmetic (`by + bh`, `run.y + run.height`) in the
                                                      // overlap check: mutating those to `-`/`*` would distort a band enough
                                                      // to wrongly report overlap for one of these.
        assert_eq!(lg(50.0, 100.0, 40.0, 10.0), None); // band well below
        assert_eq!(lg(50.0, 320.0, 40.0, 10.0), None); // band just above
        assert_eq!(lg(50.0, 400.0, 40.0, 10.0), None); // band far above
                                                       // A run one band down that still overlaps — kills mutating the box's
                                                       // top-of-band `by + bh` into `by - bh`.
        assert_eq!(lg(50.0, 310.0, 40.0, 10.0), Some(10.0));
    }

    #[test]
    fn right_label_gap_qualifies_a_run_just_right_and_aligned() {
        // Box right edge x=150 (bx=100, bw=50), band y in [300,314] — the
        // mirror image of left_label_gap_qualifies_a_run_just_left_and_aligned.
        let rg = |x, y, w, h| right_label_gap(&run("Yes", x, y, w, h), 100.0, 300.0, 50.0, 14.0);
        // Starts 10pt right of the box, same line → gap 10.
        assert_eq!(rg(160.0, 300.0, 40.0, 10.0), Some(10.0));
        // Starts right at the box's right edge (gap 0), and slightly left of
        // it (to −ALIGN_PAD) still qualifies, clamped to 0.
        assert_eq!(rg(150.0, 300.0, 40.0, 10.0), Some(0.0));
        assert_eq!(rg(147.0, 300.0, 40.0, 10.0), Some(0.0)); // right_gap = −3 (== −PAD)
                                                             // Overlapping the box by more than the pad → not a right label.
        assert_eq!(rg(145.0, 300.0, 40.0, 10.0), None); // right_gap = −5
                                                        // Exactly at the max reach vs one point beyond it.
        assert_eq!(rg(210.0, 300.0, 40.0, 10.0), Some(60.0)); // right_gap = 60
        assert_eq!(rg(211.0, 300.0, 40.0, 10.0), None); // right_gap = 61
                                                        // Right distance, wrong line (vertical bands don't overlap).
        assert_eq!(rg(160.0, 100.0, 40.0, 10.0), None); // band well below
        assert_eq!(rg(160.0, 320.0, 40.0, 10.0), None); // band just above
        assert_eq!(rg(160.0, 400.0, 40.0, 10.0), None); // band far above
                                                        // A run one band down that still overlaps.
        assert_eq!(rg(160.0, 310.0, 40.0, 10.0), Some(10.0));
    }

    #[test]
    fn best_label_finds_a_checkbox_style_label_to_the_right() {
        // "☐ Yes" layout: the box (a checkbox) sits at x in [100,114], and
        // its label starts just to the right on the same line — the pattern
        // `left`/`above` alone can't see, since the label is neither to the
        // left nor above.
        let runs = vec![run("Yes", 120.0, 300.0, 20.0, 10.0)];
        assert_eq!(
            best_label(100.0, 299.0, 14.0, 12.0, &runs).as_deref(),
            Some("Yes")
        );
    }

    #[test]
    fn above_label_gap_qualifies_a_header_just_above_and_overlapping() {
        // Box top edge y=320 (by=300,bh=20), horizontal span x in [100,190].
        let ag = |x, y, w, h| above_label_gap(&run("H", x, y, w, h), 100.0, 300.0, 90.0, 20.0);
        // Sits 6pt above the box, overlapping horizontally → gap 6.
        assert_eq!(ag(120.0, 326.0, 40.0, 9.0), Some(6.0));
        // Right at the top edge, and dipping to −ALIGN_PAD below it, clamp to 0.
        assert_eq!(ag(120.0, 320.0, 40.0, 9.0), Some(0.0));
        assert_eq!(ag(120.0, 317.0, 40.0, 9.0), Some(0.0)); // above_gap = −3
                                                            // More than the pad below the top edge → not a header.
        assert_eq!(ag(120.0, 316.0, 40.0, 9.0), None); // above_gap = −4
                                                       // Exactly at the max reach vs one point beyond.
        assert_eq!(ag(120.0, 342.0, 40.0, 9.0), Some(22.0)); // above_gap = 22
        assert_eq!(ag(120.0, 343.0, 40.0, 9.0), None); // above_gap = 23
                                                       // Right height, no horizontal overlap — several positions left/right
                                                       // of the box's span, which pin the span arithmetic (`bx + bw`,
                                                       // `run.x + run.width`) in the overlap check.
        assert_eq!(ag(0.0, 326.0, 40.0, 9.0), None); // span left of the box
        assert_eq!(ag(300.0, 326.0, 40.0, 9.0), None); // span right of the box
        assert_eq!(ag(1000.0, 326.0, 40.0, 9.0), None); // span far right
                                                        // A run whose left edge is left of the box but whose right edge just
                                                        // reaches into it qualifies — so mutating `run.x + run.width` (its
                                                        // right edge) to `-`/`*` drops or spuriously adds the overlap.
        assert_eq!(ag(70.0, 326.0, 35.0, 9.0), Some(6.0)); // right edge 105, into [100,190]
        assert_eq!(ag(50.0, 326.0, 40.0, 9.0), None); // right edge 90, short of the box
    }

    #[test]
    fn non_empty_keeps_content_and_drops_blank() {
        assert_eq!(non_empty("hi").as_deref(), Some("hi"));
        assert_eq!(non_empty("  spaced  ").as_deref(), Some("spaced"));
        assert_eq!(non_empty("   "), None);
        assert_eq!(non_empty(""), None);
    }

    #[test]
    fn has_alnum_detects_letters_or_digits() {
        assert!(has_alnum("a"));
        assert!(has_alnum("Total 7"));
        assert!(!has_alnum("---"));
        assert!(!has_alnum("   "));
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
