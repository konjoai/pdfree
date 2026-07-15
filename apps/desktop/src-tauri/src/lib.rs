mod commands;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Point pdfree-core's PDFium loader at the bundled library. In a
            // packaged build the platform library (pdfium.dll / libpdfium.so /
            // libpdfium.dylib) ships as a Tauri resource next to the app's
            // resources, not in a `vendor/pdfium/` dir the loader searches by
            // default (see tauri.windows.conf.json). Set the discovery env var
            // to the resource directory before any command binds PDFium.
            //
            // `bind()` treats a directory value as "look for the platform
            // library name inside it", so this works on every OS. We skip it
            // when the developer already set the var (e.g. `cargo tauri dev`
            // pointed at the workspace `vendor/pdfium/`); and because `bind()`
            // continues past a candidate that fails to load, pointing it at a
            // dev resource dir with no library still falls back cleanly to the
            // vendor/system search.
            if std::env::var_os("PDFIUM_DYNAMIC_LIB_PATH").is_none() {
                if let Ok(resource_dir) = app.path().resource_dir() {
                    std::env::set_var("PDFIUM_DYNAMIC_LIB_PATH", resource_dir);
                }
            }
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::document_info,
            commands::page_size,
            commands::render_page,
            commands::fit_to_page_dpi,
            commands::form_fields,
            commands::overlay_text,
            commands::boxes_on_page,
            commands::fillable_fields,
            commands::place_signature_with_audit,
            commands::merge_documents,
            commands::rotate_page,
            commands::extract_pages,
            commands::from_image,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
