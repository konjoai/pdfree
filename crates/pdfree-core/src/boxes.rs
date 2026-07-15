//! Rectangular box/cell detection for "click a box, type into it" workflows
//! (Phase 4 add-on).
//!
//! Scanned or flattened forms often draw each fillable box as a stroked
//! rectangle, or a table's cell borders as separate line segments, rather
//! than a real `AcroForm` field (see [`crate::forms`]). This module looks at
//! a page's vector graphics — not text, not form fields — to reconstruct
//! every such box on a page, so a shell can present them all up front (scan
//! on load) rather than guessing one box at a time from a click point. The
//! resulting rectangles are meant to be handed to
//! [`crate::forms::overlay_text`] as the place to stamp typed text.

use pdfium_render::prelude::*;

use crate::error::{PdfError, Result};

/// A detected rectangular box, in PDF points (72/inch, origin at the page's
/// bottom-left corner) — the same convention as [`crate::forms::TextOverlay`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetectedBox {
    /// 0-based page index the box was found on.
    pub page: u16,
    /// Horizontal position of the box, from the page's left edge.
    pub x: f32,
    /// Vertical position of the box, from the page's bottom edge.
    pub y: f32,
    /// Box width.
    pub width: f32,
    /// Box height.
    pub height: f32,
}

impl DetectedBox {
    fn area(&self) -> f32 {
        self.width * self.height
    }

    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x - TOLERANCE
            && x <= self.x + self.width + TOLERANCE
            && y >= self.y - TOLERANCE
            && y <= self.y + self.height + TOLERANCE
    }
}

/// PDF points within which two coordinates are treated as aligned — absorbs
/// the sub-point jitter real-world PDFs have in "straight" ruled lines.
const TOLERANCE: f32 = 1.5;

/// Ignore candidate boxes covering more than this fraction of the page area
/// — almost always a page border or background rect, not a fillable box.
const MAX_BOX_AREA_RATIO: f32 = 0.5;

/// Ignore candidate boxes smaller than this — stray marks, underscores, and
/// glyph strokes aren't fillable boxes.
const MIN_BOX_AREA: f32 = 36.0;

/// Ignore candidate rectangle *paths* (not grid cells) thinner than this on
/// either axis. A single ruled line's own path bounds are a thin sliver
/// (its length by roughly its stroke width) that would otherwise pass the
/// area check while being nothing like a fillable box.
const MIN_RECT_DIMENSION: f32 = 4.0;

/// Shortest horizontal ruled line, in points, that can be a fill-in-the-blank
/// underline. Below this it's more likely a checkbox edge, a divider stub, or
/// glyph detail than a "write on this line" field.
const MIN_UNDERLINE_LEN: f32 = 26.0;

/// Default height, in points, of the input affordance placed *above* a
/// detected underline — roughly one line of hand/typed entry.
const UNDERLINE_FIELD_HEIGHT: f32 = 16.0;

/// The shortest an underline field is allowed to shrink to when the writable
/// space above the line is tight.
const MIN_UNDERLINE_FIELD_HEIGHT: f32 = 9.0;

/// Minimum empty vertical room, in points, that must sit above a horizontal
/// line for it to count as a writable fill line. A line packed tightly under
/// another rule is a table band, not a blank to fill.
const MIN_FILL_GAP: f32 = 8.0;

/// When one box fully contains another whose area is below this fraction of
/// it, the outer box is treated as a *container region* (a section border,
/// an outer table frame) rather than a field of its own — see
/// [`prefer_inner_fields`].
const CONTAINER_INNER_RATIO: f32 = 0.9;

/// Reconstruct every fillable box (drawn rectangle, or table cell formed by
/// ruled lines) on `page`, in PDF points. Meant to be called once as a page
/// loads so a shell can highlight every box up front, rather than
/// point-at-a-time detection.
///
/// The core of this is a table-cell reconstruction over the page's ruled
/// lines: horizontal and vertical strokes are clustered into "rulings"
/// (merging near-collinear segments and near-touching spans), then every
/// pair of adjacent rulings in each direction is checked for a matching
/// pair of cross-rulings that together fully bound a cell — the same
/// technique lattice-based PDF table extractors use. A lone stroked
/// rectangle (common for single checkboxes or signature boxes not part of
/// a grid) is included too, unless it duplicates a cell already found.
///
/// # Errors
///
/// Returns [`PdfError::PageOutOfRange`] if `page` doesn't exist, and
/// propagates `PDFium` / load errors otherwise.
pub fn boxes_on_page(pdf_bytes: &[u8], page: u16) -> Result<Vec<DetectedBox>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    boxes_on_loaded_page(&document, page)
}

