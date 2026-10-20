mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_project,
            commands::startup_file,
            commands::parse_maa,
            commands::save_project,
            commands::save_text,
            commands::calculate,
            commands::sweep,
            commands::far_field,
            commands::near_field,
            commands::edit_elements,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the MMANA application");
}
