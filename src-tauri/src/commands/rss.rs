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
) -> Result<Subscription, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    app.db
        .insert_subscription(&id, &title, &rss_url, &mikan_url, "")
        .map_err(|e| e.to_string())?;

    Ok(Subscription {
        id,
        title,
        rss_url,
        mikan_url,
        cover_url: String::new(),
        enabled: true,
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
        .filter(|(id, _, rss_url)| *id != "manual" && !rss_url.starts_with("manual://"))
        .collect();
    let mut known_titles = app.db.get_all_episode_titles().map_err(|e| e.to_string())?;

    let mut total_new = 0u32;
    let mut total_started = 0u32;

    for sub in subs {
        let (_sub_id, sub_title, rss_url) = (&sub.0, &sub.1, &sub.2);

        // Ensure per-subs