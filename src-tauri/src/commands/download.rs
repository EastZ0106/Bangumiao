use std::sync::Mutex;
use tauri::State;
use crate::AppState;
use crate::aria2::Aria2Manager;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct DownloadItem {
    pub id: String,
    pub episode_title: String,
    pub status: String,
    pub progress: f64,
    pub file_path: String,
    pub subscription_title: Option<String>,
    pub gid: String,
}

#[tauri::command]
pub fn get_downloads(state: State<'_, Mutex<AppState>>) -> Result<Vec<DownloadItem>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT e.id, e.title, e.status, e.progress, e.file_path, e.gid, s.title
         FROM episodes e
         LEFT JOIN subscriptions s ON e.subscription_id = s.id
         ORDER BY e.created_at DESC"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(DownloadItem {
            id: row.get::<_, String>(0)?,
            episode_title: row.get::<_, String>(1)?,
            status: row.get::<_, String>(2)?,
            progress: row.get::<_, f64>(3)?,
            file_path: row.get::<_, String>(4)?,
            gid: row.get::<_, String>(5)?,
            subscription_title: row.get::<_, Option<String>>(6)?,
        })
    }).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sync_downloads(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let mut log = String::new();

    let aria2_port: u16 = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let aria2 = app.aria2.lock().map_err(|e| e.to_string())?;
        aria2.port()
    };

    if !Aria2Manager::port_is_open(aria2_port) {
        return Ok("aria2 not reachable".into());
    }

    let tmp_aria2 = Aria2Manager::new(aria2_port);
    let active_result = tmp_aria2.tell_active();
    let stopped_result = tmp_aria2.tell_stopped(0, 50);

    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;

    if let Ok(ref active) = active_result {
        log.push_str(&format!("Active: {} tasks\n", active.len()));
        for d in active {
            let completed = d.completed_length.parse::<u64>().unwrap_or(0) as f64;
            let total = d.total_length.parse::<u64>().unwrap_or(1) as f64;
            let progress = if total > 0.0 { completed / total } else { 0.0 };
            let file_path = d.files.first().map(|f| f.path.clone()).unwrap_or_default();
            log.push_str(&format!("  gid={} status={} completed={} total={} progress={:.1}% path={}\n",
                d.gid, d.status, d.completed_length, d.total_length, progress * 100.0, file_path));

            let updated = conn.execute(
                "UPDATE episodes SET status='active', progress=?1, file_path=?2 WHERE gid=?3",
                rusqlite::params![progress, file_path, d.gid],
            ).unwrap_or(0);
            log.push_str(&format!("  DB updated: {} rows for gid={}\n", updated, d.gid));
        }
    } else if let Err(ref e) = active_result {
        log.push_str(&format!("tellActive error: {}\n", e));
    } else {
        log.push_str("tellActive failed\n");
    }

    if let Ok(ref stopped) = stopped_result {
        log.push_str(&format!("Stopped: {} tasks\n", stopped.len()));
        for d in stopped {
            let file_path = d.files.first().map(|f| f.path.clone()).unwrap_or_default();
            log.push_str(&format!("  gid={} status={} path={}\n", d.gid, d.status, file_path));

            // Only update episodes that match aria2 gids — skip metadata-only tasks
            if file_path.contains("[METADATA]") {
                continue;
            }

            let status = if d.status == "complete" { "completed" } else { "failed" };
            conn.execute(
                "UPDATE episodes SET status=?1, file_path=?2 WHERE gid=?3",
                rusqlite::params![status, file_path, d.gid],
            ).ok();

            // When download finishes (complete or failed), clean up aria2-generated .torrent
            // This is separate from the user's original torrent file — it's the auto-generated
            // <gid>.torrent that aria2 creates from magnet links or base64 torrent data.
            // We always delete it regardless of the auto_delete_torrent setting.
            {
                let download_dir: String = app.db.get_setting("download_dir").unwrap_or_default();
                if !download_dir.is_empty() {
                    let gen_torrent = std::path::Path::new(&download_dir)
                        .join(format!("{}.torrent", d.gid));
                    if gen_torrent.exists() {
                        let _ = std::fs::remove_file(&gen_torrent);
                    }
                    let gen_torrent_aria2 = std::path::Path::new(&download_dir)
                        .join(format!("{}.torrent.aria2", d.gid));
                    if gen_torrent_aria2.exists() {
                        let _ = std::fs::remove_file(&gen_torrent_aria2);
                    }
                }
            }
            }
    }

    Ok(log)
}

