//! Acceptance tests: password-protect a PDF via `qpdf`, then confirm the
//! result actually requires the password to open (and the wrong password
//! doesn't work) using `pdfree-core`'s own `PDFium`-backed open path.
//!
//! Like `tests/render.rs`, these skip with a notice (rather than fail) when
//! `PDFium` isn't bundled or `qpdf` isn't installed, so a bare checkout
//! still builds green.
//!
//! Test code may `unwrap`/`expect` freely (see `.github/copilot-instructions.md`)
//! — the production-code ban only applies to `pdfree-core`'s library surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::encrypt::encrypt_with_password;
use pdfree_core::error::PdfError;
use pdfree_core::Document;

const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");

fn pdfium_available() -> bool {
    pdfree_core::pdfium::bind().is_ok()
}

fn qpdf_available() -> bool {
    std::process::Command::new("qpdf")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

macro_rules! skip_without_tools {
    () => {
        if !pdfium_available() {
            eprintln!(
                "skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable"
            );
            return;
        }
        if !qpdf_available() {
            eprintln!("skipping: qpdf not found on PATH — install it to enable this test");
            return;
        }
    };
}

#[test]
fn a_password_protected_pdf_cannot_be_opened_without_the_password() {
    skip_without_tools!();

    // Confirm the source opens fine unencrypted first — isolates whether a
    // later failure is really about the password, not a broken fixture.
    Document::from_bytes(SAMPLE.to_vec(), None).expect("sample.pdf opens with no password");

    let encrypted = encrypt_with_password(SAMPLE, "correct-horse", None).expect("encrypt");
    assert_ne!(encrypted, SAMPLE, "encryption must change the bytes");

    let err = Document::from_bytes(encrypted, None)
        .expect_err("an encrypted PDF must not open with no password");
    assert!(matches!(err, PdfError::Pdfium(_)), "got {err:?}");
}

#[test]
fn a_password_protected_pdf_rejects_the_wrong_password() {
    skip_without_tools!();

    let encrypted = encrypt_with_password(SAMPLE, "correct-horse", None).expect("encrypt");

    let err = Document::from_bytes(encrypted, Some("wrong-password"))
        .expect_err("the wrong password must not open it");
    assert!(matches!(err, PdfError::Pdfium(_)), "got {err:?}");
}

#[test]
fn a_password_protected_pdf_opens_and_renders_with_the_correct_password() {
    skip_without_tools!();

    let encrypted = encrypt_with_password(SAMPLE, "correct-horse", None).expect("encrypt");

    let doc = Document::from_bytes(encrypted.clone(), Some("correct-horse"))
        .expect("the correct password must open it");
    assert_eq!(
        doc.page_count(),
        2,
        "same page count as the unencrypted source"
    );

    // Prove the encrypted bytes are genuinely renderable when unlocked with
    // the right password — not just that *some* open call succeeds. Goes
    // straight through `pdfium-render` rather than `Document::render_page`/
    // `renderer::render_page_to_png`: those (like every other function in
    // this crate keyed only on `pdf_bytes: &[u8]`) don't accept a password
    // parameter at all, a separate, real, pre-existing gap unrelated to
    // *creating* password protection — `Document::from_bytes` validates a
    // password at open time but never carries it forward, so nothing
    // downstream can act on an already-open encrypted document. Confirmed
    // and flagged separately (see the spawned follow-up task) rather than
    // silently worked around here.
    let pdfium = pdfree_core::pdfium::bind().expect("pdfium available (checked by the macro)");
    let live = pdfium
        .load_pdf_from_byte_slice(&encrypted, Some("correct-horse"))
        .expect("pdfium-render can open it directly with the password");
    let page = live.pages().get(0).expect("page 0");
    let rendered = page
        .render_with_config(
            &pdfium_render::prelude::PdfRenderConfig::new().scale_page_by_factor(1.0),
        )
        .expect("render the decrypted page");
    assert!(rendered.width() > 0 && rendered.height() > 0);
}

#[test]
fn a_distinct_owner_password_still_lets_the_user_password_open_it() {
    skip_without_tools!();

    let encrypted = encrypt_with_password(SAMPLE, "open-me", Some("owner-only")).expect("encrypt");

    Document::from_bytes(encrypted, Some("open-me"))
        .expect("the user password must open the file regardless of the owner password");
}
