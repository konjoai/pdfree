# Handoff: Phase 4 — macOS/iOS SwiftUI shell (needs a Mac)

**For:** whoever picks this up next on an actual Mac (Wes, or a fresh Claude
Code session with Xcode/Swift toolchain access). This session ran in a
Linux-only sandbox with no Xcode, no Swift toolchain, and no Apple SDKs — so
Phase 4's native-app work stopped at the boundary of what's buildable and
testable from here. Nothing macOS/iOS-specific has been started on disk; this
is a from-scratch briefing, not a partially-done branch.

## Where the repo stands

`main` (`cd17510`) has Phases 0–3 of `pdfree-core` complete and merged: open/
render, forms, signatures, annotations, editor (font-preserving text
replace), pages (merge/split/rotate/extract/reorder), and convert
(text-extraction, image→PDF). See `docs/api.md` for the full surface and
`CLAUDE.md` for the phase plan. Two deliberate gaps remain
(`signatures::sign_with_certificate`, `convert::to_docx`/`from_docx`) — both
`PdfError::NotImplemented`, not missing engineering; see `CLAUDE.md`'s open
questions.

Phase 4 is "Platform Shells." Its macOS/iOS checklist, verbatim from
`CLAUDE.md`:

```
- [ ] Wire UniFFI codegen for pdfree-ffi (UDL already frozen)
- [ ] macOS SwiftUI app wrapping pdfree-ffi via UniFFI
- [ ] iOS app (shared SwiftUI views from macOS)
```