/// Same as [`boxes_on_page`], but works from an already-bound `PDFium`
/// document rather than binding and re-parsing `pdf_bytes` itself. Exists so
/// [`crate::pageview`] can gather a rendered page *and* its detected boxes
/// from a single bind + parse — `boxes_on_page`'s own from-scratch bind is by
/// far the heaviest per-page `pdfree-core` call, and re-paying it separately
/// for every page render was the actual root cause of "page navigation is
/// slow" and "even a 1-page PDF is slow to open". This must only ever be
/// called with a `document` from a bind that hasn't been reused across a
/// *different* prior document load — see `crate::pdfium::bind`'s docs on why
/// a single `PDFium` binding can't safely be cached and reused that way.
pub(crate) fn boxes_on_loaded_page(document: &PdfDocument, page: u16) -> Result<Vec<DetectedBox>> {
    let count = document.pages().len();
    if page >= count {
        return Err(PdfError::PageOutOfRange { index: page, count });
    }

    let loaded = document.pages().get(page)?;
    let page_area = (loaded.width().value.max(0.0) * loaded.height().value.max(0.0)).max(1.0);
    let max_area = page_area * MAX_BOX_AREA_RATIO;

    // Horizontal segments as (y, x_min, x_max); vertical as (x, y_min, y_max).
    let mut h_segments: Vec<(f32, f32, f32)> = Vec::new();
    let mut v_segments: Vec<(f32, f32, f32)> = Vec::new();
    let mut rect_paths: Vec<PdfRect> = Vec::new();
    // Bounds of every image (logo, seal, photo, embedded signature) and text
    // run on the page — used to reject a speculative Tier 3/4 candidate that
    // actually already has content sitting in it (see `region_has_content`).
    // A real fillable blank is empty; a rectangle drawn around a logo, or a
    // line with a name/signature already printed above it, is not a field
    // regardless of how well it matches the geometric shape of one.
    let mut image_rects: Vec<PdfRect> = Vec::new();
    let mut text_rects: Vec<PdfRect> = Vec::new();

    for object in loaded.objects().iter() {
        if let Some(image) = object.as_image_object() {
            // A scanned page is commonly one giant background image behind
            // everything else — that's a page backdrop, not a logo/photo
            // overlapping one specific field, and would otherwise flag
            // *every* box on the page as "occupied". Same
            // page-area-fraction cutoff `push_if_new` already uses to drop
            // page borders/backgrounds from box candidates.
            if let Ok(bounds) = image.bounds() {
                let rect = bounds.to_rect();
                if rect.width().value * rect.height().value <= max_area {
                    image_rects.push(rect);
                }
            }
            continue;
        }
        if let Some(text) = object.as_text_object() {
            if let Ok(bounds) = text.bounds() {
                text_rects.push(bounds.to_rect());
            }
            continue;
        }
        let Some(path) = object.as_path_object() else {
            continue;
        };
        if !path.is_stroked().unwrap_or(false) {
            continue;
        }
        if let Ok(bounds) = path.bounds() {
            let rect = bounds.to_rect();
            let area = rect.width().value * rect.height().value;
            let big_enough_both_axes = rect.width().value >= MIN_RECT_DIMENSION
                && rect.height().value >= MIN_RECT_DIMENSION;
            if big_enough_both_axes && (MIN_BOX_AREA..=max_area).contains(&area) {
                rect_paths.push(rect);
            }
        }
        // `path.segments()` yields each segment's *untransformed* raw
        // coordinates (see pdfium-render's own doc comment on
        // `PdfPagePathObjectSegments`) — real-world PDFs routinely place a
        // path via a non-identity object matrix (translation at minimum),
        // so reading raw points directly puts every line in the wrong spot
        // entirely. `.transform(path.matrix()?)` applies that object's own
        // matrix, matching what `path.bounds()` already reports in page
        // space.
        let Ok(matrix) = path.matrix() else {
            continue;
        };
        collect_axis_aligned_segments(
            &path.segments().transform(matrix),
            &mut h_segments,
            &mut v_segments,
        );
    }

    let h_rulings = cluster_rulings(h_segments);
    let v_rulings = cluster_rulings(v_segments);

    // Both tiers below pair "adjacent" vertical dividers — but adjacency
    // must be judged among only the dividers actually relevant to the row
    // in question, not globally by x across the whole page. A page has many
    // rows, each with its own dividers at their own x positions; sorting
    // *all* of them together and taking global neighbors would pair a
    // divider from one row with an unrelated divider from a totally
    // different row whenever their x values happen to interleave. So each
    // row/ruling first filters `v_rulings` down to the ones relevant to it,
    // then pairs adjacent dividers *within that filtered, still-x-sorted
    // subset*.
    let mut boxes = Vec::new();

    // Tier 1: fully closed cells — four rulings that together bound a
    // rectangle (the reliable case: each side confirmed by an actual line).
    for bottom_top in h_rulings.windows(2) {
        let (y_bottom, bottom_spans) = &bottom_top[0];
        let (y_top, top_spans) = &bottom_top[1];
        if *y_top <= *y_bottom {
            continue;
        }
        let relevant: Vec<&(f32, Vec<(f32, f32)>)> = v_rulings
            .iter()
            .filter(|(_, spans)| ruling_spans(spans, *y_bottom, *y_top))
            .collect();
        for pair in relevant.windows(2) {
            let (x_left, left_spans) = pair[0];
            let (x_right, right_spans) = pair[1];
            if !ruling_spans(bottom_spans, *x_left, *x_right)
                || !ruling_spans(top_spans, *x_left, *x_right)
                || !ruling_spans(left_spans, *y_bottom, *y_top)
                || !ruling_spans(right_spans, *y_bottom, *y_top)
            {
                continue;
            }
            push_if_new(
                &mut boxes,
                page,
                *x_left,
                *y_bottom,
                x_right - x_left,
                y_top - y_bottom,
                max_area,
            );
        }
    }

    // Tier 2: "open" cells — a pair of adjacent vertical dividers that both
    // meet the same horizontal ruling but have nothing closing the far
    // side. Real forms draw these constantly: a field gets side dividers
    // and a bottom (or top) rule but no full enclosing box, because the box
    // would be visually redundant with the row above/below.
    // `push_if_new` skips anything that duplicates a Tier 1 box already
    // found (a closed cell's own dividers also satisfy this looser pattern).
    for (y, spans) in &h_rulings {
        let relevant: Vec<&(f32, Vec<(f32, f32)>)> = v_rulings
            .iter()
            .filter(|(_, vspans)| {
                ruling_end_at(vspans, *y, true).is_some()
                    || ruling_end_at(vspans, *y, false).is_some()
            })
            .collect();
        for pair in relevant.windows(2) {
            let (x_left, left_spans) = pair[0];
            let (x_right, right_spans) = pair[1];
            if !ruling_spans(spans, *x_left, *x_right) {
                continue;
            }

            // Dividers rising above this ruling (open-top: box sits above
            // the line, e.g. a labeled blank with side dividers).
            if let (Some(top_l), Some(top_r)) = (
                ruling_end_at(left_spans, *y, true),
                ruling_end_at(right_spans, *y, true),
            ) {
                let top = top_l.min(top_r);
                push_if_new(
                    &mut boxes,
                    page,
                    *x_left,
                    *y,
                    x_right - x_left,
                    top - y,
                    max_area,
                );
            }

            // Dividers hanging below this ruling (open-bottom).
            if let (Some(bot_l), Some(bot_r)) = (
                ruling_end_at(left_spans, *y, false),
                ruling_end_at(right_spans, *y, false),
            ) {
                let bottom = bot_l.max(bot_r);
                push_if_new(
                    &mut boxes,
                    page,
                    *x_left,
                    bottom,
                    x_right - x_left,
                    y - bottom,
                    max_area,
                );
            }
        }
    }

    // Tier 3: a lone stroked rectangle path not already covered above
    // (common for standalone checkboxes or signature boxes not part of a
    // grid at all).
    for rect in rect_paths {
        push_if_new(
            &mut boxes,
            page,
            rect.left().value,
            rect.bottom().value,
            rect.width().value,
            rect.height().value,
            max_area,
        );
    }

    // Tier 4: fill-in-the-blank underlines — a lone horizontal ruled line you
    // write *on*, the single most common "field" on real-world forms
    // (`Name: ______`, `Weight: ______ lbs`). These have no enclosing box at
    // all, so the cell tiers above never see them. A line only qualifies if
    // no vertical divider meets it (that would make it a cell edge, already
    // handled) and there's open room above it to actually write. The
    // affordance is placed sitting on top of the line.
    //
    // Underlines are collected separately, then appended, so
    // `prefer_inner_fields` below can tell a leaf field from a container.
    let underline_start = boxes.len();
    for (y, spans) in &h_rulings {
        for &(lo, hi) in spans {
            if hi - lo < MIN_UNDERLINE_LEN {
                continue;
            }
            if vertical_meets(&v_rulings, lo, hi, *y) {
                continue;
            }
            let gap_above = h_rulings
                .iter()
                .filter(|(y2, s2)| *y2 > *y + TOLERANCE && spans_overlap(s2, lo, hi))
                .map(|(y2, _)| *y2 - *y)
                .fold(f32::INFINITY, f32::min);
            if gap_above < MIN_FILL_GAP {
                continue;
            }
            let height = if gap_above.is_finite() {
                (gap_above - 2.0).clamp(MIN_UNDERLINE_FIELD_HEIGHT, UNDERLINE_FIELD_HEIGHT)
            } else {
                UNDERLINE_FIELD_HEIGHT
            };
            // Dedup underlines only against each other and against
            // similarly-sized boxes (a cell that already captured this line)
            // — *not* against a much larger enclosing region, which
            // `prefer_inner_fields` is responsible for removing instead.
            let candidate = DetectedBox {
                page,
                x: lo,
                y: *y,
                width: hi - lo,
                height,
            };
            if !(MIN_BOX_AREA..=max_area).contains(&candidate.area()) {
                continue;
            }
            let duplicates_peer = boxes.iter().enumerate().any(|(i, b)| {
                overlap_ratio(b, &candidate) > 0.6
                    && (i >= underline_start || b.area() <= candidate.area() * 3.0)
            });
            if duplicates_peer {
                continue;
            }
            boxes.push(candidate);
        }
    }

    // Reject any candidate — from any tier above, closed cell included, since
    // a lone rectangle's own 4 edges satisfy the Tier 1 closed-cell pattern
    // just as well as a real table cell's — that already has an image or a
    // meaningful amount of text sitting inside it. A real fillable blank is
    // empty; a rectangle framing a logo/seal/photo, or a line with a value
    // already printed above it, is not a field regardless of which
    // structural pattern detected it: "when in doubt, don't highlight it."
    let is_occupied = |b: &DetectedBox| {
        region_has_content(&image_rects, &text_rects, b.x, b.y, b.width, b.height)
    };
    // `underline_start` indexes into the *pre-filter* `boxes`; removing
    // occupied entries ahead of it would shift every later index, so it's
    // recomputed here as "how many pre-underline entries survive" rather than
    // reused as-is.
    let new_underline_start = boxes[..underline_start]
        .iter()
        .filter(|b| !is_occupied(b))
        .count();
    let boxes: Vec<DetectedBox> = boxes.into_iter().filter(|b| !is_occupied(b)).collect();

    Ok(prefer_inner_fields(boxes, new_underline_start))
}

