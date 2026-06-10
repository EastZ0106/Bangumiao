#[allow(dead_code)]
pub mod aria2;
#[allow(dead_code)]
pub mod commands;
#[allow(dead_code)]
pub mod db;
#[allow(dead_code)]
pub mod filename;
#[allow(dead_code)]
pub mod rss_parser;
#[allow(dead_code)]
pub mod scheduler;

use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub base_download_dir: PathBuf,
    pub db: db::Database,
    pub app_dir: PathBuf,
    pub aria2: Mutex<aria2::Aria2Manager>,
}

impl AppState {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let app_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bangumiao");
        let db = db::Database::new(&app_dir)?;

        // On startup, run a checkpoint + vacuum to clean any stale WAL state
        {
            let conn = db.conn.lock().unwrap();
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA optimize;").ok();
        }

        // Resolve aria2c sidecar path
        // In dev mode, look relative to src-tauri/binaries/
        // In bundle mode, Tauri puts it next to the exe
        let aria2_path = {
            let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("binaries")
                .join("aria2c-x86_64-pc-windows-msvc.exe");
            if dev_path.exists() {
                dev_path
            } else {
                let exe = std::env::current_exe().unwrap_or_default();
                exe.parent()
                    .map(|p| p.join("aria2c-x86_64-pc-windows-msvc.exe"))
                    .unwrap_or_else(|| PathBuf::from("aria2c-x86_64-pc-windows-msvc.exe"))
            }
        };

        // Ensure download dir exists AND aria2 uses it
        let base_download_dir = resolve_download_dir(&db);
        std::fs::create_dir_all(&base_download_dir).ok();

        let port = db.get_setting("aria2_port")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(6800u16);

        let mut aria2 = aria2::Aria2Manager::new(port);
        match aria2.start(&aria2_path.to_string_lossy(), &base_download_dir.to_string_lossy()) {
            Ok(()) => println!("[bangumiao] aria2 started on port {}", port),
            Err(e) => {
                eprintln!("[bangumiao] Failed to start aria2: {}", e);
                eprintln!("[bangumiao] Download features will be limited");
            }
        }

        Ok(AppState { base_download_dir, db, app_dir, aria2: Mutex::new(aria2) })
    }
}

fn resolve_download_dir(db: &db::Database) -> PathBuf {
    let custom = db.get_setting("download_dir")
        .ok()
        .filter(|d| !d.is_empty());
    match custom {
        Some(d) => PathBuf::from(&d),
        None => default_download_dir(),
    }
}

fn default_download_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is always src-tauri/ — go up one level to project root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("download")
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState::new().expect("Failed to initialize app state");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(app_state))
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::rss::get_subscriptions,
            commands::rss::add_subscription,
            commands::rss::remove_subscription,
            commands::rss::toggle_subscription,
            commands::rss::refresh_all_subscriptions,
            commands::rss::wipe_all_data,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::download::get_downloads,
            commands::download::sync_downloads,
            commands::download::pause_download,
            commands::download::resume_download,
            commands::download::remove_download,
            commands::download::add_torrent_download,
            commands::download::clean_download_dir,
            commands::library::scan_library,
            commands::library::mark_watched,
            commands::mikan::open_mikan_browser,
            commands::mikan::close_mikan_browser,
            commands::mikan::update_mikan_browser_bounds,
            commands::mikan::mikan_eval,
            commands::mikan::scan_mikan_rss,
            commands::mikan::fetch_mikan_rss,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
