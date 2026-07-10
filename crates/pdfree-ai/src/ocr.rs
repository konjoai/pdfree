//! OCR with LLM cleanup (Phase 5).
//!
//! Tesseract (or Apple Vision on Apple platforms) reads scans; an LLM repairs
//! garbled characters and restores formatting.

use crate::{AiError, Result};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A process-unique temp file path — no crate-wide temp-file collisions
/// across concurrent `recognize()` calls.
fn temp_path(suffix: &str) -> PathBuf {
    let n = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("pdfree-ocr-{}-{n}-{suffix}", std::process::id()))
}

/// Extract text from a scanned page image via the `tesseract` CLI.
///
/// Shells out rather than binding `tesseract-sys` — the binary is already a
/// platform dependency users may or may not have, and shelling out keeps
/// this crate's own dependency tree free of a C toolchain requirement.
pub fn recognize(page_png: &[u8]) -> Result<String> {
    let input_path = temp_path("input.png");
    let output_base = temp_path("output");
    let output_path = output_base.with_extension("txt");

    let mut file = std::fs::File::create(&input_path)
        .map_err(|e| AiError::Provider(format!("failed to write OCR input file: {e}")))?;
    file.write_all(page_png)
        .map_err(|e| AiError::Provider(format!("failed to write OCR input file: {e}")))?;
    drop(file);

    let result = Command::new("tesseract")
        .arg(&input_path)
        .arg(&output_base)
        .output();

    let cleanup = || {
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);
    };

    let output = match result {
        Ok(output) => output,
        Err(e) => {
            cleanup();
            return Err(AiError::Provider(format!(
                "failed to run `tesseract` (is it installed? `brew install tesseract`): {e}"
            )));
        }
    };

    if !output.status.success() {
        cleanup();
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AiError::Provider(format!("tesseract failed: {stderr}")));
    }

    let text = std::fs::read_to_string(&output_path)
        .map_err(|e| AiError::Provider(format!("failed to read OCR output: {e}")));
    cleanup();

    Ok(text?.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small blank-white PNG, generated in-memory so the bytes are
    /// guaranteed well-formed. Just needs to be valid enough for
    /// `tesseract` to accept and process — the recognized text (empty, for
    /// a blank page) isn't the point, successfully round-tripping through
    /// the CLI is.
    fn blank_page_png() -> Vec<u8> {
        let img = image::RgbImage::from_pixel(64, 64, image::Rgb([255, 255, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encoding an in-memory PNG cannot fail");
        bytes
    }

    /// Real subprocess call to `tesseract`. Skipped (not failed) when the
    /// binary isn't on PATH — mirrors pdfree-core's `skip_without_pdfium!()`
    /// pattern for an environment-dependent external tool.
    #[test]
    fn recognize_round_trips_through_real_tesseract() {
        match recognize(&blank_page_png()) {
            Ok(_text) => {}
            Err(AiError::Provider(msg)) if msg.contains("is it installed?") => {
                eprintln!("skipping: tesseract not installed ({msg})");
            }
            Err(e) => panic!("unexpected OCR error: {e}"),
        }
    }

    #[test]
    fn temp_paths_are_unique_across_calls() {
        let a = temp_path("x");
        let b = temp_path("x");
        assert_ne!(a, b);
    }
}
