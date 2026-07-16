//! Highlight, underline, strikethrough, sticky-note, shape, and freehand
//! (ink) annotations (Phase 2, plus a later shape/freehand add-on).
//!
//! Markup annotations (highlight/underline/strikeout) are positioned by a
//! quad-point region — the same rectangle-in-PDF-points convention used
//! throughout this crate (see [`crate::forms::TextOverlay`]) — rather than by
//! anchoring to an extracted text run, since `pdfree-core` doesn't do text
//! layout extraction yet. A sticky note is a small icon anchored at a point
//! that opens a text popup when clicked.
//!
//! ## A known `PDFium` rendering gap (highlight/underline/strikeout only)
//!
//! Every annotation this module creates gets correct, spec-compliant data —
//! `/QuadPoints`, `/Rect`, `/C` (color), `/Contents` — verified by reading it
//! straight back with [`list`]. Per the PDF spec (ISO 32000-1, §12.5.5), a
//! compliant reader without an explicit appearance stream (`/AP`) should
//! synthesize one from those properties, and most real-world viewers
//! (Acrobat, macOS Preview, browser PDF viewers) do exactly that.
//!
//! `pdfium-render` 0.8.37 doesn't expose a way to attach an explicit `/AP` to
//! a `Highlight`/`Underline`/`Strikeout` annotation (only `Stamp` and `Ink`
//! get a public `objects_mut()` to draw one), and `PDFium`'s own rendering —
//! which is what [`crate::renderer::render_page_to_png`] uses — does *not*
//! synthesize a default appearance for these three types the way other
//! viewers do. So a highlight/underline/strikeout added here is correct and
//! portable, but won't currently show up in `pdfree-core`'s own render
//! preview. [`AnnotationKind::Note`] is unaffected — `PDFium` does synthesize
//! a default icon for `Text` annotations, confirmed by rendering it.
//!
//! ## Shapes and freehand ink deliberately avoid that gap
//!
//! [`AnnotationKind::Rectangle`], [`Circle`](AnnotationKind::Circle),
//! [`Line`](AnnotationKind::Line), and [`Arrow`](AnnotationKind::Arrow) are
//! drawn as real vector path objects inside a `Stamp` annotation (the one
//! generic container `pdfium-render` *does* expose `objects_mut()` for), and
//! [`AnnotationKind::Ink`] the same way inside a real `Ink` annotation. Both
//! draw genuine, always-visible page content rather than relying on a
//! reader synthesizing an appearance from bare geometry — so, unlike the
//! three markup kinds above, these render correctly in `pdfree-core`'s own
//! preview immediately, confirmed by a real render-and-diff test.
//!
//! **Known read-back limitation**: every shape kind becomes the *same*
//! `Stamp` annotation type once written, and `pdfium-render`'s annotation
//! collection exposes only `iter()` (read-only), never `iter_mut()` — so
//! there is no way to get back to a `Stamp` annotation's own path objects
//! (only reachable via `objects_mut()`, which needs a mutable handle this
//! crate has no way to obtain from a plain read) to inspect their geometry
//! and infer which shape it originally was. [`list`] therefore reports every
//! `Stamp` annotation as [`AnnotationKind::Shape`] rather than guessing —
//! `Rectangle`/`Circle`/`Line`/`Arrow` are meaningful to [`annotate`] (they
//! control what actually gets drawn) but never come back out of [`list`].
//! `Ink` is unaffected by this — it's a real, distinct `PdfPageAnnotationType`
//! PDFium already tells apart from `Stamp`, so it round-trips through
//! `list` as itself.

use pdfium_render::prelude::*;

use crate::error::{PdfError, Result};

/// A single point in PDF points, page-space (72 per inch, origin at the
/// page's bottom-left corner) — same convention as every other coordinate in
/// this crate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// An RGB color, 0-255 per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Color {
    /// Convenience constructor.
    #[must_use]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// A standard PDF markup annotation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKind {
    /// A translucent highlight rectangle behind the marked region.
    Highlight,
    /// A line under the marked region.
    Underline,
    /// A line through the marked region.
    StrikeOut,
    /// A sticky-note icon that opens a text popup when clicked.
    Note,
    /// An outlined rectangle spanning the annotation's bounding box.
    Rectangle,
    /// An outlined ellipse inscribed in the annotation's bounding box.
    Circle,
    /// A straight line between [`Annotation::points`]' two entries.
    Line,
    /// A straight line between [`Annotation::points`]' two entries, with an
    /// arrowhead drawn at the second point.
    Arrow,
    /// A freehand stroke following [`Annotation::points`] in order (2+
    /// points required).
    Ink,
    /// What [`list`] reports for any `Stamp`-backed annotation it reads back
    /// — see the module doc comment for why it can't tell `Rectangle`,
    /// `Circle`, `Line`, and `Arrow` apart after the fact. Never meaningful
    /// as an [`annotate`] input.
    Shape,
}

