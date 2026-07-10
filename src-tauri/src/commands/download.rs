use crate::aria2::Aria2Manager;
use crate::AppState;
use serde::Serialize;
use tauri::State;

const STOPPED_QUERY_LIMIT: i32 = 100;
const DOWNLOAD_RETRY_ATTEMPTS: usize = 5;
const DOWNLOAD_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(500);
const REMOVE_CLEANUP_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Debug, Serialize, Clone)]
pub struct DownloadItem {
    pub id: String,
    pub episode_title: String,
    pub status: String,
    pub progress: f64,
    pub file_path: String,
    pub subscription_title: Option<String>,
    pub subscription_id: Option<String>,
    pub episode_number: Option<f64>,
    pub gid: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PendingGroup {
    pub subscription_title: String,
    pub subscription_id: String,
    pub episodes: Vec<DownloadItem>,
}

#[tauri::command]
pub fn get_downloads(state: State<'_, AppState>) -> Result<Vec<DownloadItem>, String> {
    let app = state.inner();
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT e.id, e.title, e.status, e.progress, e.file_path, e.gid, s.title, e.subscription_id, e.episode_number
         FROM episodes e
         LEFT JOIN subscriptions s ON e.subscription_id = s.id
         ORDER BY e.created_at DESC"
    ).map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DownloadItem {
                id: row.get::<_, String>(0)?,
                episode_title: row.get::<_, String>(1)?,
                status: row.get::<_, String>(2)?,
                progress: row.get::<_, f64>(3)?,
                file_path: row.get::<_, String>(4)?,
                gid: row.get::<_, String>(5)?,
                subscription_title: row.get::<_, Option<String>>(6)?,
                subscription_id: row.get::<_, Option<String>>(7)?,
                episode_number: row.get::<_, Option<f64>>(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sync_downloads(state: State<'_, AppState>) -> Result<String, String> {
    let mut log = String::new();

    let tmp_aria2 = state.aria2.lock().map_err(|e| e.to_string())?.rpc_client();

    if !Aria2Manager::port_is_open(tmp_aria2.port()) {
        return Ok("aria2 not reachable".into());
    }

    let active_result = tmp_aria2.tell_active();
    let stopped_result = tmp_aria2.tell_stopped(0, STOPPED_QUERY_LIMIT);

    let app = state.inner();
    let download_dir: String = app
        .base_download_dir
        .lock()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;

    if let Ok(ref active) = active_result {
        log.push_str(&format!("Active: {} tasks\n", active.len()));
        for d in active {
            let completed = d.completed_length.parse::<u64>().unwrap_or(0) as f64;
            let total = d.total_length.parse::<u64>().unwrap_or(1) as f64;
            let progress = if total > 0.0 { completed / total } else { 0.0 };
            let file_path = d.files.first().map(|f| f.path.clone()).unwrap_or_default();
            // Only update if user hasn't explicitly paused this episode
            let current: String = conn
                .query_row(
                    "SELECT status FROM episodes WHERE gid=?1",
                    rusqlite::params![d.gid],
                    |r| r.get(0),
                )
                .unwrap_or_default();
            if current == "paused" {
                continue;
            }
            conn.execute(
                "UPDATE episodes SET status='active', progress=?1, file_path=?2 WHERE gid=?3",
                rusqlite::params![progress, file_path, d.gid],
            )
            .ok();
        }
    }

    if let Ok(ref stopped) = stopped_result {
        log.push_str(&format!("Stopped: {} tasks\n", stopped.len()));
        for d in stopped {
            let file_path = d.files.first().map(|f| f.path.clone()).unwrap_or_default();
            if file_path.contains("[METADATA]") {
                continue;
            }

            // Map aria2 status → DB status.
            // "complete" → completed, "paused" → paused (user paused, do NOT overwrite),
            // "error" / "removed" → failed, everything else → leave alone.
            let new_status = match d.status.as_str() {
                "complete" => "completed",
                "paused" => "paused",
                "error" => "failed",
                "removed" => "failed",
                _ => "", // unknown — skip update
            };

            if !new_status.is_empty() {
                conn.execute(
                    "UPDATE episodes SET status=?1, file_path=?2 WHERE gid=?3",
                    rusqlite::params![new_status, file_path, d.gid],
                )
                .ok();
            }

            // Clean up orphaned .torrent / .torrent.aria2 files
            if !download_dir.is_empty() {
                let _ = std::fs::remove_file(format!("{}/{}.torrent", download_dir, d.gid));
                let _ = std::fs::remove_file(format!("{}/{}.torrent.aria2", download_dir, d.gid));
            }
        }
    }

    Ok(log)
}

#[tauri::command]
pub fn pause_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let app = state.inner();
    let gid: String = {
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT gid FROM episodes WHERE id=?1",
            rusqlite::params![&id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?
    };
    let aria2 = app.aria2.lock().map_err(|e| e.to_string())?.rpc_client();
    if !gid.is_empty() {
        if !Aria2Manager::port_is_open(aria2.port()) {
            return Err(format!("aria2 is not reachable on port {}", aria2.port()));
        }
        aria2.pause(&gid)?;
    }
    // Update DB — always, this is the source of truth for the UI
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE episodes SET status='paused' WHERE id=?1",
        rusqlite::params![&id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn resume_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let app = state.inner();
    let gid: String = {
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT gid FROM episodes WHERE id=?1",
            rusqlite::params![&id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?
    };
    let aria2 = app.aria2.lock().map_err(|e| e.to_string())?.rpc_client();
    if !gid.is_empty() {
        if !Aria2Manager::port_is_open(aria2.port()) {
            return Err(format!("aria2 is not reachable on port {}", aria2.port()));
        }
        aria2.unpause(&gid)?;
    }
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE episodes SET status='active' WHERE id=?1",
        rusqlite::params![&id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn remove_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let (gid, file_path, torrent_url, download_dir, auto_delete, aria2) = {
        let app = state.inner();
        let download_dir = app
            .base_download_dir
            .lock()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string();
        let auto_delete = app
            .db
            .get_setting("auto_delete_torrent")
            .unwrap_or("true".into())
            == "true";
        let aria2 = app.aria2.lock().map_err(|e| e.to_string())?.rpc_client();
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let (gid, file_path, torrent_url): (String, String, String) = conn
            .query_row(
                "SELECT gid, file_path, torrent_url FROM episodes WHERE id=?1",
                rusqlite::params![&id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM episodes WHERE id=?1", rusqlite::params![&id])
            .map_err(|e| e.to_string())?;
        (
            gid,
            file_path,
            torrent_url,
            download_dir,
            auto_delete,
            aria2,
        )
    };

    std::thread::spawn(move || {
        if !gid.is_empty() && Aria2Manager::port_is_open(aria2.port()) {
            let _ = aria2.force_remove(&gid);
            let _ = aria2.remove_download_result(&gid);
        }
        std::thread::sleep(REMOVE_CLEANUP_DELAY);
        if !file_path.is_empty() {
            remove_file_if_within_base(&file_path, &download_dir);
            let aria2_file = format!("{}.aria2", file_path);
            remove_file_if_within_base(&aria2_file, &download_dir);
        }
        if auto_delete && !torrent_url.is_empty() && torrent_url.ends_with(".torrent") {
            remove_file_if_within_base(&torrent_url, &download_dir);
        }
        if !download_dir.is_empty() && !gid.is_empty() {
            let _ = std::fs::remove_file(format!("{}/{}.torrent", download_dir, gid));
            let _ = std::fs::remove_file(format!("{}/{}.torrent.aria2", download_dir, gid));
        }
    });

    Ok(())
}

#[tauri::command]
pub fn add_torrent_download(
    state: State<'_, AppState>,
    torrent_url: String,
    title: String,
) -> Result<DownloadItem, String> {
    let app = state.inner();
    let aria2 = app.aria2.lock().map_err(|e| e.to_string())?.rpc_client();
    let download_dir = app
        .base_download_dir
        .lock()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    let manual_dir = format!("{}/手动下载", download_dir);
    std::fs::create_dir_all(&manual_dir).ok();

    let mut gid = None;
    let mut last_err = String::new();
    for attempt in 0..DOWNLOAD_RETRY_ATTEMPTS {
        let r = if torrent_url.starts_with("magnet:") {
            aria2.add_uri_with_dir(&torrent_url, &manual_dir)
        } else if torrent_url.ends_with(".torrent") && std::path::Path::new(&torrent_url).exists() {
            std::fs::read(&torrent_url)
                .map_err(|e| format!("Cannot read torrent: {}", e))
                .and_then(|data| aria2.add_torrent_bytes_with_dir(&data, &manual_dir))
        } else {
            aria2.add_torrent_with_dir(&torrent_url, &manual_dir)
        };
        match r {
            Ok(g) if !g.is_empty() => {
                gid = Some(g);
                break;
            }
            Ok(_) => last_err = "Empty GID".into(),
            Err(e) => {
                last_err = e;
                if attempt + 1 < DOWNLOAD_RETRY_ATTEMPTS {
                    std::thread::sleep(DOWNLOAD_RETRY_DELAY);
                }
            }
        }
    }
    let gid = gid.ok_or_else(|| {
        format!(
            "Failed after {} retries: {}",
            DOWNLOAD_RETRY_ATTEMPTS, last_err
        )
    })?;
    let id = uuid::Uuid::new_v4().to_string();
    {
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO episodes (id,subscription_id,title,torrent_url,status,gid,progress) VALUES (?1,'manual',?2,?3,'active',?4,0)",
            rusqlite::params![&id, &title, &torrent_url, &gid],
        ).map_err(|e| e.to_string())?;
    }
    Ok(DownloadItem {
        id,
        episode_title: title,
        status: "active".into(),
        progress: 0.0,
        file_path: String::new(),
        subscription_title: None,
        subscription_id: Some("manual".into()),
        episode_number: None,
        gid,
    })
}

#[tauri::command]
pub fn clean_download_dir(state: State<'_, AppState>) -> Result<String, String> {
    let (download_dir, active_gids) = {
        let app = state.inner();
        let download_dir = app
            .base_download_dir
            .lock()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string();
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT gid FROM episodes WHERE status IN ('active','paused','pending') AND gid != ''")
            .map_err(|e| e.to_string())?;
        let gids = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())?;
        (download_dir, gids)
    };
    if download_dir.is_empty() {
        return Ok("No download directory configured".into());
    }
    let dir = std::path::Path::new(&download_dir);
    if !dir.exists() {
        return Ok(format!("Does not exist: {}", download_dir));
    }
    let mut cleaned = Vec::new();
    clean_dir_recursive(dir, &mut cleaned, &active_gids);
    Ok(format!(
        "Cleaned {} files: {}",
        cleaned.len(),
        cleaned.join(", ")
    ))
}

fn clean_dir_recursive(dir: &std::path::Path, cleaned: &mut Vec<String>, active_gids: &[String]) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            clean_dir_recursive(&path, cleaned, active_gids);
        } else if name.ends_with(".torrent") || name.ends_with(".torrent.aria2") {
            // Skip files belonging to active/paused/pending downloads (stem = aria2 GID)
            let stem = name.replace(".torrent.aria2", "").replace(".torrent", "");
            if active_gids.iter().any(|g| g == &stem) {
                continue;
            }
            let _ = std::fs::remove_file(&path);
            cleaned.push(format!(
                "{}/{}",
                dir.file_name().unwrap_or_default().to_string_lossy(),
                name
            ));
        } else if name.ends_with(".aria2") {
            // .aria2 control files belong to in-progress downloads. They're named
            // after the video file (not GID), so we can't match by GID. Only clean
            // orphaned ones — we do this by checking if ANY active gid exists; if
            // aria2 is running, skip all .aria2 files to be safe. Otherwise clean.
            if active_gids.is_empty() {
                let _ = std::fs::remove_file(&path);
                cleaned.push(format!(
                    "{}/{}",
                    dir.file_name().unwrap_or_default().to_string_lossy(),
                    name
                ));
            }
        }
    }
}

fn remove_file_if_within_base(file_path: &str, base_dir: &str) {
    let path = std::path::Path::new(file_path);
    if !path.exists() {
        return;
    }

    let Ok(path) = path.canonicalize() else {
        return;
    };
    let Ok(base) = std::path::Path::new(base_dir).canonicalize() else {
        return;
    };

    if path.starts_with(base) {
        let _ = std::fs::remove_file(path);
    }
}

/// Start download for a single pending episode
#[tauri::command]
pub fn start_download(state: State<'_, AppState>, episode_id: String) -> Result<String, String> {
    let app = state.inner();
    let (torrent_url, magnet_uri, subscription_title) = {
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT torrent_url, magnet_uri, IFNULL((SELECT s.title FROM subscriptions s WHERE s.id = e.subscription_id), '') FROM episodes e WHERE e.id = ?1",
            rusqlite::params![&episode_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
        ).map_err(|e| format!("Episode not found: {}", e))?
    };

    let base_dir = app
        .base_download_dir
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    let safe_name = crate::commands::rss::sanitize_dir_name(&subscription_title);
    let sub_dir = base_dir.join(&safe_name);
    std::fs::create_dir_all(&sub_dir).ok();

    let aria2 = app.aria2.lock().map_err(|e| e.to_string())?.rpc_client();

    if !Aria2Manager::port_is_open(aria2.port()) {
        return Err(format!(
            "aria2c (port {}) is not reachable. Please wait a moment and retry.",
            aria2.port()
        ));
    }
    let gid = if !torrent_url.is_empty() {
        aria2.add_torrent_with_dir(&torrent_url, &sub_dir.to_string_lossy())
    } else if !magnet_uri.is_empty() {
        aria2.add_uri_with_dir(&magnet_uri, &sub_dir.to_string_lossy())
    } else {
        return Err("No torrent URL or magnet URI available".to_string());
    }
    .map_err(|e| format!("aria2 RPC failed: {}", e))?;

    if gid.is_empty() {
        return Err("aria2 returned empty GID".into());
    }

    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE episodes SET status = 'active', gid = ?1 WHERE id = ?2",
        rusqlite::params![&gid, &episode_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(gid)
}

/// Batch start downloads for multiple pending episodes
#[tauri::command]
pub fn batch_start_downloads(
    state: State<'_, AppState>,
    episode_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut started: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for id in &episode_ids {
        let (torrent_url, magnet_uri, subscription_title) = {
            let app = state.inner();
            let conn = match app.db.conn.lock() {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!("{}: lock error {}", id, e));
                    continue;
                }
            };
            match conn.query_row(
                "SELECT torrent_url, magnet_uri, IFNULL((SELECT s.title FROM subscriptions s WHERE s.id = e.subscription_id), '') FROM episodes e WHERE e.id = ?1",
                rusqlite::params![id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            ) {
                Ok(v) => v,
                Err(e) => { errors.push(format!("{}: {}", id, e)); continue; }
            }
        };

        let base_dir = {
            let app = state.inner();
            app.base_download_dir
                .lock()
                .map_err(|e| e.to_string())?
                .clone()
        };
        let safe_name = crate::commands::rss::sanitize_dir_name(&subscription_title);
        let sub_dir = base_dir.join(&safe_name);
        std::fs::create_dir_all(&sub_dir).ok();

        let gid = {
            let app = state.inner();
            let tmp_aria2 = app.aria2.lock().map_err(|e| e.to_string())?.rpc_client();
            if !crate::aria2::Aria2Manager::port_is_open(tmp_aria2.port()) {
                errors.push(format!("{}: aria2 not reachable", id));
                continue;
            }
            let r = if !torrent_url.is_empty() {
                tmp_aria2.add_torrent_with_dir(&torrent_url, &sub_dir.to_string_lossy())
            } else if !magnet_uri.is_empty() {
                tmp_aria2.add_uri_with_dir(&magnet_uri, &sub_dir.to_string_lossy())
            } else {
                Err("No torrent URL or magnet URI".into())
            };
            match r {
                Ok(g) if !g.is_empty() => g,
                Ok(_) => {
                    errors.push(format!("{}: empty GID", id));
                    continue;
                }
                Err(e) => {
                    errors.push(format!("{}: {}", id, e));
                    continue;
                }
            }
        };

        {
            let app = state.inner();
            let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE episodes SET status = 'active', gid = ?1 WHERE id = ?2",
                rusqlite::params![&gid, id],
            )
            .ok();
        }
        started.push(gid);
    }

    if started.is_empty() && !errors.is_empty() {
        return Err(errors.join("; "));
    }
    Ok(started)
}

/// Get pending episodes grouped by subscription
#[tauri::command]
pub fn get_pending_episodes(state: State<'_, AppState>) -> Result<Vec<PendingGroup>, String> {
    let app = state.inner();
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT e.id, e.title, e.status, e.progress, e.file_path, e.gid, s.title, e.subscription_id, e.episode_number
         FROM episodes e
         LEFT JOIN subscriptions s ON e.subscription_id = s.id
         WHERE e.status = 'pending'
         ORDER BY s.title, e.episode_number DESC"
    ).map_err(|e| e.to_string())?;

    let items = stmt
        .query_map([], |row| {
            Ok(DownloadItem {
                id: row.get::<_, String>(0)?,
                episode_title: row.get::<_, String>(1)?,
                status: row.get::<_, String>(2)?,
                progress: row.get::<_, f64>(3)?,
                file_path: row.get::<_, String>(4)?,
                gid: row.get::<_, String>(5)?,
                subscription_title: row.get::<_, Option<String>>(6)?,
                subscription_id: row.get::<_, Option<String>>(7)?,
                episode_number: row.get::<_, Option<f64>>(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<DownloadItem>, _>>()
        .map_err(|e| e.to_string())?;

    // Group by subscription_title
    let mut groups: Vec<PendingGroup> = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_eps: Vec<DownloadItem> = Vec::new();

    for item in items {
        let key = item.subscription_title.clone().unwrap_or_default();
        if Some(&key) != current_key.as_ref() {
            if !current_eps.is_empty() {
                let prev_key = current_key.take().unwrap_or_default();
                let sub_id = current_eps
                    .first()
                    .and_then(|e| e.subscription_id.clone())
                    .unwrap_or_default();
                groups.push(PendingGroup {
                    subscription_title: prev_key,
                    subscription_id: sub_id,
                    episodes: current_eps,
                });
                current_eps = Vec::new();
            }
            current_key = Some(key);
        }
        current_eps.push(item);
    }
    if !current_eps.is_empty() {
        let key = current_key.unwrap_or_default();
        let sub_id = current_eps
            .first()
            .and_then(|e| e.subscription_id.clone())
            .unwrap_or_default();
        groups.push(PendingGroup {
            subscription_title: key,
            subscription_id: sub_id,
            episodes: current_eps,
        });
    }

    Ok(groups)
}
