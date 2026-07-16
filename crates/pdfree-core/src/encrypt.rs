//! PDF password protection on export.
//!
//! **`PDFium` has no encryption capability at all — confirmed, not assumed.**
//! `pdfium-render` 0.8.37's `Document::save_to_bytes`/`save_to_writer`
//! hardcode `flags = 0` with no password parameter anywhere in the call, and
//! reading `PDFium`'s own raw C API confirms every single `password`
//! parameter across the entire binding (`FPDF_LoadDocument`,
//! `FPDF_LoadMemDocument`, `FPDF_LoadCustomDocument`, `FPDFAvail_GetDocument`)
//! is for *opening* an already-encrypted PDF, never for *creating* one.
//! `PDFium` is a rendering/reading engine; encryption authoring was never in
//! its scope, on purpose — this isn't a `pdfium-render` binding gap to
//! revisit later, it's a real capability the underlying engine doesn't have.
//!
//! [`encrypt_with_password`] instead shells out to
//! [`qpdf`](https://qpdf.sourceforge.io/), the mature, widely-packaged CLI
//! tool for PDF encryption — the same "CLI tool already a platform
//! dependency, not bundled" tradeoff `pdfree-ai`'s `ocr.rs` already made for
//! `tesseract`. Unlike `PDFium`, `qpdf` is *not* bundled/vendored the way
//! `docs/pdfium-bundling.md` bundles `PDFium` itself — fetching and
//! packaging a real per-platform `qpdf` binary is its own scoped exercise,
//! out of scope for this pass — so this returns a clear, actionable error
//! rather than a confusing failure when `qpdf` isn't on `PATH`.

use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{PdfError, Result};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(suffix: &str) -> std::path::PathBuf {
    let n = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "pdfree-encrypt-{}-{n}-{suffix}",
        std::process::id()
    ))
}

/// Password-protect a PDF, returning the encrypted bytes. `owner_password`
/// defaults to `user_password` when not given — `qpdf` requires both, and
/// using the same value for both covers the common "just require a password
/// to open this file" case (an owner password only matters for restricting
/// print/copy/edit permissions on an otherwise-openable file, which this
/// function doesn't expose separately from the open password yet).
///
/// Uses `qpdf`'s strongest encryption (256-bit AES) unconditionally — no
/// weak-crypto opt-out is exposed, since there's no reason for a new PDF to
/// choose a weaker cipher today.
///
/// # Errors
///
/// Returns [`PdfError::InvalidPassword`] if `user_password` is empty,
/// [`PdfError::ToolNotFound`] if `qpdf` isn't on `PATH`, and
/// [`PdfError::EncryptionFailed`] if `qpdf` itself reports a failure (e.g.
/// `pdf_bytes` isn't a valid PDF).
pub fn encrypt_with_password(
    pdf_bytes: &[u8],
    user_password: &str,
    owner_password: Option<&str>,
) -> Result<Vec<u8>> {
    if user_password.is_empty() {
        return Err(PdfError::InvalidPassword(
            "user_password must not be empty".to_string(),
        ));
    }
    let owner_password = owner_password.unwrap_or(user_password);

    let input_path = temp_path("input.pdf");
    let output_path = temp_path("output.pdf");

    let mut file = std::fs::File::create(&input_path)?;
    file.write_all(pdf_bytes)?;
    drop(file);

    let cleanup = || {
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);
    };

    let result = Command::new("qpdf")
        .arg("--encrypt")
        .arg(user_password)
        .arg(owner_password)
        .arg("256")
        .arg("--")
        .arg(&input_path)
        .arg(&output_path)
        .output();

    let output = match result {
        Ok(output) => output,
        Err(_) => {
            cleanup();
            return Err(PdfError::ToolNotFound("qpdf"));
        }
    };

    if !output.status.success() {
        cleanup();
        return Err(PdfError::EncryptionFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let bytes = std::fs::read(&output_path);
    cleanup();
    Ok(bytes?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qpdf_available() -> bool {
        Command::new("qpdf")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    macro_rules! skip_without_qpdf {
        () => {
            if !qpdf_available() {
                eprintln!("skipping: qpdf not found on PATH — install it to enable this test");
                return;
            }
        };
    }

    #[test]
    fn rejects_an_empty_password_without_shelling_out() {
        let err = encrypt_with_password(b"not even a real pdf", "", None)
            .expect_err("empty password must be rejected");
        assert!(matches!(err, PdfError::InvalidPassword(_)), "got {err:?}");
    }

    #[test]
    fn reports_encryption_failure_for_invalid_pdf_bytes() {
        skip_without_qpdf!();

        let err = encrypt_with_password(b"not a pdf at all", "secret", None)
            .expect_err("garbage bytes are not a valid PDF");
        assert!(matches!(err, PdfError::EncryptionFailed(_)), "got {err:?}");
    }
}
