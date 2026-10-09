use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    window.show().map_err(|error| error.to_string())?;

    #[cfg(target_os = "linux")]
    crate::linux_fix::nudge_main_window(window);

    #[cfg(not(target_os = "linux"))]
    let _ = window.set_focus();

    Ok(())
}
