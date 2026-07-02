//! # pdfree-core
//!
//! The `PDFree` engine: the single, platform-agnostic source of truth for all
//! PDF logic. It compiles to native code (macOS, Linux, Windows) and to WASM
//! for the browser, and is wrapped by the platform shells through
//! `pdfree-ffi` (`UniFFI` → Swift) and `pdfree-wasm` (wasm-bindgen → JS).
//!
//! Rendering and parsing go through [PDFium](https://pdfium.googlesource.com/pdfium/)
//! — the same engine Chrome uses — via the `pdfium-render` crate, loaded
//! dynamically at runtime (see [`pdfium`] for library discovery).
//!
//! ## Phase 0 (this milestone)
//!
//! Prove the `PDFium` integration: open a document and render a page to PNG.
//!
//! ```no_run
//! use pdfree_core::{Document, RenderOptions};
//!
//! let doc = Document::open("contract.pdf")?;
//! println!("{} pages", doc.page_count());
//! let png = doc.render_page(0, &RenderOptions::with_dpi(150.0))?;
//! std::fs::write("page-1.png", png)?;
//! # Ok::<(), pdfree_core::PdfError>(())
//! ```
//!
//! Later phases fill in [`forms`], [`signatures`], [`annotations`],
//! [`editor`], [`pages`], and [`convert`], which currently return
//! [`PdfError::NotImplemented`].

pub mod annotations;
pub mod boxes;
pub mod convert;
pub mod document;
pub mod editor;
pub mod error;
pub mod forms;
pub mod pages;
pub mod pdfium;
pub mod renderer;
pub mod signatures;

pub use document::{Document, Metadata};
pub use error::{PdfError, Result};
pub use renderer::{fit_to_page, RenderOptions};

/// Convenience: open a document from a file path.
///
/// Equivalent to [`Document::open`].
///
/// # Errors
///
/// Propagates any error from [`Document::open`].
pub fn open_document<P: AsRef<std::path::Path>>(path: P) -> Result<Document> {
    Document::open(path)
}

/// Convenience: render page `index` (0-based) of a document to PNG bytes.
///
/// Equivalent to [`Document::render_page`].
///
/// # Errors
///
/// Propagates any error from [`Document::render_page`].
pub fn render_page(document: &Document, index: u16, options: &RenderOptions) -> Result<Vec<u8>> {
    document.render_page(index, options)
}