/// One annotation to add to a page, in PDF points (72 per inch, origin at the
/// page's bottom-left corner) — the same convention as
/// [`crate::forms::TextOverlay`].
#[derive(Debug, Clone)]
pub struct Annotation {
    /// 0-based page index to annotate.
    pub page: u16,
    /// What kind of markup this is.
    pub kind: AnnotationKind,
    /// Horizontal position of the annotation's bounding box, from the page's
    /// left edge.
    pub x: f32,
    /// Vertical position of the annotation's bounding box, from the page's
    /// bottom edge.
    pub y: f32,
    /// Width of the annotation's bounding box. For [`AnnotationKind::Note`]
    /// this is the sticky-note icon's size.
    pub width: f32,
    /// Height of the annotation's bounding box. For [`AnnotationKind::Note`]
    /// this is the sticky-note icon's size.
    pub height: f32,
    /// Markup color. Defaults to yellow for `Highlight`, red for
    /// `Underline`/`StrikeOut`/`Rectangle`/`Circle`/`Line`/`Arrow`/`Ink` when
    /// not given; ignored for `Note` (sticky notes use the viewer's icon
    /// color).
    pub color: Option<Color>,
    /// The note text: the sticky-note body for `Note`, or a reviewer comment
    /// attached to a markup annotation (shown in the same popup UI).
    pub note: Option<String>,
    /// Explicit geometry for `Line`/`Arrow` (exactly 2 points: start, end)
    /// and `Ink` (2+ points, the freehand stroke in drawn order). Ignored for
    /// every other kind, which are fully described by the `x`/`y`/`width`/
    /// `height` bounding box above.
    pub points: Vec<Point>,
}

const DEFAULT_HIGHLIGHT: Color = Color {
    r: 255,
    g: 235,
    b: 59,
};
const DEFAULT_MARKUP_LINE: Color = Color {
    r: 220,
    g: 38,
    b: 38,
};
/// Stroke width for `Rectangle`/`Circle`/`Line`/`Arrow`.
const SHAPE_STROKE_WIDTH: f32 = 2.0;
/// Stroke width for `Ink` — slightly thicker to read as a pen/marker stroke.
const INK_STROKE_WIDTH: f32 = 3.0;
/// Arrowhead length and half-width, in PDF points.
const ARROWHEAD_LENGTH: f32 = 10.0;
const ARROWHEAD_HALF_WIDTH: f32 = 4.0;

/// Add one or more markup/note annotations to a document, returning the
/// updated PDF as new bytes.
///
/// # Errors
///
/// Returns [`PdfError::PageOutOfRange`] if an annotation names a page that
/// doesn't exist, [`PdfError::InvalidAnnotation`] if `width`/`height` is not
/// a positive finite number, and propagates `PDFium` / load errors otherwise.
pub fn annotate(pdf_bytes: &[u8], annotations: &[Annotation]) -> Result<Vec<u8>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;
    let count = document.pages().len();

    for annotation in annotations {
        if annotation.page >= count {
            return Err(PdfError::PageOutOfRange {
                index: annotation.page,
                count,
            });
        }

        let mut page = document.pages().get(annotation.page)?;

        match annotation.kind {
            AnnotationKind::Highlight
            | AnnotationKind::Underline
            | AnnotationKind::StrikeOut
            | AnnotationKind::Note
            | AnnotationKind::Rectangle
            | AnnotationKind::Circle => {
                let rect = bounding_box_rect(annotation)?;
                apply_box_annotation(&mut page, annotation, rect)?;
            }
            AnnotationKind::Line | AnnotationKind::Arrow => {
                let [p1, p2] = require_points(&annotation.points, 2, 2, "Line/Arrow")?;
                apply_line_annotation(&document, &mut page, annotation, p1, p2)?;
            }
            AnnotationKind::Ink => {
                if annotation.points.len() < 2 {
                    return Err(PdfError::InvalidAnnotation(format!(
                        "Ink needs at least 2 points, got {}",
                        annotation.points.len()
                    )));
                }
                apply_ink_annotation(&document, &mut page, annotation)?;
            }
            AnnotationKind::Shape => {
                return Err(PdfError::InvalidAnnotation(
                    "AnnotationKind::Shape is a list()-only value, not a valid annotate() input"
                        .to_string(),
                ));
            }
        }
    }

    Ok(document.save_to_bytes()?)
}

