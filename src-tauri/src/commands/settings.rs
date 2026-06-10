use std::sync::Mutex;
use tauri::State;
use crate::AppState;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub download_dir: String,
    pub refresh_interval: i32,
    pub aria2_port: i32,
    pub max_concurrent_downloads: i32,
    #[serde(default = "default_auto_delete")]
    pub auto_delete_torrent: bool,
}

fn default_auto_delete() -> bool { true }

#[tauri::command]
pub fn get_settings(state: State<'_, Mutex<AppState>>) -> Result<AppSettings, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    Ok(AppSettings {
        download_dir: app.db.get_setting("download_dir").unwrap_or_default(),
        refresh_interval: app.db.get_setting("refresh_interval").unwrap_or("30".into()).parse().unwrap_or(30),
        aria2_port: app.db.get_setting("aria2_port").unwrap_or("6800".into()).parse().unwrap_or(6800),
        max_concurrent_downloads: app.db.get_setting("max_concurrent_downloads").unwrap_or("3".into()).parse().unwrap_or(3),
        auto_delete_torrent: app.db.get_setting("auto_delete_torrent").unwrap_or("true".into()) == "true",
    })
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, Mutex<AppState>>,
    settings: AppSettings,
) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    app.db.set_setting("download_dir", &settings.download_dir).map_err(|e| e.to_string())?;
    // Sync base_download_dir with the new setting and ensure it exists
    if !settings.download_dir.is_empty() {
        app.base_download_dir = std::path::PathBuf::from(&settings.download_dir);
    } else {
        app.base_download_dir = app.app_dir.join("download");
    }
    std::fs::create_dir_all(&app.base_download_dir).ok();
    app.db.set_setting("refresh_interval", &settings.refresh_interval.to_string()).map_err(|e| e.to_string())?;
    app.db.set_setting("aria2_port", &settings.aria2_port.to_string()).map_err(|e| e.to_string())?;
    app.db.set_setting("max_concurrent_downloads", &settings.max_concurrent_downloads.to_string()).map_err(|e| e.to_string())?;
    app.db.set_setting("auto_delete_torrent", if settings.auto_delete_torrent { "true" } else { "false" }).map_err(|e| e.to_string())?;
    Ok(())
}