/// Drop any box that acts as a *container* for real fields rather than being
/// a field itself: a section border or outer table frame that fully encloses
/// smaller boxes. Core UX Principle: "if there is a field or fields inside of
/// a box, highlight the fields only." A box is dropped when it fully contains
/// either any underline field or at least two smaller boxes — a plain cell
/// that merely holds a single checkbox is left alone.
fn prefer_inner_fields(boxes: Vec<DetectedBox>, underline_start: usize) -> Vec<DetectedBox> {
    let is_underline = |i: usize| i >= underline_start;
    let n = boxes.len();
    let mut drop = vec![false; n];
    for i in 0..n {
        // A leaf fill line is never itself a container.
        if is_underline(i) {
            continue;
        }
        let mut inner_boxes = 0;
        let mut inner_underline = false;
        for j in 0..n {
            if i == j {
                continue;
            }
            if fully_contains(&boxes[i], &boxes[j])
                && boxes[j].area() < CONTAINER_INNER_RATIO * boxes[i].area()
            {
                inner_boxes += 1;
                inner_underline |= is_underline(j);
            }
        }
        if inner_underline || inner_boxes >= 2 {
            drop[i] = true;
        }
    }
    boxes
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !drop[*i])
        .map(|(_, b)| b)
        .collect()
}

