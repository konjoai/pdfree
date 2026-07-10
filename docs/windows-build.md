# Windows Build (PDFree Desktop)

The Windows app is the **Tauri desktop shell** (`apps/desktop`) — a native
Rust window that hosts the same React UI as the web app (`apps/web`). It talks
to `pdfree-core` directly (no WASM); PDFium ships as a bundled `pdfium.dll`.

## Getting the installer (no Windows machine needed)

CI builds the installer on every push/PR via the **"Windows desktop installer
(Tauri)"** job in `.github/workflows/ci.yml`:

1. Open the workflow run on GitHub → **Actions** tab.
2. Download the **`pdfree-windows-x64-installer`** artifact.
3. It contains an `.msi` (WiX) and an `.exe` (NSIS) installer — run either.

The bundled `pdfium.dll` is placed in the app's resource dir, and the app sets
`PDFIUM_DYNAMIC_LIB_PATH` to that dir at startup (`src-tauri/src/lib.rs`), so
PDFium is found automatically after install.

> Target: **x64 (`x86_64-pc-windows-msvc`)**. ARM64 is not built yet.

## Building locally on Windows (for development)

Prerequisites:

- [Rust](https://rustup.rs/) (MSVC toolchain — the rustup default on Windows)
- [Node.js](https://nodejs.org/) 20+
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- WebView2 runtime (preinstalled on Windows 10/11; the installer fetches it if
  missing)
- Tauri CLI: `npm install -g @tauri-apps/cli@^2`

Then, from a Git Bash / MSYS shell at the repo root:

```bash
# 1. Fetch pdfium.dll into vendor/pdfium/
scripts/fetch-pdfium.sh win-x64

# 2. Build the web frontend (Tauri serves apps/web/dist)
cd apps/web && npm install && npm run build && cd ../..

# 3a. Run the app in dev (hot-reload frontend)
cd apps/desktop && tauri dev

# 3b. …or build a distributable installer
cd apps/desktop && tauri build
# installers land in target/release/bundle/{msi,nsis}/
```

In `tauri dev`, PDFium is discovered from the workspace `vendor/pdfium/` dir; in
a packaged build it comes from the bundled resource (see above).

## Key files

- `apps/desktop/src-tauri/tauri.conf.json` — base Tauri config.
- `apps/desktop/src-tauri/tauri.windows.conf.json` — Windows overrides; bundles
  `pdfium.dll` as a resource (auto-merged when building for Windows).
- `apps/desktop/src-tauri/src/lib.rs` — sets `PDFIUM_DYNAMIC_LIB_PATH` from the
  resource dir at startup.
- `apps/desktop/src-tauri/src/commands.rs` — Tauri commands wrapping
  `pdfree-core`.
