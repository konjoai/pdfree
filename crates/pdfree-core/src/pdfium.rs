//! `PDFium` library discovery and binding.
//!
//! `PDFree` renders through `PDFium` — the same engine Chrome uses — via the
//! `pdfium-render` crate. Binding is platform-dependent, gated on
//! `target_arch`:
//!
//! - **Native** (macOS/Linux/Windows): loads the shared library dynamically
//!   at runtime. That keeps `pdfree-core` free of any build-time link to a
//!   native blob and lets each platform ship the `PDFium` binary its own way
//!   (see `docs/pdfium-bundling.md`). [`bind`] looks in this order and
//!   returns as soon as one binds successfully:
//!   1. `$PDFIUM_DYNAMIC_LIB_PATH` — an explicit path to the library file
//!      *or* to the directory that contains it. Always wins when set.
//!   2. A bundled `vendor/pdfium/` directory, resolved relative to both the
//!      current working directory and the crate manifest, so it works in a
//!      checkout and in a packaged app.
//!   3. The system library search path (`bind_to_system_library`).
//!
//!   Every path tried is recorded so a failure can tell the user exactly
//!   where `PDFree` looked.
//!
//! - **`wasm32`**: there is no filesystem to search — the actual `PDFium`
//!   WASM module must be loaded and initialized from JavaScript *before*
//!   any Rust code in this crate runs, by calling `pdfium-render`'s exported
//!   `initialize_pdfium_render(pdfiumModule, ourWasmModule, debug)` (see
//!   `docs/pdfium-bundling.md` for the web app's exact setup). [`bind`] just
//!   checks whether that already happened.

#[cfg(not(target_arch = "wasm32"))]
mod native {
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
        // Resolve relative to this crate so tests and tools work regardless
        // of the process working directory. `../../vendor/pdfium` walks up
        // from `crates/pdfree-core` to the workspace root.
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        dirs.push(manifest.join("../../vendor/pdfium"));
        dirs
    }

    pub fn bind() -> crate::error::Result<Pdfium> {
        let mut searched: Vec<PathBuf> = Vec::new();

        // Build the ordered list of explicit file candidates: the
        // environment override first (highest priority), then any bundled
        // vendor directory.
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

        // Finally, fall back to the system library search path. Its error is
        // the most representative one to surface if everything failed.
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

    /// Expand a user-supplied path into the concrete library files to try:
    /// if the path is a directory, look for the platform library name
    /// inside it.
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
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::path::PathBuf;

    use pdfium_render::prelude::Pdfium;

    use crate::error::PdfError;

    /// Checks whether JavaScript has already initialized the `PDFium` WASM
    /// module (via `pdfium-render`'s exported `initialize_pdfium_render`)
    /// and, if so, binds to it. There is nothing else to search — unlike
    /// native targets, `wasm32` has no filesystem and no bundled-binary
    /// fallback; module loading is entirely the web app's responsibility,
    /// before this function is ever called.
    pub fn bind() -> crate::error::Result<Pdfium> {
        Pdfium::bind_to_system_library()
            .map(Pdfium::new)
            .map_err(|source| PdfError::PdfiumUnavailable {
                searched: vec![PathBuf::from(
                    "<wasm: call pdfium-render's initialize_pdfium_render() \
                     from JavaScript before any PDFree operation>",
                )],
                source,
            })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{bind, library_file_name};

#[cfg(target_arch = "wasm32")]
pub use wasm::bind;
