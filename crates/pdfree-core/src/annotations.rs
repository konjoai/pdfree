//! Highlight, underline, strikethrough, and sticky-note annotations (Phase 2).
//!
//! Markup annotations (highlight/underline/strikeout) are positioned by a
//! quad-point region — the same rectangle-in-PDF-points convention used
//! throughout this crate (see [`crate::forms::TextOverlay`]) — rather than by
//! anchoring to an extracted text run, since `pdfree-core` doesn't do text
//! layout extraction yet. A sticky note is a small icon anchored at a point
//! that opens a text popup when clicked.
//!
//! ## A known `PDFium` rendering gap
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

use pdfium_render::prelude::*;

use crate::error::{PdfError, Result};

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
    /// Markup color. Defaults to yellow for `Highlight` and red for
    /// `Underline`/`StrikeOut` when not given; ignored for `Note` (sticky
    /// notes use the viewer's icon color).
    pub color: Option<Color>,
    /// The note text: the sticky-note body for `Note`, or a reviewer comment
    /// attached to a markup annotation (shown in the same popup UI).
    pub note: Option<String>,
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

        let mut page = document.pages().get(annotation.page)?;
        let rect = PdfRect::new_from_values(
            annotation.y,
            annotation.x,
            annotation.y + annotation.height,
            annotation.x + annotation.width,
        );

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
        }
    }

    Ok(document.save_to_bytes()?)
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
                AnnotationKind::Underline | AnnotationKind::StrikeOut => {
                    annotation.stroke_color().ok()
                }
                AnnotationKind::Note => None,
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
        _ => None,
    }
}

fn to_pdf_color(color: Color, alpha: u8) -> PdfColor {
    PdfColor::new(color.r, color.g, color.b, alpha)
}

fn from_pdf_color(color: PdfColor) -> Color {
    Color::new(color.red(), color.green(), color.blue())
}
