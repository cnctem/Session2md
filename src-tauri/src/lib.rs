mod commands;
mod session_manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_sessions,
            commands::get_session_messages,
            commands::export_session_markdown,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Session2md");
}
