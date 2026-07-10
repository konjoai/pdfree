// Loads and wires together the two WASM modules PDFree's *browser* engine
// needs. Under Tauri, none of this runs — the native Rust backend binds
// PDFium directly via pdfree_core::pdfium::bind() (the same vendor-dir /
// system-library search already used by the macOS app), so there is no JS-
// side PDFium module to load at all. See `./runtime.ts` for the Tauri
// detection this file and `./engine.ts` share.
//
// Browser path:
// 1. The PDFium WASM build itself (Emscripten output — `pdfium.js` defines
//    a global `PDFiumModule()` factory; see public/pdfium/README.md for
//    where the actual binary comes from — it is not bundled with this repo).
// 2. Our own Rust `pdfree-wasm` module (generated into `src/wasm/` by
//    scripts/build-wasm.sh, also not committed — see that directory's
//    .gitignore entry).
//
// The sequence below (call `PDFiumModule()`, call our module's default
// init export, then call `initialize_pdfium_render(pdfiumModule,
// rustModule, debug)`) matches pdfium-render's own documented example
// (github.com/ajrcarey/pdfium-render, examples/index.html) — that
// three-argument call is exported automatically by pdfium-render's WASM
// bindings backend, not something pdfree-wasm defines itself.

import { isTauri } from "./runtime";
import initRustModule, { initialize_pdfium_render } from "../wasm/pdfree_wasm";

declare global {
  interface Window {
    /** Defined by the classic (non-module) <script src="/pdfium/pdfium.js">
     * tag in index.html — see public/pdfium/README.md. */
    PDFiumModule?: () => Promise<unknown>;
  }
}

let readyPromise: Promise<void> | null = null;

/**
 * Loads both WASM modules and binds them together (browser only — a no-op
 * under Tauri, see module docs above). Safe to call more than once — later
 * calls reuse the first attempt's in-flight/settled promise.
 */
export function ensurePdfiumReady(): Promise<void> {
  if (isTauri()) return Promise.resolve();
  if (readyPromise) return readyPromise;

  readyPromise = (async () => {
    if (typeof window.PDFiumModule !== "function") {
      throw new Error(
        "PDFium WASM module not found — see public/pdfium/README.md. " +
          "Expected a global PDFiumModule() from a <script src=\"/pdfium/pdfium.js\"> tag.",
      );
    }

    const [pdfiumModule, rustModule] = await Promise.all([
      window.PDFiumModule(),
      initRustModule(),
    ]);

    const ok = initialize_pdfium_render(pdfiumModule, rustModule, false);
    if (!ok) {
      throw new Error("pdfium-render's initialize_pdfium_render() returned false");
    }
  })();

  return readyPromise;
}
