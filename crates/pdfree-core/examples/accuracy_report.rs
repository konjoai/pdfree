//! E2E accuracy harness — Phase 1 of the e2e suite (see `docs/` / CI).
//!
//! Runs every PDF in the test corpus (`scripts/fetch-test-corpus.sh` →
//! `tests/corpus/`) through every `pdfree-core` feature and grades the result:
//!
//! - **Detection** (informational): how many fillable boxes the scanner finds
//!   on page 1, how many AcroForm fields, how many signature fields. Also
//!   renders page 1 with every detected box outlined, to a PNG you can eyeball
//!   — this is the "grade on accuracy, I'll review the results" artifact.
//! - **Round-trips** (graded): fill, overlay text, sign, rotate, merge,
//!   split/extract, and text extraction each run against every real form and
//!   assert their invariant held. A *hard* check failing exits non-zero (CI
//!   fails); a *soft* check (fill, text extraction — legitimately N/A on some
//!   forms) is reported but never fails the build.
//!
//! Output: `target/accuracy-report/report.md` + one `<form>_overlay.png` per
//! form. Override paths with `PDFREE_CORPUS_DIR` / `PDFREE_REPORT_DIR`.
//!
//! Run: `cargo run -p pdfree-core --example accuracy_report`
//! (needs PDFium — `scripts/fetch-pdfium.sh` — and a fetched corpus).

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use pdfree_core::boxes::{self, DetectedBox};
use pdfree_core::forms::{self, FieldKind, FillValue, SignatureFieldKind, TextOverlay};
use pdfree_core::pages::{self, Rotation};
use pdfree_core::renderer::{self, RenderOptions};
use pdfree_core::{convert, Document};

const OVERLAY_DPI: f32 = 120.0;
const SIGNATURE_PNG: &[u8] = include_bytes!("../tests/fixtures/signature.png");

struct Check {
    name: &'static str,
    hard: bool,
    ok: bool,
    detail: String,
}

struct FormResult {
    name: String,
    pages: u16,
    fields: usize,
    signature_fields: usize,
    boxes_p0: usize,
    overlay_png: Option<String>,
    checks: Vec<Check>,
    load_error: Option<String>,
}

fn main() {
    // A bare checkout / no-PDFium environment shouldn't hard-fail the harness.
    if pdfree_core::pdfium::bind().is_err() {
        eprintln!("PDFium not found — run scripts/fetch-pdfium.sh. Skipping accuracy report.");
        return;
    }

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let corpus_dir = std::env::var_os("PDFREE_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("tests/corpus"));
    let report_dir = std::env::var_os("PDFREE_REPORT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target/accuracy-report"));
    fs::create_dir_all(&report_dir).expect("create report dir");

    let mut pdfs: Vec<PathBuf> = fs::read_dir(&corpus_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "pdf"))
        .collect();
    pdfs.sort();

    if pdfs.is_empty() {
        eprintln!(
            "No corpus PDFs in {} — run scripts/fetch-test-corpus.sh first.",
            corpus_dir.display()
        );
        return;
    }

    let results: Vec<FormResult> = pdfs.iter().map(|p| grade_one(p, &report_dir)).collect();

    let report = render_report(&results);
    let report_path = report_dir.join("report.md");
    fs::write(&report_path, &report).expect("write report");

    // Console summary + exit code.
    let mut hard_failures = 0;
    for r in &results {
        for c in &r.checks {
            if c.hard && !c.ok {
                hard_failures += 1;
            }
        }
    }
    println!("\n{report}");
    println!("Report + overlays written to {}", report_dir.display());
    if hard_failures > 0 {
        eprintln!("\n{hard_failures} hard check(s) failed.");
        std::process::exit(1);
    }
}

