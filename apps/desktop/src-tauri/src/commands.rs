//! Tauri IPC commands wrapping `pdfree-core` directly (native — no WASM
//! needed, unlike `apps/web`'s in-browser path). Mirrors exactly the subset
//! of `pdfree-wasm`'s surface that `apps/web`'s React UI actually calls —
//! see `crates/pdfree-wasm/src/lib.rs` for the full engine surface and
//! `apps/web/src/lib/engine.ts` for the frontend-side backend switch that
//! calls these when running under Tauri instead of calling the WASM module
//! directly. Same JSON field-naming convention (`camelCase`) as the WASM
//! side, so `apps/web/src/types.ts` describes both without modification —
//! this is what makes "reuse the web UI" (CLAUDE.md's Phase 4 Tauri item)
//! true in practice, not just nominally.
//!
//! Every command is stateless — takes the current PDF bytes, returns new
//! bytes or derived data — matching `pdfree-core`'s own "operate on whole-
//! document byte buffers" function shapes directly, rather than wrapping
//! them in a stateful per-document object (there is no long-lived
//! `PdfDocument` handle held on the Rust side between IPC calls).

use pdfree_core::{convert, fields, forms, pages, signatures};
use serde::{Deserialize, Serialize};

fn to_err(e: pdfree_core::PdfError) -> String {
    e.to_string()
}

// ---------------------------------------------------------------------------
// Document (Phase 0): metadata, render, page size.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfo {
    pub page_count: u16,
    pub title: Option<String>,
    pub author: Option<String>,
}

