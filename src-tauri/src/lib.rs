mod commands;
#[cfg(target_os = "linux")]
mod linux_fix;
mod session2md_settings;
mod session_manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(session_manager::search::SessionSearchState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_sessions,
            commands::get_session_messages,
            commands::search_sessions,
            commands::export_session_markdown,
            commands::open_external_url,
            commands::delete_session,
            commands::delete_sessions,
            commands::get_session2md_settings,
            commands::save_session2md_settings,
            commands::check_app_update,
            commands::show_main_window,
        ])
        .on_page_load(|webview, payload| {
            if webview.label() == "main"
                && payload.event() == tauri::webview::PageLoadEvent::Finished
            {
                let _ = webview.window().show();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Session2md");
}