/// Whether `outer` fully encloses `inner` (within [`TOLERANCE`]).
fn fully_contains(outer: &DetectedBox, inner: &DetectedBox) -> bool {
    inner.x >= outer.x - TOLERANCE
        && inner.y >= outer.y - TOLERANCE
        && inner.x + inner.width <= outer.x + outer.width + TOLERANCE
        && inner.y + inner.height <= outer.y + outer.height + TOLERANCE
}

/// Whether any of `spans` overlaps `[lo, hi]`.
fn spans_overlap(spans: &[(f32, f32)], lo: f32, hi: f32) -> bool {
    spans
        .iter()
        .any(|&(a, b)| a <= hi + TOLERANCE && lo <= b + TOLERANCE)
}

/// Whether a vertical ruling actually meets the horizontal line at height `y`
/// somewhere across `[lo, hi]` — i.e. this horizontal span is a cell edge,
/// not a free-standing fill line.
fn vertical_meets(v_rulings: &[(f32, Vec<(f32, f32)>)], lo: f32, hi: f32, y: f32) -> bool {
    v_rulings.iter().any(|(vx, vspans)| {
        *vx >= lo - TOLERANCE
            && *vx <= hi + TOLERANCE
            && vspans
                .iter()
                .any(|&(a, b)| a - TOLERANCE <= y && y <= b + TOLERANCE)
    })
}