#[tauri::command]
pub fn pause_download(state: State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    let (gid, port) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let gid: String = conn.query_row(
            "SELECT gid FROM episodes WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        let port = app.aria2.lock().map_err(|e| e.to_string())?.port();
        (gid, port)
    };

    // Skip aria2 call if it's not reachable
    if !gid.is_empty() && Aria2Manager::port_is_open(port) {
        let tmp = Aria2Manager::new(port);
        let _ = tmp.pause(&gid);
    }

    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE episodes SET status = 'paused' WHERE id = ?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn resume_download(state: State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    let (gid, port) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let gid: String = conn.query_row(
            "SELECT gid FROM episodes WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        let port = app.aria2.lock().map_err(|e| e.to_string())?.port();
        (gid, port)
    };

    if !gid.is_empty() && Aria2Manager::port_is_open(port) {
        let tmp = Aria2Manager::new(port);
        let _ = tmp.unpause(&gid);
    }

    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE episodes SET status = 'downloading' WHERE id = ?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn remove_download(state: State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    // Collect all data in ONE lock acquisition
    let (gid, port, file_path, torrent_url, download_dir, auto_delete) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;

        let gid: String = conn.query_row(
            "SELECT gid FROM episodes WHERE id = ?1",
            rusqlite::params![&id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        let file_path: String = conn.query_row(
            "SELECT file_path FROM episodes WHERE id = ?1",
            rusqlite::params![&id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        let torrent_url: String = conn.query_row(
            "SELECT torrent_url FROM episodes WHERE id = ?1",
            rusqlite::params![&id],
            |row| row.get(0),
        ).unwrap_or_default();

        let download_dir = app.db.get_setting("download_dir").unwrap_or_default();
        let auto_delete = app.db.get_setting("auto_delete_torrent").unwrap_or("true".into()) == "true";
        let port = app.aria2.lock().map_err(|e| e.to_string())?.port();

        (gid, port, file_path, torrent_url, download_dir, auto_delete)
    }; // ALL locks released here

    // aria2 TCP calls on a blocking thread so we don't freeze the UI.
    // Use a timeout so a stuck aria2 task won't hang removal forever.
    let gid_for_aria2 = gid.clone();
    let port_for_aria2 = port;
    let aria2_result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::task::spawn_blocking(move || {
            if !gid_for_aria2.is_empty() && Aria2Manager::port_is_open(port_for_aria2) {
                let tmp = Aria2Manager::new(port_for_aria2);
                let _ = tmp.unpause(&gid_for_aria2);
                std::thread::sleep(std::time::Duration::from_millis(50));
                let _ = tmp.remove(&gid_for_aria2);
                let _ = tmp.remove_download_result(&gid_for_aria2);
            }
        })
    ).await;

    // Timeout or JoinError is non-fatal — we still delete DB and files
    if let Err(ref e) = aria2_result {
        eprintln!("[bangumiao] aria2 cleanup timed out or panicked: {}", e);
    } else if let Err(ref e) = aria2_result.unwrap() {
        eprintln!("[bangumiao] aria2 cleanup task panicked: {}", e);
    }

    // Delete downloaded video file and .aria2 control file
    if !file_path.is_empty() {
        let p = std::path::Path::new(&file_path);
        if p.exists() { let _ = std::fs::remove_file(p); }
        let aria2_file = file_path.clone() + ".aria2";
        let _ = std::fs::remove_file(&aria2_file);
    }

    // Delete original torrent file (local path) if setting enabled
    if auto_delete && !torrent_url.is_empty() && torrent_url.ends_with(".torrent") {
        let torrent_path = std::path::Path::new(&torrent_url);
        if torrent_path.exists() { let _ = std::fs::remove_file(torrent_path); }
    }

    // Always clean up aria2-generated <gid>.torrent (from magnet/base64 conversion)
    if !download_dir.is_empty() && !gid.is_empty() {
        let generated = std::path::Path::new(&download_dir).join(format!("{}.torrent", gid));
        if generated.exists() { let _ = std::fs::remove_file(&generated); }
        let gen_aria2 = std::path::Path::new(&download_dir).join(format!("{}.torrent.aria2", gid));
        if gen_aria2.exists() { let _ = std::fs::remove_file(&gen_aria2); }
    }

    // Finally delete from DB (separate lock)
    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM episodes WHERE id = ?1", rusqlite::params![&id])
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn add_torrent_download(
    state: State<'_, Mutex<AppState>>,
    torrent_url: String,
    title: String,
) -> Result<DownloadItem, String> {
    let gid = {
        let port = {
            let app = state.lock().map_err(|e| e.to_string())?;
            let aria2 = app.aria2.lock().map_err(|e| e.to_string())?;
            aria2.port()
        };

        let mut last_err = String::new();
        let mut gid_result = None;
        for i in 0..5 {
            let tmp = Aria2Manager::new(port);
            let result = if torrent_url.starts_with("magnet:") {
                tmp.add_uri(&torrent_url)
            } else if torrent_url.ends_with(".torrent") && std::path::Path::new(&torrent_url).exists() {
                match std::fs::read(&torrent_url) {
                    Ok(data) => {
                        let encoded = crate::aria2::base64_encode(&data);
                        tmp.add_torrent_b64(&encoded)
                    }
                    Err(e) => Err(format!("Cannot read torrent file: {}", e))
                }
            } else {
                tmp.add_torrent(&torrent_url)
            };
            match result {
                Ok(g) => {
                    if !g.is_empty() {
                        println!("[bangumiao] Download added: gid={}", g);
                        gid_result = Some(g);
                        break;
                    } else {
                        last_err = "Empty GID returned".into();
                    }
                }
                Err(e) => {
                    last_err = e;
                    if i < 4 { std::thread::sleep(std::time::Duration::from_millis(500)); }
                }
            }
        }
        gid_result.ok_or_else(|| format!("Failed after 5 retries: {}", last_err))?
    };

    let id = uuid::Uuid::new_v4().to_string();
    {
        let app = state.lock().map_err(|e| e.to_string())?;
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO episodes (id, subscription_id, title, torrent_url, status, gid, progress) VALUES (?1, 'manual', ?2, ?3, 'active', ?4, 0)",
            rusqlite::params![id, title, torrent_url, gid],
        ).map_err(|e| e.to_string())?;
    }

    Ok(DownloadItem {
        id,
        episode_title: title.clone(),
        status: "active".into(),
        progress: 0.0,
        file_path: String::new(),
        subscription_title: None,
        gid: gid.clone(),
    })
}

/// Clean up leftover .torrent, .aria2, .torrent.aria2 files in the download directory
#[tauri::command]
pub fn clean_download_dir(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let download_dir = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.db.get_setting("download_dir").unwrap_or_default()
    };

    if download_dir.is_empty() {
        return Ok("No download directory configured".into());
    }

    let dir = std::path::Path::new(&download_dir);
    if !dir.exists() {
        return Ok(format!("Download directory does not exist: {}", download_dir));
    }

    let mut cleaned = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Cannot read download dir: {}", e))?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_torrent_junk = name.ends_with(".torrent")
            || name.ends_with(".torrent.aria2")
            || name.ends_with(".aria2");
        if is_torrent_junk {
            let _ = std::fs::remove_file(entry.path());
            cleaned.push(name);
        }
    }

    Ok(format!("Cleaned {} files: {}", cleaned.len(), cleaned.join(", ")))
}
