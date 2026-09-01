mod commands;
mod session2md_settings;
mod session_manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_sessions,
            commands::get_session_messages,
            commands::export_session_markdown,
            commands::delete_session,
            commands::delete_sessions,
            commands::get_session2md_settings,
            commands::save_session2md_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Session2md");
}
