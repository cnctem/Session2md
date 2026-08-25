use crate::session2md_settings::{self, Session2mdSettings, Session2mdSettingsSnapshot};

#[tauri::command]
pub fn get_session2md_settings() -> Result<Session2mdSettingsSnapshot, String> {
    session2md_settings::get_settings_snapshot()
}

#[tauri::command]
pub fn save_session2md_settings(
    settings: Session2mdSettings,
) -> Result<Session2mdSettingsSnapshot, String> {
    session2md_settings::save_settings(settings)
}