fn grade_one(path: &Path, report_dir: &Path) -> FormResult {
    let name = path.file_stem().unwrap().to_string_lossy().into_owned();
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => return load_failed(&name, format!("read failed: {e}")),
    };

    let doc = match Document::from_bytes(bytes.clone(), None) {
        Ok(d) => d,
        Err(e) => return load_failed(&name, format!("open failed: {e}")),
    };
    let pages = doc.page_count();

    let fields = forms::fields(&bytes).unwrap_or_default();
    let signature_fields = fields
        .iter()
        .filter(|f| f.signature_kind != SignatureFieldKind::None)
        .count();
    let boxes = boxes::boxes_on_page(&bytes, 0).unwrap_or_default();

    let overlay_png = render_overlay(&bytes, &boxes, &name, report_dir)
        .map_err(|e| eprintln!("  overlay render failed for {name}: {e}"))
        .ok();

    let checks = vec![
        check_render(&bytes),
        check_overlay_text(&bytes),
        check_sign(&bytes),
        check_rotate(&bytes),
        check_merge(&bytes, pages),
        check_extract(&bytes),
        check_to_text(&bytes),
        check_fill(&bytes, &fields),
    ];

    FormResult {
        name,
        pages,
        fields: fields.len(),
        signature_fields,
        boxes_p0: boxes.len(),
        overlay_png,
        checks,
        load_error: None,
    }
}

fn load_failed(name: &str, msg: String) -> FormResult {
    FormResult {
        name: name.to_string(),
        pages: 0,
        fields: 0,
        signature_fields: 0,
        boxes_p0: 0,
        overlay_png: None,
        checks: vec![Check {
            name: "open",
            hard: true,
            ok: false,
            detail: msg.clone(),
        }],
        load_error: Some(msg),
    }
}

// MARK: - Checks

fn ok(name: &'static str, hard: bool, detail: impl Into<String>) -> Check {
    Check {
        name,
        hard,
        ok: true,
        detail: detail.into(),
    }
}

fn fail(name: &'static str, hard: bool, detail: impl Into<String>) -> Check {
    Check {
        name,
        hard,
        ok: false,
        detail: detail.into(),
    }
}

fn check_render(bytes: &[u8]) -> Check {
    match renderer::render_page_to_png(bytes, 0, &RenderOptions::with_dpi(96.0)) {
        Ok(png) if !png.is_empty() => ok("render page 1", true, format!("{} bytes", png.len())),
        Ok(_) => fail("render page 1", true, "empty PNG"),
        Err(e) => fail("render page 1", true, e.to_string()),
    }
}

fn check_overlay_text(bytes: &[u8]) -> Check {
    let marker = "PDFREEOVERLAYCHK";
    let overlay = TextOverlay {
        page: 0,
        x: 72.0,
        y: 72.0,
        text: marker.to_string(),
        font_size: 12.0,
    };
    match forms::overlay_text(bytes, &[overlay]) {
        Ok(new_bytes) => match convert::to_text(&new_bytes) {
            Ok(text) if text.contains(marker) => {
                ok("overlay text round-trip", true, "stamped + re-extracted")
            }
            Ok(_) => fail(
                "overlay text round-trip",
                true,
                "marker not found after stamp",
            ),
            Err(e) => fail("overlay text round-trip", true, format!("re-extract: {e}")),
        },
        Err(e) => fail("overlay text round-trip", true, e.to_string()),
    }
}

fn check_sign(bytes: &[u8]) -> Check {
    let placement = pdfree_core::signatures::SignaturePlacement {
        page: 0,
        x: 100.0,
        y: 100.0,
        width: 140.0,
        height: 46.0,
    };
    match pdfree_core::signatures::place_signature(bytes, SIGNATURE_PNG, placement) {
        Ok(signed) => match Document::from_bytes(signed, None)
            .and_then(|d| d.render_page(0, &RenderOptions::with_dpi(72.0)))
        {
            Ok(_) => ok("sign (place image)", true, "placed + renders"),
            Err(e) => fail("sign (place image)", true, format!("re-render: {e}")),
        },
        Err(e) => fail("sign (place image)", true, e.to_string()),
    }
}