/// Validate and build the bounding-box `PdfRect` shared by every box-shaped
/// annotation kind (`Highlight`/`Underline`/`StrikeOut`/`Note`/`Rectangle`/
/// `Circle`).
fn bounding_box_rect(annotation: &Annotation) -> Result<PdfRect> {
    let valid_size = annotation.width.is_finite()
        && annotation.width > 0.0
        && annotation.height.is_finite()
        && annotation.height > 0.0;
    if !valid_size {
        return Err(PdfError::InvalidAnnotation(format!(
            "width/height must be positive, finite numbers (got {}x{})",
            annotation.width, annotation.height
        )));
    }
    Ok(PdfRect::new_from_values(
        annotation.y,
        annotation.x,
        annotation.y + annotation.height,
        annotation.x + annotation.width,
    ))
}

fn apply_box_annotation(
    page: &mut PdfPage<'_>,
    annotation: &Annotation,
    rect: PdfRect,
) -> Result<()> {
    match annotation.kind {
        AnnotationKind::Highlight => {
            let color = annotation.color.unwrap_or(DEFAULT_HIGHLIGHT);
            let mut markup = page.annotations_mut().create_highlight_annotation()?;
            markup.set_bounds(rect)?;
            markup
                .attachment_points_mut()
                .create_attachment_point_at_end(PdfQuadPoints::from_rect(&rect))?;
            markup.set_fill_color(to_pdf_color(color, 128))?;
            if let Some(note) = &annotation.note {
                markup.set_contents(note)?;
            }
        }
        AnnotationKind::Underline => {
            let color = annotation.color.unwrap_or(DEFAULT_MARKUP_LINE);
            let mut markup = page.annotations_mut().create_underline_annotation()?;
            markup.set_bounds(rect)?;
            markup
                .attachment_points_mut()
                .create_attachment_point_at_end(PdfQuadPoints::from_rect(&rect))?;
            markup.set_stroke_color(to_pdf_color(color, 255))?;
            if let Some(note) = &annotation.note {
                markup.set_contents(note)?;
            }
        }
        AnnotationKind::StrikeOut => {
            let color = annotation.color.unwrap_or(DEFAULT_MARKUP_LINE);
            let mut markup = page.annotations_mut().create_strikeout_annotation()?;
            markup.set_bounds(rect)?;
            markup
                .attachment_points_mut()
                .create_attachment_point_at_end(PdfQuadPoints::from_rect(&rect))?;
            markup.set_stroke_color(to_pdf_color(color, 255))?;
            if let Some(note) = &annotation.note {
                markup.set_contents(note)?;
            }
        }
        AnnotationKind::Note => {
            let text = annotation.note.as_deref().unwrap_or("");
            let mut sticky = page.annotations_mut().create_text_annotation(text)?;
            sticky.set_bounds(rect)?;
        }
        AnnotationKind::Rectangle => {
            let color = annotation.color.unwrap_or(DEFAULT_MARKUP_LINE);
            let stroke_color = to_pdf_color(color, 255);
            let mut stamp = page.annotations_mut().create_stamp_annotation()?;
            stamp.set_bounds(rect)?;
            // Set on the annotation itself too (not just the path object
            // below) so `list()` can read the color back — the visible
            // stroke comes from the path object, but nothing reads that
            // back per the module doc comment's `iter_mut()` gap.
            stamp.set_stroke_color(stroke_color)?;
            stamp.objects_mut().create_path_object_rect(
                rect,
                Some(stroke_color),
                Some(PdfPoints::new(SHAPE_STROKE_WIDTH)),
                None,
            )?;
            if let Some(note) = &annotation.note {
                stamp.set_contents(note)?;
            }
        }
        AnnotationKind::Circle => {
            let color = annotation.color.unwrap_or(DEFAULT_MARKUP_LINE);
            let stroke_color = to_pdf_color(color, 255);
            let mut stamp = page.annotations_mut().create_stamp_annotation()?;
            stamp.set_bounds(rect)?;
            stamp.set_stroke_color(stroke_color)?;
            stamp.objects_mut().create_path_object_ellipse(
                rect,
                Some(stroke_color),
                Some(PdfPoints::new(SHAPE_STROKE_WIDTH)),
                None,
            )?;
            if let Some(note) = &annotation.note {
                stamp.set_contents(note)?;
            }
        }
        AnnotationKind::Line
        | AnnotationKind::Arrow
        | AnnotationKind::Ink
        | AnnotationKind::Shape => {
            unreachable!("apply_box_annotation is only called for box-shaped kinds")
        }
    }
    Ok(())
}

