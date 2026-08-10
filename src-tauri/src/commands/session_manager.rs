#![allow(non_snake_case)]

use crate::session_manager;
use std::path::PathBuf;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn list_sessions() -> Result<Vec<session_manager::SessionMeta>, String> {
    let sessions = tauri::async_runtime::spawn_blocking(session_manager::scan_sessions)
        .await
        .map_err(|e| format!("Failed to scan sessions: {e}"))?;
    Ok(sessions)
}

#[tauri::command]
pub async fn get_session_messages(
    providerId: String,
    sourcePath: String,
) -> Result<Vec<session_manager::SessionMessage>, String> {
    let provider_id = providerId.clone();
    let source_path = sourcePath.clone();
    tauri::async_runtime::spawn_blocking(move || {
        session_manager::load_messages(&provider_id, &source_path)
    })
    .await
    .map_err(|e| format!("Failed to load session messages: {e}"))?
}

#[tauri::command]
pub async fn export_session_markdown<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    defaultName: String,
    markdown: String,
) -> Result<Option<String>, String> {
    let destination = app
        .dialog()
        .file()
        .add_filter("Markdown", &["md"])
        .set_file_name(&defaultName)
        .blocking_save_file();

    let Some(destination) = destination else {
        return Ok(None);
    };

    let file_path = destination.to_string();
    let path = PathBuf::from(&file_path);
    tauri::async_runtime::spawn_blocking(move || std::fs::write(path, markdown))
        .await
        .map_err(|error| format!("Failed to export session: {error}"))?
        .map_err(|error| format!("Failed to write Markdown file: {error}"))?;

    Ok(Some(file_path))
}
