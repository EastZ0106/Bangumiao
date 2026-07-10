use crate::AppState;
use tauri::State;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub download_dir: String,
    pub refresh_interval: i32,
    pub aria2_port: i32,
    pub max_concurrent_downloads: i32,
    #[serde(default = "default_auto_delete")]
    pub auto_delete_torrent: bool,
    #[serde(default = "default_close_to_tray")]
    pub close_to_tray: bool,
}

fn default_auto_delete() -> bool {
    true
}
fn default_close_to_tray() -> bool {
    true
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let app = state.inner();
    Ok(AppSettings {
        download_dir: app.db.get_setting("download_dir").unwrap_or_default(),
        refresh_interval: app
            .db
            .get_setting("refresh_interval")
            .unwrap_or("30".into())
            .parse()
            .unwrap_or(30),
        aria2_port: app
            .db
            .get_setting("aria2_port")
            .unwrap_or("6800".into())
            .parse()
            .unwrap_or(6800),
        max_concurrent_downloads: app
            .db
            .get_setting("max_concurrent_downloads")
            .unwrap_or("3".into())
            .parse()
            .unwrap_or(3),
        auto_delete_torrent: app
            .db
            .get_setting("auto_delete_torrent")
            .unwrap_or("true".into())
            == "true",
        close_to_tray: app.db.get_setting("close_to_tray").unwrap_or("true".into()) == "true",
    })
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    let app = state.inner();
    let refresh_interval = settings.refresh_interval.clamp(1, 1440);
    let aria2_port = settings.aria2_port.clamp(1024, 65535) as u16;
    let max_concurrent_downloads = settings.max_concurrent_downloads.clamp(1, 10) as u16;

    let old_download_dir = app
        .base_download_dir
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    let old_port = app.aria2.lock().map_err(|e| e.to_string())?.port();
    let old_max = app
        .db
        .get_setting("max_concurrent_downloads")
        .unwrap_or("3".into())
        .parse::<u16>()
        .unwrap_or(3);

    app.db
        .set_setting("download_dir", &settings.download_dir)
        .map_err(|e| e.to_string())?;

    let new_download_dir = if !settings.download_dir.is_empty() {
        std::path::PathBuf::from(&settings.download_dir)
    } else {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("download")
    };
    std::fs::create_dir_all(&new_download_dir).ok();
    *app.base_download_dir.lock().map_err(|e| e.to_string())? = new_download_dir.clone();

    app.db
        .set_setting("refresh_interval", &refresh_interval.to_string())
        .map_err(|e| e.to_string())?;
    app.db
        .set_setting("aria2_port", &aria2_port.to_string())
        .map_err(|e| e.to_string())?;
    app.db
        .set_setting(
            "max_concurrent_downloads",
            &max_concurrent_downloads.to_string(),
        )
        .map_err(|e| e.to_string())?;
    app.db
        .set_setting(
            "auto_delete_torrent",
            if settings.auto_delete_torrent {
                "true"
            } else {
                "false"
            },
        )
        .map_err(|e| e.to_string())?;
    app.db
        .set_setting(
            "close_to_tray",
            if settings.close_to_tray {
                "true"
            } else {
                "false"
            },
        )
        .map_err(|e| e.to_string())?;

    if old_port != aria2_port
        || old_download_dir != new_download_dir
        || old_max != max_concurrent_downloads
    {
        let mut aria2 = app.aria2.lock().map_err(|e| e.to_string())?;
        aria2.set_port(aria2_port);
        aria2.start_with_config(
            &app.aria2_path.to_string_lossy(),
            &new_download_dir.to_string_lossy(),
            &app.app_dir.to_string_lossy(),
            max_concurrent_downloads,
        )?;
    }

    Ok(())
}