fn apply_line_annotation(
    document: &PdfDocument<'_>,
    page: &mut PdfPage<'_>,
    annotation: &Annotation,
    p1: Point,
    p2: Point,
) -> Result<()> {
    let color = annotation.color.unwrap_or(DEFAULT_MARKUP_LINE);
    let stroke_color = to_pdf_color(color, 255);
    let mut points = vec![p1, p2];
    if annotation.kind == AnnotationKind::Arrow {
        let (tip, left, right) = arrowhead(p1, p2);
        points.extend([tip, left, right]);
    }
    let bounds = bounding_rect(&points);

    let mut stamp = page.annotations_mut().create_stamp_annotation()?;
    stamp.set_bounds(bounds)?;
    stamp.set_stroke_color(stroke_color)?;
    stamp.objects_mut().create_path_object_line(
        PdfPoints::new(p1.x),
        PdfPoints::new(p1.y),
        PdfPoints::new(p2.x),
        PdfPoints::new(p2.y),
        stroke_color,
        PdfPoints::new(SHAPE_STROKE_WIDTH),
    )?;

    if annotation.kind == AnnotationKind::Arrow {
        let (tip, left, right) = arrowhead(p1, p2);
        let mut head = PdfPagePathObject::new(
            document,
            PdfPoints::new(tip.x),
            PdfPoints::new(tip.y),
            Some(stroke_color),
            Some(PdfPoints::new(SHAPE_STROKE_WIDTH)),
            Some(stroke_color),
        )?;
        head.line_to(PdfPoints::new(left.x), PdfPoints::new(left.y))?;
        head.line_to(PdfPoints::new(right.x), PdfPoints::new(right.y))?;
        head.close_path()?;
        stamp.objects_mut().add_path_object(head)?;
    }

    if let Some(note) = &annotation.note {
        stamp.set_contents(note)?;
    }
    Ok(())
}

fn apply_ink_annotation(
    document: &PdfDocument<'_>,
    page: &mut PdfPage<'_>,
    annotation: &Annotation,
) -> Result<()> {
    let color = annotation.color.unwrap_or(DEFAULT_MARKUP_LINE);
    let stroke_color = to_pdf_color(color, 255);
    let bounds = bounding_rect(&annotation.points);

    let mut ink = page.annotations_mut().create_ink_annotation()?;
    ink.set_bounds(bounds)?;
    ink.set_stroke_color(stroke_color)?;

    let first = annotation.points[0];
    let mut stroke = PdfPagePathObject::new(
        document,
        PdfPoints::new(first.x),
        PdfPoints::new(first.y),
        Some(stroke_color),
        Some(PdfPoints::new(INK_STROKE_WIDTH)),
        None,
    )?;
    for point in &annotation.points[1..] {
        stroke.line_to(PdfPoints::new(point.x), PdfPoints::new(point.y))?;
    }
    ink.objects_mut().add_path_object(stroke)?;

    if let Some(note) = &annotation.note {
        ink.set_contents(note)?;
    }
    Ok(())
}

/// Require exactly `min..=max` points, returning them as a fixed-size array
/// (only ever called with `min == max == 2` today, hence the array return —
/// kept general enough to read clearly if a 3-point kind is ever added).
fn require_points(points: &[Point], min: usize, max: usize, kind_name: &str) -> Result<[Point; 2]> {
    if points.len() < min || points.len() > max || min != 2 || max != 2 {
        return Err(PdfError::InvalidAnnotation(format!(
            "{kind_name} needs exactly 2 points, got {}",
            points.len()
        )));
    }
    Ok([points[0], points[1]])
}

/// The smallest axis-aligned `PdfRect` enclosing every point, expanded by a
/// small margin so a thin/zero-area line or single-direction stroke still
/// gets a non-degenerate bounding box.
fn bounding_rect(points: &[Point]) -> PdfRect {
    const MARGIN: f32 = SHAPE_STROKE_WIDTH;
    let min_x = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min) - MARGIN;
    let max_x = points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max) + MARGIN;
    let min_y = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min) - MARGIN;
    let max_y = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max) + MARGIN;
    PdfRect::new_from_values(min_y, min_x, max_y, max_x)
}

