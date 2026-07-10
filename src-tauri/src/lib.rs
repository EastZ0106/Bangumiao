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
use tauri::{Manager, RunEvent, WindowEvent};

pub struct AppState {
    pub base_download_dir: Mutex<PathBuf>,
    pub db: db::Database,
    pub app_dir: PathBuf,
    pub aria2_path: PathBuf,
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
            let conn = db.conn.lock().map_err(|_| {
                std::io::Error::other("database connection lock poisoned during startup")
            })?;
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA optimize;")
                .ok();
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
                let exe = std::env::current_exe().map_err(|e| {
                    std::io::Error::other(format!("cannot locate application executable: {e}"))
                })?;
                exe.parent()
                    .map(|p| p.join("aria2c-x86_64-pc-windows-msvc.exe"))
                    .ok_or_else(|| {
                        std::io::Error::other("application executable has no parent directory")
                    })?
            }
        };

        // Ensure download dir exists AND aria2 uses it
        let base_download_dir = resolve_download_dir(&db);
        std::fs::create_dir_all(&base_download_dir).ok();

        let port = db
            .get_setting("aria2_port")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(6800u16);
        let max_concurrent_downloads = db
            .get_setting("max_concurrent_downloads")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3u16);

        let mut aria2 = aria2::Aria2Manager::new(port);
        match aria2.start_with_config(
            &aria2_path.to_string_lossy(),
            &base_download_dir.to_string_lossy(),
            &app_dir.to_string_lossy(),
            max_concurrent_downloads,
        ) {
            Ok(()) => {
                // Wait for aria2 to be ready
                let mut ready = false;
                for _ in 0..40 {
                    if aria2::Aria2Manager::port_is_open(port) {
                        ready = true;
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                if ready {
                    println!("[bangumiao] aria2 started on port {}", port);
                } else {
                    eprintln!(
                        "[bangumiao] aria2 process started but port {} not open after 10s",
                        port
                    );
                }
            }
            Err(e) => {
                eprintln!("[bangumiao] Failed to start aria2: {}", e);
                eprintln!("[bangumiao] Download features will be limited");
            }
        }

        Ok(AppState {
            base_download_dir: Mutex::new(base_download_dir),
            db,
            app_dir,
            aria2_path,
            aria2: Mutex::new(aria2),
        })
    }
}

fn resolve_download_dir(db: &db::Database) -> PathBuf {
    let custom = db
        .get_setting("download_dir")
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState::new().expect("Failed to initialize app state");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .setup(|app| {
            // Start background RSS refresh scheduler
            let handle = app.handle().clone();
            crate::scheduler::start_scheduler(handle);

            let tray_menu = tauri::menu::MenuBuilder::new(app)
                .text("show", "打开窗口")
                .separator()
                .text("quit", "退出")
                .build()?;

            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| Box::<dyn std::error::Error>::from("missing icon"))?;
            let _tray = tauri::tray::TrayIconBuilder::new()
                .icon(icon)
                .tooltip("🐱 bangumiao")
                .menu(&tray_menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::rss::get_subscriptions,
            commands::rss::add_subscription,
            commands::rss::remove_subscription,
            commands::rss::toggle_subscription,
            commands::rss::refresh_all_subscriptions,
            commands::rss::update_auto_download,
            commands::rss::wipe_all_data,
            commands::download::start_download,
            commands::download::batch_start_downloads,
            commands::download::get_pending_episodes,
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
            commands::library::open_file,
            commands::library::open_file_dir,
            commands::feedback::send_feedback,
            commands::mikan::open_mikan_browser,
            commands::mikan::close_mikan_browser,
            commands::mikan::update_mikan_browser_bounds,
            commands::mikan::mikan_eval,
            commands::mikan::scan_mikan_rss,
            commands::mikan::fetch_mikan_rss,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            let RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } = event
            else {
                return;
            };
            if label != "main" {
                return;
            }

            let close_to_tray = app_handle
                .state::<AppState>()
                .db
                .get_setting("close_to_tray")
                .ok()
                .map(|value| value == "true")
                .unwrap_or(true);

            if close_to_tray {
                api.prevent_close();
                if let Some(w) = app_handle.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
        });
}