(The other two Phase 4 items — Web/WASM and Tauri — are a separate track,
buildable/testable on Linux; not this handoff's concern.)

## Current state of `pdfree-ffi`

`crates/pdfree-ffi/` exists but is **structurally ready, mechanically
untouched** — no `uniffi` crate dependency anywhere, no `build.rs`, no
scaffolding has ever actually been generated. Concretely:

- `Cargo.toml`: `crate-type = ["lib", "cdylib", "staticlib"]`, depends on
  `pdfree-core` + `thiserror`. No `uniffi` dependency (neither runtime nor
  `[build-dependencies]`).
- `src/pdfree.udl`: the frozen interface contract — but it only covers
  Phase 0's API surface: `version()`, and `PdfDocument` with
  `from_bytes`/`page_count`/`title`/`author`/`render_page`. Nothing from
  Phases 1–3 (forms, signatures, annotations, editor, pages, convert) is in
  the UDL yet.
- `src/lib.rs`: a plain Rust wrapper implementing exactly that UDL surface
  (`PdfDocument::from_bytes`/`page_count`/`title`/`author`/`render_page`,
  plus a `PdfFreeError` enum mirroring the UDL's `[Error] enum
  PdfFreeError`). Doc comments in this file explicitly say codegen is
  "Phase 4" and call it "a mechanical step because the API here already
  matches the UDL one-for-one" — true for what's there, but what's there is
  only Phase 0.
- `scripts/build-macos.sh` already stubs the intended pipeline: cross-compile
  `pdfree-ffi` for `aarch64-apple-darwin` + `x86_64-apple-darwin`, `lipo`
  them into a universal dylib, then (per its own trailing echo, not yet
  actually run):
  ```
  uniffi-bindgen generate crates/pdfree-ffi/src/pdfree.udl \
    --language swift --out-dir apps/macos/Sources/Bridge/
  ```
- `apps/` does not exist at all yet — no Xcode project, no SwiftUI sources.

## Two real decisions before wiring codegen

Neither of these is mechanical; both affect the shape of the Swift API the
macOS/iOS apps will actually code against, so decide them before generating
anything, not after.

### 1. UDL file vs. proc-macro UniFFI

The existing comments (`lib.rs`, `pdfree.udl`) assume the classic **UDL
file** workflow: `uniffi::include_scaffolding!("pdfree")` in `lib.rs`, paired
with the hand-maintained `.udl` file, `uniffi-bindgen` reading the `.udl` to
emit Swift/Kotlin. That's UniFFI's original approach and it still works.

Modern UniFFI (0.28+) also supports **proc-macro mode**
(`#[uniffi::export]` on the Rust fns/impls directly, no `.udl` file, no
manual interface duplication) — the interface is derived from the Rust code
itself, so it can't drift out of sync with `lib.rs` the way a hand-maintained
`.udl` can. Given the UDL here is already stale relative to `pdfree-core`
(only mirrors Phase 0, needs Phases 1–3 added by hand either way), it's
worth deciding now whether to:
- (a) keep the UDL file and manually extend it to cover forms/signatures/
  annotations/editor/pages/convert, or
- (b) migrate to proc-macros and let the interface generate itself from an
  expanded `lib.rs`.

Check the pinned `uniffi` version's docs/changelog for current guidance —
this project has never pinned a `uniffi` version, so whichever gets added
first should be a deliberate, recent pin, not whatever `cargo add uniffi`
happens to resolve to today.

### 2. Expand the FFI surface before or after codegen plumbing?

`pdfree-ffi` today only exposes open + render. A macOS app built against the
current UDL can display PDFs and read metadata — nothing else. Filling
forms, signing, annotating, editing text, and merging/splitting/converting
are all live in `pdfree-core` (Phases 1–3) but **not reachable through
`pdfree-ffi` at all yet**. Recommend: get the codegen pipeline working
end-to-end against the current tiny surface first (proves the mechanism
before investing in a bigger surface), then expand `pdfree-ffi`'s API to
match `pdfree-core`'s Phase 1–3 modules, mirroring the types in
`docs/api.md` (`FieldKind`/`FillValue`, `TextOverlay`, `SignaturePlacement`,
`Annotation`/`AnnotationKind`, `TextRun`, `Rotation`, etc.) rather than
inventing a parallel FFI-specific vocabulary.

## What's actually verifiable without a Mac (do this part first, anywhere)

None of this needs Xcode — it only needs the Rust toolchain and
`uniffi-bindgen`, both available on Linux:

1. Add a pinned `uniffi` dependency to `crates/pdfree-ffi/Cargo.toml`
   (`[dependencies]` for the runtime macros/scaffolding, `[build-dependencies]`
   if using `uniffi::generate_scaffolding` from a `build.rs` for UDL mode).
2. Wire whichever mode was chosen above (`include_scaffolding!` + `build.rs`
   calling `uniffi::generate_scaffolding("src/pdfree.udl")`, or
   `#[uniffi::export]` proc-macros directly).
3. `cargo build -p pdfree-ffi` on the host target (`x86_64-unknown-linux-gnu`
   or whatever this sandbox resolves to) — confirms the scaffolding compiles
   at all before touching cross-compilation.
4. Run the `uniffi-bindgen generate ... --language swift` step from
   `scripts/build-macos.sh` and confirm it emits well-formed Swift source
   (readable, no generation errors) — this validates the UDL↔bindgen
   plumbing without needing to actually compile the Swift output.
5. Add a test/CI step for this (`cargo build -p pdfree-ffi` at minimum;
   ideally `uniffi-bindgen generate` in CI too, gated so it doesn't require
   a Mac runner) so this doesn't silently rot the way the plain-wrapper
   version did.

Steps 1–4 are safe to do in this Linux sandbox in a follow-up session if
useful — they'd get real signal on whether the codegen plumbing itself
works, before ever touching a Mac.

## What genuinely needs a Mac

- Cross-compiling `pdfree-ffi` for `aarch64-apple-darwin` and
  `x86_64-apple-darwin` for real (needs the Apple SDKs / Xcode command line
  tools as the linker backend — `rustup target add` alone isn't enough).
- Running `scripts/build-macos.sh` for real and confirming the `lipo`'d
  universal dylib actually loads.
- Creating the actual Xcode project / SwiftUI app under `apps/macos/`,
  wiring the generated Swift bindings in as a local Swift package or direct
  source drop (`apps/macos/Sources/Bridge/`, per `build-macos.sh`'s planned
  `--out-dir`).
- Bundling PDFium for macOS per `docs/pdfium-bundling.md`'s macOS section —
  read that doc before assuming the dylib ships the same way `pdfree-ffi`
  does; PDFium is a separate, larger native dependency loaded at runtime.
- Building the actual SwiftUI views (open/render first, matching what
  `pdfree-ffi` currently exposes; expand as the FFI surface grows) and
  running the app in Simulator/on-device to confirm rendering, scrolling,
  and (once wired) form-fill/sign/annotate/edit flows actually work — this
  repo's stated quality bar throughout Phases 0–3 has been "test against
  real PDFs, not synthetic ones" (IRS 1040, real contracts); keep that bar
  for the UI layer too, not just the engine.
- iOS is explicitly "shared SwiftUI views from macOS" per `CLAUDE.md` — do
  macOS first, then iOS should mostly be view-modifier/target-membership
  work rather than a rewrite, assuming the views are built with both
  platforms in mind (avoid macOS-only APIs like certain `NSViewRepresentable`
  bridges without an iOS equivalent already sketched).

## Pointers

- `docs/architecture.md` — crate table lists `pdfree-ffi` as "Wrapper live;
  codegen in Phase 4," and has the full data-flow diagram for how a render
  call crosses the FFI boundary.
- `docs/api.md` — the Rust-side API Phase 4's expanded FFI surface should
  mirror.
- `docs/pdfium-bundling.md` — required reading before assuming how PDFium
  ships on macOS; `pdfree-ffi`'s dylib and PDFium's dylib are two separate
  runtime-loaded native dependencies.
- `scripts/build-macos.sh` — already encodes the intended build pipeline;
  treat it as the source of truth for the target triples and the
  `uniffi-bindgen` invocation shape, updating it as the real pipeline is
  built out rather than starting over.
- `CLAUDE.md` — Phase 4 checklist and the "Claude Code Instructions"
  section's standing rules (pure-Rust core, no watermarks/limits/silent
  uploads, BUSL-1.1 license) apply to the Swift shell too, not just Rust.

## Unrelated repo state worth knowing (not blocking, just context)

Two independent, in-progress items live on `main` right now; neither affects
Phase 4 and neither should distract from it, but they'll show up if you look
at CI:

- `.github/workflows/konjo-gates.yml`'s `gates` job has
  `continue-on-error: true` and `KIBAN_REF` pinned to `v1.1.4`, mid-way
  through verifying whether that release fixes a `repo:clippy`
  output-parsing bug in the `kiban` quality-gate tool (unrelated to this
  repo's actual code quality — see the TODO comments in that file and in
  `.konjo/profile.yml` for the full history). `pdfree`'s real quality
  enforcement comes from `.github/workflows/konjo-gate.yml` ("Wall 2"),
  which is green and does not depend on `kiban`.
- `cargo-mutants` found 21 real `MISSED`-mutant findings in `pdfree-core`
  (test-coverage gaps across `annotations.rs`, `document.rs`, `lib.rs`,
  `pdfium.rs`, `renderer.rs`, `signatures.rs`) — a legitimate, still-open
  test-writing task, tracked via the same `gates` job output, independent of
  Phase 4.
