use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;
use crate::AppState;
use crate::filename;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct AnimeGroup {
    pub title: String,
    pub episodes: Vec<EpisodeItem>,
}

#[derive(Debug, Serialize, Clone)]
pub struct EpisodeItem {
    pub file_path: String,
    pub episode_number: Option<f64>,
    pub episode_title: String,
    pub downloaded: bool,
    pub watched: bool,
    pub file_name: String,
}

const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "mov", "webm", "flv", "ts"];

#[tauri::command]
pub fn scan_library(state: State<'_, Mutex<AppState>>) -> Result<Vec<AnimeGroup>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let dir = app.base_download_dir.clone();

    if !dir.exists() {
        return Ok(vec![]);
    }

    // Scan for video files recursively
    let video_files = find_video_files(&dir)?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;

    // Get watch records from DB
    let mut watch_map: HashMap<String, bool> = HashMap::new();
    let mut watch_stmt = conn
        .prepare("SELECT file_path, watched FROM watch_records")
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, bool)> = watch_stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get::<_, i32>(1)? != 0))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    for (path, watched) in rows {
        watch_map.insert(path, watched);
    }

    // Parse filenames and group by anime title
    let mut groups: HashMap<String, Vec<EpisodeItem>> = HashMap::new();

    for file_path in &video_files {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let parsed = filename::parse(file_name);
        let path_str = file_path.to_string_lossy().to_string();

        let watched = watch_map.get(&path_str).copied().unwrap_or(false);

        // Ensure DB record exists
        let _ = conn.execute(
            "INSERT OR IGNORE INTO watch_records (id, file_path, anime_title, episode_number, watched)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                path_str,
                parsed.anime_title,
                parsed.episode_number,
                if watched { 1 } else { 0 },
            ],
        );

        let item = EpisodeItem {
            file_path: path_str.clone(),
            episode_number: parsed.episode_number,
            episode_title: file_name.to_string(),
            downloaded: true,
            watched,
            file_name: file_name.to_string(),
        };

        let title = if parsed.anime_title.is_empty() {
            "未分类".to_string()
        } else {
            parsed.anime_title.clone()
        };

        groups.entry(title).or_default().push(item);
    }

    // Sort episodes within each group
    let mut result: Vec<AnimeGroup> = groups
        .into_iter()
        .map(|(title, mut episodes)| {
            episodes.sort_by(|a, b| {
                a.episode_number
                    .partial_cmp(&b.episode_number)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            AnimeGroup { title, episodes }
        })
        .collect();

    result.sort_by(|a, b| a.title.cmp(&b.title));

    Ok(result)
}

fn find_video_files(dir: &PathBuf) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            if let Ok(sub_files) = find_video_files(&path) {
                files.extend(sub_files);
            }
        } else if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            if VIDEO_EXTENSIONS.contains(&ext_lower.as_str()) {
                files.push(path);
            }
        }
    }

    Ok(files)
}

#[tauri::command]
pub fn mark_watched(state: State<'_, Mutex<AppState>>, file_path: String) -> Result<(), String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE watch_records SET watched = 1, watched_at = ?1 WHERE file_path = ?2",
        rusqlite::params![now, file_path],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
