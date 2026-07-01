//! WebAssembly bindings for `pdfree-core`.
//!
//! Exposes the Phase 0 surface — open a PDF from bytes, read its page count and
//! metadata, and render a page to PNG — to JavaScript. The web app
//! (`apps/web`) imports the `wasm-pack`-generated module and drives these
//! directly from React.

use pdfree_core::{Document, RenderOptions};
use wasm_bindgen::prelude::*;

/// A PDF document loaded in the browser.
#[wasm_bindgen]
pub struct PdfDocument {
    inner: Document,
}

#[wasm_bindgen]
impl PdfDocument {
    /// Load a document from raw bytes (e.g. a `File`/`ArrayBuffer` from an
    /// `<input type="file">`).
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: Vec<u8>) -> std::result::Result<PdfDocument, JsError> {
        let inner = Document::from_bytes(bytes, None).map_err(to_js)?;
        Ok(Self { inner })
    }

    /// Number of pages.
    #[wasm_bindgen(js_name = pageCount)]
    pub fn page_count(&self) -> u16 {
        self.inner.page_count()
    }

    /// Document title, if present.
    pub fn title(&self) -> Option<String> {
        self.inner.metadata().title.clone()
    }

    /// Document author, if present.
    pub fn author(&self) -> Option<String> {
        self.inner.metadata().author.clone()
    }

    /// Render page `index` (0-based) to PNG bytes at the given DPI.
    #[wasm_bindgen(js_name = renderPage)]
    pub fn render_page(&self, index: u16, dpi: f32) -> std::result::Result<Vec<u8>, JsError> {
        self.inner
            .render_page(index, &RenderOptions::with_dpi(dpi))
            .map_err(to_js)
    }
}

fn to_js(err: pdfree_core::PdfError) -> JsError {
    JsError::new(&err.to_string())
}