/// The arrowhead triangle for a line from `p1` (tail) to `p2` (tip): returns
/// `(tip, left, right)`. Degenerates to a triangle pointing along the
/// positive x-axis if `p1 == p2` (zero-length line) rather than dividing by
/// zero.
fn arrowhead(p1: Point, p2: Point) -> (Point, Point, Point) {
    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    let len = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = if len > 0.0 {
        (dx / len, dy / len)
    } else {
        (1.0, 0.0)
    };
    let (px, py) = (-uy, ux);

    let base_x = p2.x - ux * ARROWHEAD_LENGTH;
    let base_y = p2.y - uy * ARROWHEAD_LENGTH;

    let left = Point::new(
        base_x + px * ARROWHEAD_HALF_WIDTH,
        base_y + py * ARROWHEAD_HALF_WIDTH,
    );
    let right = Point::new(
        base_x - px * ARROWHEAD_HALF_WIDTH,
        base_y - py * ARROWHEAD_HALF_WIDTH,
    );
    (p2, left, right)
}

/// One annotation read back from a document, as reported by [`list`].
#[derive(Debug, Clone)]
pub struct AnnotationInfo {
    /// 0-based page index the annotation is on.
    pub page: u16,
    /// What kind of markup this is.
    pub kind: AnnotationKind,
    /// Horizontal position of the annotation's bounding box, from the page's
    /// left edge.
    pub x: f32,
    /// Vertical position of the annotation's bounding box, from the page's
    /// bottom edge.
    pub y: f32,
    /// Width of the annotation's bounding box.
    pub width: f32,
    /// Height of the annotation's bounding box.
    pub height: f32,
    /// The markup color, if the annotation kind carries one.
    pub color: Option<Color>,
    /// The note/comment text, if any.
    pub note: Option<String>,
}

/// Enumerate the highlight/underline/strikeout/note annotations in a
/// document, in page order.
///
/// Other PDF annotation kinds this crate doesn't create — links, form
/// widgets, popups, stamps, and so on — are skipped rather than reported
/// under an approximate [`AnnotationKind`].
///
/// # Errors
///
/// Returns an error if `PDFium` cannot be loaded or the bytes are not a
/// readable PDF.
pub fn list(pdf_bytes: &[u8]) -> Result<Vec<AnnotationInfo>> {
    let pdfium = crate::pdfium::bind()?;
    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)?;

    let mut out = Vec::new();
    for (page_index, page) in document.pages().iter().enumerate() {
        for annotation in page.annotations().iter() {
            let Some(kind) = annotation_kind(annotation.annotation_type()) else {
                continue;
            };

            let bounds = annotation.bounds().unwrap_or(PdfRect::ZERO);
            let color = match kind {
                AnnotationKind::Highlight => annotation.fill_color().ok(),
                AnnotationKind::Underline
                | AnnotationKind::StrikeOut
                | AnnotationKind::Shape
                | AnnotationKind::Ink => annotation.stroke_color().ok(),
                AnnotationKind::Note => None,
                AnnotationKind::Rectangle
                | AnnotationKind::Circle
                | AnnotationKind::Line
                | AnnotationKind::Arrow => {
                    unreachable!("annotation_kind() never returns these — list()-only mapping")
                }
            }
            .map(from_pdf_color);

            out.push(AnnotationInfo {
                // Page counts are u16 throughout this crate; page_index is
                // bounded by document.pages().len(), so this never truncates.
                #[allow(clippy::cast_possible_truncation)]
                page: page_index as u16,
                kind,
                x: bounds.left().value,
                y: bounds.bottom().value,
                width: bounds.width().value,
                height: bounds.height().value,
                color,
                note: annotation.contents(),
            });
        }
    }
    Ok(out)
}

fn annotation_kind(pdfium_kind: PdfPageAnnotationType) -> Option<AnnotationKind> {
    match pdfium_kind {
        PdfPageAnnotationType::Highlight => Some(AnnotationKind::Highlight),
        PdfPageAnnotationType::Underline => Some(AnnotationKind::Underline),
        PdfPageAnnotationType::Strikeout => Some(AnnotationKind::StrikeOut),
        PdfPageAnnotationType::Text => Some(AnnotationKind::Note),
        // Every shape kind (`Rectangle`/`Circle`/`Line`/`Arrow`) is written
        // as a `Stamp` annotation — see the module doc comment for why
        // `list()` can't tell which one this was.
        PdfPageAnnotationType::Stamp => Some(AnnotationKind::Shape),
        PdfPageAnnotationType::Ink => Some(AnnotationKind::Ink),
        _ => None,
    }
}

fn to_pdf_color(color: Color, alpha: u8) -> PdfColor {
    PdfColor::new(color.r, color.g, color.b, alpha)
}

fn from_pdf_color(color: PdfColor) -> Color {
    Color::new(color.red(), color.green(), color.blue())
}
