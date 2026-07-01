//! Local `uniffi-bindgen` entry point (library mode) — no global install
//! needed. Run `cargo run -p pdfree-ffi --bin uniffi-bindgen -- generate
//! --library <path-to-libpdfree_ffi.dylib> --language swift --out-dir <dir>`.

fn main() {
    uniffi::uniffi_bindgen_main()
}
