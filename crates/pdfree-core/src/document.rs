//! Open, inspect, and save PDF documents.
//!
//! A [`Document`] owns the PDF bytes in memory. Every operation that needs
//! `PDFium` binds it on demand and works from those bytes, which keeps the core
//! filesystem-free — the same code renders on macOS, Linux, Windows, and in the
//! browser (where there is no filesystem at all).

use std::path::Path;

use pdfium_render::prelude::*;

use crate::error::Result;
use crate::renderer::{self, RenderOptions};

/// A loaded PDF, held as its raw bytes plus cheaply-read summary information.
#[derive(Debug, Clone)]
pub struct Document {
    bytes: Vec<u8>,
    metadata: Metadata,
}

/// Document-level information read once at open time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    /// Number of pages in the document.
    pub page_count: u16,
}

impl Document {
    /// Open a PDF from a file on disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or `PDFium` cannot parse it.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(bytes, None)
    }

    /// Open a password-protected PDF from a file on disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the password is wrong.
    pub fn open_with_password<P: AsRef<Path>>(path: P, password: &str) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(bytes, Some(password))
    }

    /// Open a PDF already held in memory. `password` unlocks encrypted files.
    ///
    /// # Errors
    ///
    /// Returns an error if `PDFium` cannot be loaded or the bytes are not a
    /// readable PDF.
    pub fn from_bytes(bytes: Vec<u8>, password: Option<&str>) -> Result<Self> {
        let pdfium = crate::pdfium::bind()?;
        let document = pdfium.load_pdf_from_byte_slice(&bytes, password)?;
        let metadata = read_metadata(&document);
        // Drop the PDFium document/handle here; `Document` keeps only the bytes.
        drop(document);
        Ok(Self { bytes, metadata })
    }

    /// Document metadata (title, author, page count, …).
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Number of pages.
    #[must_use]
    pub fn page_count(&self) -> u16 {
        self.metadata.page_count
    }

    /// The raw PDF bytes — the source of truth for saving and re-processing.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Render page `index` (0-based) to PNG bytes at the given options.
    ///
    /// # Errors
    ///
    /// Propagates any error from [`renderer::render_page_to_png`].
    pub fn render_page(&self, index: u16, options: &RenderOptions) -> Result<Vec<u8>> {
        renderer::render_page_to_png(&self.bytes, index, options)
    }

    /// Save the document to disk, preserving the original bytes exactly.
    ///
    /// Phase 0 is read + render only, so saving is a byte-for-byte write.
    /// Later phases (edit, forms, sign) will re-serialize a mutated document
    /// here instead.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        std::fs::write(path, &self.bytes)?;
        Ok(())
    }
}

/// Pull the interesting metadata tags out of a `PDFium` document.
fn read_metadata(document: &PdfDocument) -> Metadata {
    let tags = document.metadata();
    let get = |tag: PdfDocumentMetadataTagType| -> Option<String> {
        tags.get(tag)
            .map(|t| t.value().to_string())
            .filter(|s| !s.is_empty())
    };

    Metadata {
        title: get(PdfDocumentMetadataTagType::Title),
        author: get(PdfDocumentMetadataTagType::Author),
        subject: get(PdfDocumentMetadataTagType::Subject),
        creator: get(PdfDocumentMetadataTagType::Creator),
        producer: get(PdfDocumentMetadataTagType::Producer),
        page_count: document.pages().len(),
    }
}