#[tauri::command]
pub fn document_info(pdf_bytes: Vec<u8>) -> Result<DocumentInfo, String> {
    let doc = pdfree_core::Document::from_bytes(pdf_bytes, None).map_err(to_err)?;
    Ok(DocumentInfo {
        page_count: doc.page_count(),
        title: doc.metadata().title.clone(),
        author: doc.metadata().author.clone(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSize {
    pub width: f32,
    pub height: f32,
}

#[tauri::command]
pub fn page_size(pdf_bytes: Vec<u8>, index: u16) -> Result<PageSize, String> {
    let doc = pdfree_core::Document::from_bytes(pdf_bytes, None).map_err(to_err)?;
    let (width, height) = doc.page_size(index).map_err(to_err)?;
    Ok(PageSize { width, height })
}

#[tauri::command]
pub fn render_page(pdf_bytes: Vec<u8>, index: u16, dpi: f32) -> Result<Vec<u8>, String> {
    let doc = pdfree_core::Document::from_bytes(pdf_bytes, None).map_err(to_err)?;
    doc.render_page(index, &pdfree_core::RenderOptions::with_dpi(dpi))
        .map_err(to_err)
}

#[tauri::command]
pub fn fit_to_page_dpi(
    page_width_pts: f32,
    page_height_pts: f32,
    viewport_width_px: f32,
    viewport_height_px: f32,
) -> f32 {
    pdfree_core::fit_to_page(
        page_width_pts,
        page_height_pts,
        viewport_width_px,
        viewport_height_px,
    )
    .dpi
}

// ---------------------------------------------------------------------------
// Forms (Phase 1) + boxes (Phase 4 add-on).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    pub name: String,
    pub kind: String,
    pub value: Option<String>,
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub signature_kind: String,
}

impl From<forms::FormField> for FormField {
    fn from(f: forms::FormField) -> Self {
        Self {
            name: f.name,
            kind: format!("{:?}", f.kind),
            value: f.value,
            page: f.page,
            x: f.x,
            y: f.y,
            width: f.width,
            height: f.height,
            signature_kind: format!("{:?}", f.signature_kind),
        }
    }
}

#[tauri::command]
pub fn form_fields(pdf_bytes: Vec<u8>) -> Result<Vec<FormField>, String> {
    forms::fields(&pdf_bytes)
        .map(|fields| fields.into_iter().map(Into::into).collect())
        .map_err(to_err)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOverlay {
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub font_size: f32,
}

impl From<TextOverlay> for forms::TextOverlay {
    fn from(o: TextOverlay) -> Self {
        Self {
            page: o.page,
            x: o.x,
            y: o.y,
            text: o.text,
            font_size: o.font_size,
        }
    }
}

#[tauri::command]
pub fn overlay_text(pdf_bytes: Vec<u8>, overlays: Vec<TextOverlay>) -> Result<Vec<u8>, String> {
    let overlays: Vec<forms::TextOverlay> = overlays.into_iter().map(Into::into).collect();
    forms::overlay_text(&pdf_bytes, &overlays).map_err(to_err)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedBox {
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<pdfree_core::boxes::DetectedBox> for DetectedBox {
    fn from(b: pdfree_core::boxes::DetectedBox) -> Self {
        Self {
            page: b.page,
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        }
    }
}

#[tauri::command]
pub fn boxes_on_page(pdf_bytes: Vec<u8>, page: u16) -> Result<Vec<DetectedBox>, String> {
    pdfree_core::boxes::boxes_on_page(&pdf_bytes, page)
        .map(|boxes| boxes.into_iter().map(Into::into).collect())
        .map_err(to_err)
}

// ---------------------------------------------------------------------------
// Fields (label-aware fillable-field detection, Phase 4 add-on).
//
// The list a shell should actually highlight: every AcroForm widget, plus
// every detected box/line that has a human-readable label next to it and
// doesn't duplicate a widget. Mirrors `pdfree-wasm`'s `fillableFields`
// (crates/pdfree-wasm/src/lib.rs) so `apps/web`'s React UI gets identical
// field-overlay accuracy whether it's running in-browser (WASM) or under
// Tauri (this command).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldSource {
    AcroForm,
    Detected,
}

impl From<fields::FieldSource> for FieldSource {
    fn from(s: fields::FieldSource) -> Self {
        match s {
            fields::FieldSource::AcroForm => FieldSource::AcroForm,
            fields::FieldSource::Detected => FieldSource::Detected,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillableField {
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: Option<String>,
    pub field_name: Option<String>,
    pub signature_kind: String,
    pub source: FieldSource,
}

impl From<fields::FillableField> for FillableField {
    fn from(f: fields::FillableField) -> Self {
        Self {
            page: f.page,
            x: f.x,
            y: f.y,
            width: f.width,
            height: f.height,
            label: f.label,
            field_name: f.field_name,
            signature_kind: format!("{:?}", f.signature_kind),
            source: f.source.into(),
        }
    }
}

#[tauri::command]
pub fn fillable_fields(pdf_bytes: Vec<u8>, page: u16) -> Result<Vec<FillableField>, String> {
    fields::fillable_fields(&pdf_bytes, page)
        .map(|found| found.into_iter().map(Into::into).collect())
        .map_err(to_err)
}

// ---------------------------------------------------------------------------
// Signatures (Phase 2).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignaturePlacement {
    pub page: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<SignaturePlacement> for signatures::SignaturePlacement {
    fn from(p: SignaturePlacement) -> Self {
        Self {
            page: p.page,
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureAudit {
    pub signer_name: String,
    pub signed_at: String,
    pub device_info: Option<String>,
}

impl From<SignatureAudit> for signatures::SignatureAudit {
    fn from(a: SignatureAudit) -> Self {
        Self {
            signer_name: a.signer_name,
            signed_at: a.signed_at,
            device_info: a.device_info,
        }
    }
}

#[tauri::command]
pub fn place_signature_with_audit(
    pdf_bytes: Vec<u8>,
    image_png: Vec<u8>,
    at: SignaturePlacement,
    audit: SignatureAudit,
) -> Result<Vec<u8>, String> {
    signatures::place_signature_with_audit(&pdf_bytes, &image_png, at.into(), &audit.into())
        .map_err(to_err)
}

// ---------------------------------------------------------------------------
// Pages (Phase 3) + convert (Phase 3).
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn merge_documents(documents: Vec<Vec<u8>>) -> Result<Vec<u8>, String> {
    pages::merge(&documents).map_err(to_err)
}

#[tauri::command]
pub fn rotate_page(pdf_bytes: Vec<u8>, page: u16, rotation: String) -> Result<Vec<u8>, String> {
    let rotation = match rotation.as_str() {
        "None" => pages::Rotation::None,
        "Clockwise90" => pages::Rotation::Clockwise90,
        "Clockwise180" => pages::Rotation::Clockwise180,
        "Clockwise270" => pages::Rotation::Clockwise270,
        other => return Err(format!("unknown rotation: {other}")),
    };
    pages::rotate(&pdf_bytes, page, rotation).map_err(to_err)
}

#[tauri::command]
pub fn extract_pages(pdf_bytes: Vec<u8>, page_list: Vec<u16>) -> Result<Vec<u8>, String> {
    pages::extract(&pdf_bytes, &page_list).map_err(to_err)
}

#[tauri::command]
pub fn from_image(image_bytes: Vec<u8>, dpi: f32) -> Result<Vec<u8>, String> {
    convert::from_image(&image_bytes, dpi).map_err(to_err)
}