/// Find the smallest box from [`boxes_on_page`] that encloses `(x, y)` —
/// a convenience for a shell that wants a single point-driven lookup (e.g.
/// a manual double-click fallback) without re-deriving the grid itself.
///
/// # Errors
///
/// Returns [`PdfError::PageOutOfRange`] if `page` doesn't exist, and
/// propagates `PDFium` / load errors otherwise.
pub fn box_at_point(pdf_bytes: &[u8], page: u16, x: f32, y: f32) -> Result<Option<DetectedBox>> {
    let boxes = boxes_on_page(pdf_bytes, page)?;
    Ok(boxes
        .into_iter()
        .filter(|b| b.contains(x, y))
        .min_by(|a, b| {
            a.area()
                .partial_cmp(&b.area())
                .unwrap_or(std::cmp::Ordering::Equal)
        }))
}

/// Walk a path's segments, recording any `LineTo` whose endpoints are
/// (within [`TOLERANCE`]) purely horizontal or purely vertical. A `BezierTo`
/// breaks the straight-line chain — the segment after a curve doesn't get
/// compared against the curve's endpoint as if it were a straight run.
fn collect_axis_aligned_segments(
    segments: &PdfPagePathObjectSegments<'_>,
    h_segments: &mut Vec<(f32, f32, f32)>,
    v_segments: &mut Vec<(f32, f32, f32)>,
) {
    let mut last: Option<(f32, f32)> = None;
    for segment in segments.iter() {
        let (px, py) = segment.point();
        let (px, py) = (px.value, py.value);

        match segment.segment_type() {
            PdfPathSegmentType::LineTo => {
                if let Some((lx, ly)) = last {
                    if (py - ly).abs() <= TOLERANCE {
                        h_segments.push((ly, lx.min(px), lx.max(px)));
                    } else if (px - lx).abs() <= TOLERANCE {
                        v_segments.push((lx, ly.min(py), ly.max(py)));
                    }
                }
                last = Some((px, py));
            }
            PdfPathSegmentType::MoveTo => last = Some((px, py)),
            PdfPathSegmentType::BezierTo | PdfPathSegmentType::Unknown => last = None,
        }
    }
}

/// Cluster raw `(coord, span_min, span_max)` segments into rulings: entries
/// within [`TOLERANCE`] of each other's coordinate are merged into one
/// ruling whose span is the union of all their spans (so a ruling drawn as
/// several separate strokes along the same line is treated as one line).
/// Returned sorted ascending by coordinate.
fn cluster_rulings(mut segments: Vec<(f32, f32, f32)>) -> Vec<(f32, Vec<(f32, f32)>)> {
    segments.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut clusters: Vec<(f32, Vec<(f32, f32)>)> = Vec::new();
    for (coord, lo, hi) in segments {
        match clusters.last_mut() {
            Some((last_coord, spans)) if (coord - *last_coord).abs() <= TOLERANCE => {
                spans.push((lo, hi));
            }
            _ => clusters.push((coord, vec![(lo, hi)])),
        }
    }

    clusters
        .into_iter()
        .map(|(coord, spans)| (coord, merge_spans(spans)))
        .collect()
}