fn check_rotate(bytes: &[u8]) -> Check {
    let before = match renderer::render_page_to_png(bytes, 0, &RenderOptions::with_dpi(72.0)) {
        Ok(p) => p,
        Err(e) => return fail("rotate", true, e.to_string()),
    };
    match pages::rotate(bytes, 0, Rotation::Clockwise90) {
        Ok(rotated) => {
            match renderer::render_page_to_png(&rotated, 0, &RenderOptions::with_dpi(72.0)) {
                Ok(after) if after != before => ok("rotate", true, "render changed"),
                Ok(_) => fail("rotate", true, "render unchanged after 90°"),
                Err(e) => fail("rotate", true, e.to_string()),
            }
        }
        Err(e) => fail("rotate", true, e.to_string()),
    }
}

fn check_merge(bytes: &[u8], pages: u16) -> Check {
    match pages::merge(&[bytes.to_vec(), bytes.to_vec()]) {
        Ok(merged) => match Document::from_bytes(merged, None) {
            Ok(d) if d.page_count() == pages.saturating_mul(2) => {
                ok("merge", true, format!("{} → {} pages", pages, pages * 2))
            }
            Ok(d) => fail(
                "merge",
                true,
                format!("expected {} pages, got {}", pages * 2, d.page_count()),
            ),
            Err(e) => fail("merge", true, e.to_string()),
        },
        Err(e) => fail("merge", true, e.to_string()),
    }
}

fn check_extract(bytes: &[u8]) -> Check {
    match pages::extract(bytes, &[0]) {
        Ok(extracted) => match Document::from_bytes(extracted, None) {
            Ok(d) if d.page_count() == 1 => ok("extract page 1", true, "1 page"),
            Ok(d) => fail(
                "extract page 1",
                true,
                format!("got {} pages", d.page_count()),
            ),
            Err(e) => fail("extract page 1", true, e.to_string()),
        },
        Err(e) => fail("extract page 1", true, e.to_string()),
    }
}

fn check_to_text(bytes: &[u8]) -> Check {
    // Soft: an image-only scan would legitimately have no extractable text.
    match convert::to_text(bytes) {
        Ok(t) if t.trim().len() > 20 => {
            ok("extract text", false, format!("{} chars", t.trim().len()))
        }
        Ok(t) => fail(
            "extract text",
            false,
            format!("only {} chars (image-only?)", t.trim().len()),
        ),
        Err(e) => fail("extract text", false, e.to_string()),
    }
}

fn check_fill(bytes: &[u8], fields: &[forms::FormField]) -> Check {
    // Soft: only meaningful when a writable text field exists; many flat
    // forms have none, which is N/A, not a failure.
    let Some(field) = fields
        .iter()
        .find(|f| f.kind == FieldKind::Text && f.signature_kind == SignatureFieldKind::None)
    else {
        return ok("fill text field", false, "n/a — no text field");
    };
    let marker = "PDFREEFILL";
    match forms::fill(
        bytes,
        &[(field.name.clone(), FillValue::Text(marker.to_string()))],
    ) {
        Ok(filled) => match forms::fields(&filled) {
            Ok(after) => {
                let persisted = after
                    .iter()
                    .find(|f| f.name == field.name)
                    .and_then(|f| f.value.as_deref())
                    .is_some_and(|v| v.contains(marker));
                if persisted {
                    ok(
                        "fill text field",
                        false,
                        format!("'{}' persisted", short(&field.name)),
                    )
                } else {
                    fail(
                        "fill text field",
                        false,
                        format!("'{}' did not persist", short(&field.name)),
                    )
                }
            }
            Err(e) => fail("fill text field", false, format!("re-read: {e}")),
        },
        Err(e) => fail("fill text field", false, e.to_string()),
    }
}

fn short(name: &str) -> String {
    if name.len() <= 32 {
        name.to_string()
    } else {
        format!("…{}", &name[name.len() - 30..])
    }
}

