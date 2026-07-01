//! `PDFium` library discovery and binding.
//!
//! `PDFree` renders through `PDFium` — the same engine Chrome uses — via the
//! `pdfium-render` crate, which loads the shared library dynamically at
//! runtime. That keeps `pdfree-core` free of any build-time link to a native
//! blob and lets each platform ship the `PDFium` binary its own way (see
//! `docs/pdfium-bundling.md`).
//!
//! ## Discovery order
//!
//! [`bind`] looks for the library in this order and returns as soon as one
//! binds successfully:
//!
//! 1. `$PDFIUM_DYNAMIC_LIB_PATH` — an explicit path to the library file *or*
//!    to the directory that contains it. Always wins when set.
//! 2. A bundled `vendor/pdfium/` directory, resolved relative to both the
//!    current working directory and the crate manifest, so it works in a
//!    checkout and in a packaged app.
//! 3. The system library search path (`bind_to_system_library`).
//!
//! Every path tried is recorded so a failure can tell the user exactly where
//! `PDFree` looked.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use pdfium_render::prelude::{Pdfium, PdfiumError};

use crate::error::PdfError;

/// The platform-specific `PDFium` file name, e.g. `libpdfium.so`,
/// `libpdfium.dylib`, or `pdfium.dll`.
#[must_use]
pub fn library_file_name() -> OsString {
    Pdfium::pdfium_platform_library_name()
}

/// Candidate directories that may hold a bundled `PDFium` binary.
fn vendor_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("vendor").join("pdfium"));
    }
    // Resolve relative to this crate so tests and tools work regardless of the
    // process working directory. `../../vendor/pdfium` walks up from
    // `crates/pdfree-core` to the workspace root.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    dirs.push(manifest.join("../../vendor/pdfium"));
    dirs
}

/// Bind to `PDFium`, searching the locations documented on this module.
///
/// Returns a ready-to-use [`Pdfium`] instance, or [`PdfError::PdfiumUnavailable`]
/// listing every path that was tried.
///
/// # Errors
///
/// Returns [`PdfError::PdfiumUnavailable`] if the library cannot be found or
/// loaded from any of the searched locations.
pub fn bind() -> crate::error::Result<Pdfium> {
    let mut searched: Vec<PathBuf> = Vec::new();

    // Build the ordered list of explicit file candidates: the environment
    // override first (highest priority), then any bundled vendor directory.
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PDFIUM_DYNAMIC_LIB_PATH") {
        candidates.extend(candidates_for(&PathBuf::from(path)));
    }
    for dir in vendor_dirs() {
        candidates.push(dir.join(library_file_name()));
    }

    for candidate in candidates {
        if let Ok(pdfium) = try_bind(&candidate) {
            return Ok(pdfium);
        }
        searched.push(candidate);
    }

    // Finally, fall back to the system library search path. Its error is the
    // most representative one to surface if everything failed.
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(source) => {
            searched.push(PathBuf::from(format!(
                "<system: {}>",
                library_file_name().to_string_lossy()
            )));
            Err(PdfError::PdfiumUnavailable { searched, source })
        }
    }
}

/// Expand a user-supplied path into the concrete library files to try: if the
/// path is a directory, look for the platform library name inside it.
fn candidates_for(path: &Path) -> Vec<PathBuf> {
    if path.is_dir() {
        vec![path.join(library_file_name())]
    } else {
        vec![path.to_path_buf()]
    }
}

fn try_bind(path: &Path) -> std::result::Result<Pdfium, PdfiumError> {
    Pdfium::bind_to_library(path).map(Pdfium::new)
}
