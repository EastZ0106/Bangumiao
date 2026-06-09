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
pub fn remove_download(state: State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    let (gid, port, file_path) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let gid: String = conn.query_row(
            "SELECT gid FROM episodes WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        let file_path: String = conn.query_row(
            "SELECT file_path FROM episodes WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        let port = app.aria2.lock().map_err(|e| e.to_string())?.port();
        (gid, port, file_path)
    };

    if !gid.is_empty() && Aria2Manager::port_is_open(port) {
        let tmp = Aria2Manager::new(port);
        let _ = tmp.remove(&gid);
        let _ = tmp.remove_download_result(&gid);
    }

    // Delete the downloaded video file
    if !file_path.is_empty() {
        let p = std::path::Path::new(&file_path);
        if p.exists() {
            let _ = std::fs::remove_file(p);
        }
        // Also delete .aria2 control file
        let aria2_file = file_path.clone() + ".aria2";
        let _ = std::fs::remove_file(&aria2_file);
    }

    // Auto-delete torrent file if setting enabled
    let torrent_url: String = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT torrent_url FROM episodes WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        ).unwrap_or_default()
    };
    let auto_delete: bool = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.db.get_setting("auto_delete_torrent").unwrap_or("true".into()) == "true"
    };
    if auto_delete && !torrent_url.is_empty() && torrent_url.ends_with(".torrent") {
        let torrent_path = std::path::Path::new(&torrent_url);
        if torrent_path.exists() {
            let _ = std::fs::remove_file(torrent_path);
        }
    }

    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM episodes WHERE id = ?1", rusqlite::params![id]).map_err(|e| e.to_string())?;
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
                // Local .torrent file — read and submit as base64
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
                    if i < 4 {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
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
