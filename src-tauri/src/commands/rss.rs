use std::sync::Mutex;
use tauri::State;

use crate::rss_parser;
use crate::AppState;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct Subscription {
    pub id: String,
    pub title: String,
    pub rss_url: String,
    pub mikan_url: String,
    pub cover_url: String,
    pub enabled: bool,
    pub auto_download: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct RefreshResult {
    pub new_episodes: u32,
    pub started_downloads: u32,
}

/// Wipe all subscriptions and episodes from the database.
/// Keeps settings and watch_records intact.
/// Also re-creates the database file fresh to eliminate any WAL/corruption issues.
#[tauri::command]
pub fn wipe_all_data(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    // Take a strong lock and delete everything
    {
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let ep_count: i32 = conn.query_row("SELECT COUNT(*) FROM episodes", [], |r| r.get(0)).unwrap_or(0);
        let sub_count: i32 = conn.query_row("SELECT COUNT(*) FROM subscriptions WHERE id != 'manual'", [], |r| r.get(0)).unwrap_or(0);
        conn.execute("DELETE FROM episodes", []).map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM subscriptions WHERE id != 'manual'", []).map_err(|e| e.to_string())?;
        // Run VACUUM to rebuild the database file cleanly
        conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", []).ok();
        conn.execute("VACUUM", []).ok();
        Ok(format!("Cleared {} episodes and {} subscriptions", ep_count, sub_count))
    }
}

#[tauri::command]
pub fn get_subscriptions(state: State<'_, Mutex<AppState>>) -> Result<Vec<Subscription>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    app.db.get_subscriptions().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_subscription(
    state: State<'_, Mutex<AppState>>,
    rss_url: String,
    title: String,
    mikan_url: String,
    auto_download: bool,
) -> Result<Subscription, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    app.db
        .insert_subscription(&id, &title, &rss_url, &mikan_url, "", auto_download)
        .map_err(|e| e.to_string())?;

    Ok(Subscription {
        id,
        title,
        rss_url,
        mikan_url,
        cover_url: String::new(),
        enabled: true,
        auto_download,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub fn remove_subscription(state: State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    app.db.remove_subscription(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_subscription(state: State<'_, Mutex<AppState>>, id: String) -> Result<bool, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    app.db.toggle_subscription(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn refresh_all_subscriptions(state: State<'_, Mutex<AppState>>) -> Result<RefreshResult, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let subs: Vec<_> = app.db.get_enabled_subscriptions()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|(id, _, rss_url, _)| *id != "manual" && !rss_url.starts_with("manual://"))
        .collect();
    let mut known_titles = app.db.get_all_episode_titles().map_err(|e| e.to_string())?;

    let mut total_new = 0u32;
    let mut total_started = 0u32;

    for sub in subs {
        let (_sub_id, sub_title, rss_url, auto_download) = (&sub.0, &sub.1, &sub.2, sub.3);

        // Ensure per-subscription sub-directory exists
        let base_dir = app.base_download_dir.clone();
        let safe_name = sanitize_dir_name(sub_title);
        let sub_dir = base_dir.join(&safe_name);
        std::fs::create_dir_all(&sub_dir).ok();

        let xml = reqwest::blocking::get(rss_url)
            .and_then(|r| r.text())
            .map_err(|e| format!("Failed to fetch RSS: {}", e))?;

        let feed = rss_parser::parse_rss(&xml).map_err(|e| format!("RSS parse error: {}", e))?;

        let new_eps = rss_parser::extract_new_episodes(&feed, &known_titles);

        for ep in new_eps {
            total_new += 1;

            // Scope: keep aria2 lock short so we don't deadlock with DB
            let gid: Option<String> = if auto_download {
                let aria2 = app.aria2.lock().map_err(|e| e.to_string())?;
                if !ep.torrent_url.is_empty() || !ep.magnet_uri.is_empty() {
                    let result = if !ep.torrent_url.is_empty() {
                        aria2.add_torrent_with_dir(&ep.torrent_url, &sub_dir.to_string_lossy())
                    } else {
                        aria2.add_uri_with_dir(&ep.magnet_uri, &sub_dir.to_string_lossy())
                    };
                    match result {
                        Ok(g) => {
                            if !g.is_empty() { total_started += 1; }
                            Some(g)
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                }
            } else {
                None
            };

            // Insert episode into DB
            let _id = app
                .db
                .insert_episode(
                    &sub.0,
                    &ep.title,
                    ep.episode_number,
                    &ep.torrent_url,
                    &ep.magnet_uri,
                    &ep.pub_date,
                    gid.as_deref(),
                )
                .map_err(|e| e.to_string())?;

            known_titles.insert(ep.title.clone());
        }
    }

    Ok(RefreshResult { new_episodes: total_new, started_downloads: total_started })
}

#[tauri::command]
pub fn update_auto_download(
    state: State<'_, Mutex<AppState>>,
    id: String,
    auto_download: bool,
) -> Result<(), String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    app.db.update_auto_download(&id, auto_download)
        .map_err(|e| e.to_string())
}

pub(crate) fn sanitize_dir_name(name: &str) -> String {
    let mut s = String::new();
    for ch in name.chars() {
        let c = ch as u32;
        // Windows-illegal chars: \ / : * ? " < > |
        match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => s.push('_'),
            _ if c < 0x20 => s.push('_'),
            _ => s.push(ch),
        }
    }
    let trimmed = s.trim().trim_matches('.');
    if trimmed.is_empty() { "untitled".into() } else { trimmed.to_string() }
}