/// Merge overlapping or near-touching (within [`TOLERANCE`]) spans into the
/// smallest set of disjoint spans covering the same ground.
fn merge_spans(mut spans: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for (lo, hi) in spans {
        match merged.last_mut() {
            Some(last) if lo <= last.1 + TOLERANCE => last.1 = last.1.max(hi),
            _ => merged.push((lo, hi)),
        }
    }
    merged
}

/// Whether a ruling's merged spans fully cover `[lo, hi]` (within tolerance)
/// — i.e. the ruled line actually runs across this candidate cell edge,
/// rather than just existing somewhere on the same coordinate.
fn ruling_spans(spans: &[(f32, f32)], lo: f32, hi: f32) -> bool {
    spans
        .iter()
        .any(|&(a, b)| a - TOLERANCE <= lo && hi <= b + TOLERANCE)
}

/// Find a span in a cross-ruling's spans that touches `coord` at one end,
/// and return its other end — i.e. "does a divider line start/end exactly
/// here, and if so, how far does it extend?" `want_far_end_above` selects
/// which end of the divider touches `coord`: `true` means the divider's
/// *low* end is at `coord` and it extends upward (its high end is
/// returned); `false` means its *high* end is at `coord` and it extends
/// downward (its low end is returned).
fn ruling_end_at(spans: &[(f32, f32)], coord: f32, want_far_end_above: bool) -> Option<f32> {
    spans.iter().find_map(|&(lo, hi)| {
        if want_far_end_above && (lo - coord).abs() <= TOLERANCE {
            Some(hi)
        } else if !want_far_end_above && (hi - coord).abs() <= TOLERANCE {
            Some(lo)
        } else {
            None
        }
    })
}

/// Validate a candidate box (positive area within bounds) and push it only
/// if it doesn't substantially overlap a box already found — later, looser
/// detection tiers shouldn't duplicate an earlier, more precise one.
#[allow(clippy::too_many_arguments)]
fn push_if_new(
    boxes: &mut Vec<DetectedBox>,
    page: u16,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    max_area: f32,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let candidate = DetectedBox {
        page,
        x,
        y,
        width,
        height,
    };
    if !(MIN_BOX_AREA..=max_area).contains(&candidate.area()) {
        return;
    }
    if boxes.iter().any(|b| overlap_ratio(b, &candidate) > 0.6) {
        return;
    }
    boxes.push(candidate);
}

/// Whether an image or text run already occupies a meaningful portion of
/// `(x, y, width, height)` — i.e. this candidate field isn't actually blank.
/// A sliver of overlap (a descender poking into the box, a line's own
/// stroke) shouldn't disqualify a real field, so this requires the overlap
/// to cover a real fraction of *whichever is smaller* — the candidate or the
/// content — not just touch it. Dividing by the candidate's area alone would
/// miss a small "Jane Doe" sitting on a wide underline (it can easily cover
/// under 15% of a whole wide field while still very obviously being the
/// field's already-written answer); dividing by the content's area alone
/// instead correctly reads that case as "this text is basically entirely
/// inside the field" regardless of how wide the field itself is.
fn region_has_content(
    images: &[PdfRect],
    texts: &[PdfRect],
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> bool {
    const OCCUPIED_RATIO: f32 = 0.15;
    let area = width * height;
    if area <= 0.0 {
        return false;
    }
    let overlaps = |r: &PdfRect| -> bool {
        let x_overlap = (r.right().value).min(x + width) - (r.left().value).max(x);
        let y_overlap = (r.top().value).min(y + height) - (r.bottom().value).max(y);
        if x_overlap <= 0.0 || y_overlap <= 0.0 {
            return false;
        }
        let content_area = r.width().value * r.height().value;
        let smaller = area.min(content_area);
        if smaller <= 0.0 {
            return false;
        }
        (x_overlap * y_overlap) / smaller >= OCCUPIED_RATIO
    };
    images.iter().any(overlaps) || texts.iter().any(overlaps)
}

fn overlap_ratio(a: &DetectedBox, b: &DetectedBox) -> f32 {
    let x_overlap = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
    let y_overlap = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
    if x_overlap <= 0.0 || y_overlap <= 0.0 {
        return 0.0;
    }
    let intersection = x_overlap * y_overlap;
    let smaller_area = a.area().min(b.area());
    if smaller_area <= 0.0 {
        0.0
    } else {
        intersection / smaller_area
    }
}
