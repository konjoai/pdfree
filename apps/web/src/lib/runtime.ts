/** True when this build is running inside the Tauri desktop shell rather
 * than a plain browser tab. `__TAURI_INTERNALS__` is the marker Tauri v2's
 * webview injects before any page script runs — checking for it (rather
 * than a bundler-time env flag) means the exact same built `apps/web/dist`
 * output works unmodified in both the browser and the Tauri window, which
 * is the whole point of "Tauri reuses the web UI" (CLAUDE.md's Phase 4
 * checklist). */
export function isTauri(): boolean {
  return typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== "undefined";
}
