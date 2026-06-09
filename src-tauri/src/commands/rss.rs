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
pub fn refresh_all_subscriptions(state: State<'_, Mutex<AppState>>) -> Result<Vec<String>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let subs = app.db.get_enabled_subscriptions().map_err(|e| e.to_string())?;
    let known_titles = app.db.get_all_episode_titles().map_err(|e| e.to_string())?;

    let mut new_ids = Vec::new();

    for sub in subs {
        let xml = reqwest::blocking::get(&sub.2)
            .and_then(|r| r.text())
            .map_err(|e| format!("Failed to fetch RSS: {}", e))?;

        let feed = rss_parser::parse_rss(&xml).map_err(|e| format!("RSS parse error: {}", e))?;

        let new_eps = rss_parser::extract_new_episodes(&feed, &known_titles);

        for ep in new_eps {
            let gid: Option<String> = if !ep.magnet_uri.is_empty() || !ep.torrent_url.is_empty() {
                let aria2 = app.aria2.lock().map_err(|e| e.to_string())?;
                let result = if !ep.torrent_url.is_empty() {
                    aria2.add_torrent(&ep.torrent_url)
                } else {
                    aria2.add_uri(&ep.magnet_uri)
                };
                match result {
                    Ok(g) => {
                        // Update aria2 gid immediately
                        let _conn = app.db.conn.lock().map_err(|e| e.to_string())?;
                        Some(g)
                    }
                    Err(_) => None,
                }
            } else {
                None
            };

            let id = app
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
            new_ids.push(id);
        }
    }

    Ok(new_ids)
}