// MARK: - Overlay rendering

fn render_overlay(
    bytes: &[u8],
    boxes: &[DetectedBox],
    name: &str,
    report_dir: &Path,
) -> Result<String, String> {
    let (_w_pts, h_pts) = renderer::page_size_points(bytes, 0).map_err(|e| e.to_string())?;
    let png = renderer::render_page_to_png(bytes, 0, &RenderOptions::with_dpi(OVERLAY_DPI))
        .map_err(|e| e.to_string())?;
    let mut img = image::load_from_memory(&png)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let scale = OVERLAY_DPI / 72.0;
    let green = image::Rgba([55, 192, 122, 255]);
    for b in boxes {
        let left = (b.x * scale) as i32;
        let right = ((b.x + b.width) * scale) as i32;
        // PDF y is bottom-up; image y is top-down.
        let top = ((h_pts - (b.y + b.height)) * scale) as i32;
        let bottom = ((h_pts - b.y) * scale) as i32;
        draw_rect_outline(&mut img, left, top, right, bottom, green);
    }
    let file = format!("{name}_overlay.png");
    img.save(report_dir.join(&file))
        .map_err(|e| e.to_string())?;
    Ok(file)
}

fn draw_rect_outline(
    img: &mut image::RgbaImage,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    color: image::Rgba<u8>,
) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let thickness = 2;
    let put = |img: &mut image::RgbaImage, x: i32, y: i32| {
        if x >= 0 && y >= 0 && x < w && y < h {
            img.put_pixel(x as u32, y as u32, color);
        }
    };
    for t in 0..thickness {
        for x in left..=right {
            put(img, x, top + t);
            put(img, x, bottom - t);
        }
        for y in top..=bottom {
            put(img, left + t, y);
            put(img, right - t, y);
        }
    }
}

// MARK: - Report

fn render_report(results: &[FormResult]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# PDFree accuracy report\n");
    let _ = writeln!(
        s,
        "Corpus of {} forms. Detection counts are informational; round-trip \
         checks are graded (a **hard** failure fails CI, a soft one is FYI).\n",
        results.len()
    );

    let _ = writeln!(
        s,
        "| Form | Pages | AcroForm fields | Sig fields | Boxes (p1) | Checks | Overlay |"
    );
    let _ = writeln!(s, "|---|--:|--:|--:|--:|:--|:--|");
    for r in results {
        let (pass, total) = (r.checks.iter().filter(|c| c.ok).count(), r.checks.len());
        let overlay = r
            .overlay_png
            .as_deref()
            .map(|f| format!("[png]({f})"))
            .unwrap_or_else(|| "—".into());
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {} | {}/{} | {} |",
            r.name, r.pages, r.fields, r.signature_fields, r.boxes_p0, pass, total, overlay
        );
    }

    let _ = writeln!(s, "\n## Per-form checks\n");
    for r in results {
        let _ = writeln!(s, "### {}", r.name);
        if let Some(err) = &r.load_error {
            let _ = writeln!(s, "- ❌ **failed to open**: {err}\n");
            continue;
        }
        for c in &r.checks {
            let icon = if c.ok {
                "✅"
            } else if c.hard {
                "❌"
            } else {
                "⚠️"
            };
            let tag = if c.hard { "" } else { " _(soft)_" };
            let _ = writeln!(s, "- {icon} **{}**{tag} — {}", c.name, c.detail);
        }
        let _ = writeln!(s);
    }

    let hard_fail: usize = results
        .iter()
        .flat_map(|r| &r.checks)
        .filter(|c| c.hard && !c.ok)
        .count();
    let soft_fail: usize = results
        .iter()
        .flat_map(|r| &r.checks)
        .filter(|c| !c.hard && !c.ok)
        .count();
    let _ = writeln!(
        s,
        "## Totals\n\n- Hard failures (fail CI): **{hard_fail}**\n- Soft failures (FYI): {soft_fail}"
    );
    s
}
