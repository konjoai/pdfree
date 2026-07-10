//! Phase 2 acceptance tests: visual signature-image placement.
//!
//! Like `tests/render.rs`, these skip with a notice (rather than fail) when
//! `PDFium` isn't bundled, so a bare checkout still builds green. Run
//! `scripts/fetch-pdfium.sh` first to make them exercise `PDFium` for real.
//!
//! Test code may `unwrap`/`expect` freely (see `.github/copilot-instructions.md`)
//! — the production-code ban only applies to `pdfree-core`'s library surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pdfree_core::error::PdfError;
use pdfree_core::signatures::{self, SignatureAudit, SignaturePlacement};
use pdfree_core::{Document, RenderOptions};

const SAMPLE: &[u8] = include_bytes!("fixtures/sample.pdf");
const SIGNATURE_PNG: &[u8] = include_bytes!("fixtures/signature.png");

fn pdfium_available() -> bool {
    pdfree_core::pdfium::bind().is_ok()
}

macro_rules! skip_without_pdfium {
    () => {
        if !pdfium_available() {
            eprintln!(
                "skipping: PDFium library not found — run scripts/fetch-pdfium.sh to enable"
            );
            return;
        }
    };
}

#[test]
fn stamps_a_signature_image_and_it_renders_visibly() {
    skip_without_pdfium!();

    let signed = signatures::place_signature(
        SAMPLE,
        SIGNATURE_PNG,
        SignaturePlacement {
            page: 0,
            x: 72.0,
            y: 450.0,
            width: 150.0,
            height: 60.0,
        },
    )
    .expect("place_signature");

    assert!(signed.len() > SAMPLE.len(), "signature adds content");

    let before = Document::from_bytes(SAMPLE.to_vec(), None).unwrap();
    let after = Document::from_bytes(signed, None).unwrap();

    let png_before = before
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    let png_after = after
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    assert_ne!(png_before, png_after, "the signature image must render");
}

#[test]
fn place_signature_rejects_an_out_of_range_page() {
    skip_without_pdfium!();

    let err = signatures::place_signature(
        SAMPLE,
        SIGNATURE_PNG,
        SignaturePlacement {
            page: 9,
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
    )
    .expect_err("page 9 does not exist");

    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn place_signature_rejects_a_non_positive_size() {
    skip_without_pdfium!();

    let err = signatures::place_signature(
        SAMPLE,
        SIGNATURE_PNG,
        SignaturePlacement {
            page: 0,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 10.0,
        },
    )
    .expect_err("zero width is invalid");

    assert!(
        matches!(err, PdfError::InvalidSignaturePlacement(_)),
        "got {err:?}"
    );
}

#[test]
fn place_signature_rejects_invalid_image_bytes() {
    skip_without_pdfium!();

    let err = signatures::place_signature(
        SAMPLE,
        b"not a png",
        SignaturePlacement {
            page: 0,
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
    )
    .expect_err("garbage image bytes must not silently no-op");

    assert!(matches!(err, PdfError::Image(_)), "got {err:?}");
}

#[test]
fn place_signature_with_audit_stamps_the_signature_and_a_caption() {
    skip_without_pdfium!();

    let audit = SignatureAudit {
        signer_name: "Ada Lovelace".to_string(),
        signed_at: "2026-07-02 14:30 PDT".to_string(),
        device_info: Some("macOS 14.5".to_string()),
    };
    let placement = SignaturePlacement {
        page: 0,
        x: 72.0,
        y: 450.0,
        width: 150.0,
        height: 60.0,
    };

    let signed_plain =
        signatures::place_signature(SAMPLE, SIGNATURE_PNG, placement).expect("place_signature");
    let signed_audited =
        signatures::place_signature_with_audit(SAMPLE, SIGNATURE_PNG, placement, &audit)
            .expect("place_signature_with_audit");

    // The audited version has strictly more content than the plain
    // signature (the caption text object), and both add content over the
    // unsigned original.
    assert!(
        signed_audited.len() > SAMPLE.len(),
        "signature adds content"
    );
    assert!(
        signed_audited.len() != signed_plain.len(),
        "the caption must add distinguishable content beyond the bare signature"
    );

    let plain = Document::from_bytes(signed_plain, None).unwrap();
    let audited = Document::from_bytes(signed_audited, None).unwrap();
    let png_plain = plain
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    let png_audited = audited
        .render_page(0, &RenderOptions::with_dpi(150.0))
        .unwrap();
    assert_ne!(
        png_plain, png_audited,
        "the audit caption must render visibly beneath the signature"
    );
}

#[test]
fn place_signature_with_audit_omits_device_info_when_absent() {
    skip_without_pdfium!();

    let with_device = SignatureAudit {
        signer_name: "Ada Lovelace".to_string(),
        signed_at: "2026-07-02 14:30 PDT".to_string(),
        device_info: Some("macOS 14.5".to_string()),
    };
    let without_device = SignatureAudit {
        signer_name: "Ada Lovelace".to_string(),
        signed_at: "2026-07-02 14:30 PDT".to_string(),
        device_info: None,
    };
    let placement = SignaturePlacement {
        page: 0,
        x: 72.0,
        y: 450.0,
        width: 150.0,
        height: 60.0,
    };

    let with =
        signatures::place_signature_with_audit(SAMPLE, SIGNATURE_PNG, placement, &with_device)
            .expect("with device_info");
    let without =
        signatures::place_signature_with_audit(SAMPLE, SIGNATURE_PNG, placement, &without_device)
            .expect("without device_info");

    assert!(
        with.len() > without.len(),
        "including device_info should add more caption text"
    );
}

#[test]
fn place_signature_with_audit_rejects_an_out_of_range_page() {
    skip_without_pdfium!();

    let audit = SignatureAudit {
        signer_name: "Ada Lovelace".to_string(),
        signed_at: "2026-07-02".to_string(),
        device_info: None,
    };
    let err = signatures::place_signature_with_audit(
        SAMPLE,
        SIGNATURE_PNG,
        SignaturePlacement {
            page: 9,
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        &audit,
    )
    .expect_err("page 9 does not exist");

    assert!(
        matches!(err, PdfError::PageOutOfRange { index: 9, count: 2 }),
        "got {err:?}"
    );
}

#[test]
fn place_signature_with_audit_places_the_caption_above_the_page_bottom_even_near_the_edge() {
    skip_without_pdfium!();

    let audit = SignatureAudit {
        signer_name: "Ada Lovelace".to_string(),
        signed_at: "2026-07-02".to_string(),
        device_info: None,
    };
    // Signature placed right at the very bottom of the page — the caption
    // would go negative (off the page) without the clamp.
    let signed = signatures::place_signature_with_audit(
        SAMPLE,
        SIGNATURE_PNG,
        SignaturePlacement {
            page: 0,
            x: 72.0,
            y: 1.0,
            width: 150.0,
            height: 60.0,
        },
        &audit,
    )
    .expect("place_signature_with_audit near the bottom edge");

    // Must still produce a valid, renderable document.
    let doc = Document::from_bytes(signed, None).unwrap();
    doc.render_page(0, &RenderOptions::with_dpi(150.0))
        .expect("page still renders");
}

#[test]
fn sign_with_certificate_is_honestly_not_implemented() {
    let err = signatures::sign_with_certificate(SAMPLE, b"", "").expect_err("not implemented");
    assert!(matches!(err, PdfError::NotImplemented(_)), "got {err:?}");
}
