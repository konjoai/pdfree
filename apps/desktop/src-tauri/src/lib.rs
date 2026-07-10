mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
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
            commands::place_signature_with_audit,
            commands::merge_documents,
            commands::rotate_page,
            commands::extract_pages,
            commands::from_image,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
