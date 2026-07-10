use crate::filename;
use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;

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
const MAX_LIBRARY_DEPTH: usize = 64;

#[tauri::command]
pub fn scan_library(state: State<'_, AppState>) -> Result<Vec<AnimeGroup>, String> {
    let app = state.inner();
    let dir = app
        .base_download_dir
        .lock()
        .map_err(|e| e.to_string())?
        .clone();

    if !dir.exists() {
        return Ok(vec![]);
    }

    let video_files = find_video_files(&dir, 0)?;
    let scanned_files: Vec<_> = video_files
        .into_iter()
        .map(|file_path| {
            let file_name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let parsed = filename::parse(&file_name);
            let path_str = file_path.to_string_lossy().to_string();
            (file_path, file_name, parsed, path_str)
        })
        .collect();

    let mut conn = app.db.conn.lock().map_err(|e| e.to_string())?;

    // Get watch records from DB
    let mut watch_map: HashMap<String, bool> = HashMap::new();
    let mut watch_stmt = conn
        .prepare("SELECT file_path, watched FROM watch_records")
        .map_err(|e| e.to_string())?;
    let rows = watch_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get::<_, i32>(1)? != 0)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<(String, bool)>, _>>()
        .map_err(|e| e.to_string())?;
    drop(watch_stmt);
    for (path, watched) in rows {
        watch_map.insert(path, watched);
    }

    // Parse filenames and group by anime title
    // Strategy: first try to group by parent folder, then fall back to parsed title
    let mut groups: HashMap<String, Vec<EpisodeItem>> = HashMap::new();

    // Build a map: file_path -> subscription_title from the episodes table.
    // This lets us group episodes by their RSS subscription's display title,
    // which is always consistent regardless of filename format.
    let mut sub_title_map: HashMap<String, String> = HashMap::new();
    let mut stmt = conn
        .prepare(
            "SELECT e.file_path, s.title FROM episodes e
             LEFT JOIN subscriptions s ON e.subscription_id = s.id
             WHERE e.file_path != '' AND e.subscription_id != 'manual'",
        )
        .map_err(|e| e.to_string())?;
    let subscription_rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<(String, String)>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    for (path, title) in subscription_rows {
        sub_title_map.insert(path, title);
    }

    let transaction = conn.transaction().map_err(|e| e.to_string())?;
    for (file_path, file_name, parsed, path_str) in scanned_files {
        let watched = watch_map.get(&path_str).copied().unwrap_or(false);

        // Ensure DB record exists
        transaction
            .execute(
            "INSERT OR IGNORE INTO watch_records (id, file_path, anime_title, episode_number, watched)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                path_str,
                parsed.anime_title,
                parsed.episode_number,
                if watched { 1 } else { 0 },
            ],
            )
            .map_err(|e| e.to_string())?;

        let item = EpisodeItem {
            file_path: path_str.clone(),
            episode_number: parsed.episode_number,
            episode_title: file_name.clone(),
            downloaded: true,
            watched,
            file_name,
        };

        // Determine group key:
        // 1. If we have a subscription_title from the episodes table, use that
        // 2. Otherwise, use the parent folder name
        // 3. Fall back to the parsed anime title
        let title = if let Some(sub_title) = sub_title_map.get(&path_str) {
            sub_title.clone()
        } else {
            let parent_folder = file_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("未分类");
            if parent_folder != "." && parent_folder != "download" && parent_folder != "手动下载"
            {
                parent_folder.to_string()
            } else if parsed.anime_title.is_empty() {
                "未分类".to_string()
            } else {
                parsed.anime_title.clone()
            }
        };

        groups.entry(title).or_default().push(item);
    }
    transaction.commit().map_err(|e| e.to_string())?;

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

fn find_video_files(dir: &PathBuf, depth: usize) -> Result<Vec<PathBuf>, String> {
    if depth > MAX_LIBRARY_DEPTH {
        return Err(format!(
            "library directory nesting exceeds {} levels near {}",
            MAX_LIBRARY_DEPTH,
            dir.display()
        ));
    }
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() {
            continue;
        }

        if path.is_dir() {
            files.extend(find_video_files(&path, depth + 1)?);
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
pub fn mark_watched(state: State<'_, AppState>, file_path: String) -> Result<(), String> {
    let app = state.inner();
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE watch_records SET watched = 1, watched_at = ?1 WHERE file_path = ?2",
        rusqlite::params![now, file_path],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_file(file_path: String) -> Result<(), String> {
    open::that(&file_path).map_err(|e| format!("Failed to open file: {}", e))
}

#[tauri::command]
pub fn open_file_dir(file_path: String) -> Result<(), String> {
    let dir = std::path::Path::new(&file_path)
        .parent()
        .map(|p| p.to_path_buf())
        .or_else(|| {
            // Fallback: if it's already a directory or no parent
            let p = std::path::Path::new(&file_path);
            if p.is_dir() {
                Some(p.to_path_buf())
            } else {
                None
            }
        })
        .ok_or_else(|| format!("Cannot determine parent directory of: {}", file_path))?;
    open::that(&dir).map_err(|e| format!("Failed to open dir: {}", e))
}
