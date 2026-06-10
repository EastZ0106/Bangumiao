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

    // If aria2 has no tasks, sync pending episodes
    if active_result.as_ref().map(|v| v.is_empty()).unwrap_or(true)
        && stopped_result.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
        let app = state.lock().map_err(|e| e.to_string())?;
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let pending: i32 = conn.query_row(
            "SELECT COUNT(*) FROM episodes WHERE gid != '' AND status = 'pending'",
            [], |r| r.get(0)
        ).unwrap_or(0);
        if pending > 0 {
            log.push_str(&format!("{} pending episodes marked active\n", pending));
            conn.execute("UPDATE episodes SET status='active' WHERE gid != '' AND status='pending'", []).ok();
        }
        return Ok(log);
    }

    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;

    if let Ok(ref active) = active_result {
        log.push_str(&format!("Active: {} tasks\n", active.len()));
        for d in active {
            let completed = d.completed_length.parse::<u64>().unwrap_or(0) as f64;
            let total = d.total_length.parse::<u64>().unwrap_or(1) as f64;
            let progress = if total > 0.0 { completed / total } else { 0.0 };
            let file_path = d.files.first().map(|f| f.path.clone()).unwrap_or_default();
            conn.execute(
                "UPDATE episodes SET status='active', progress=?1, file_path=?2 WHERE gid=?3",
                rusqlite::params![progress, file_path, d.gid],
            ).ok();
        }
    }

    if let Ok(ref stopped) = stopped_result {
        log.push_str(&format!("Stopped: {} tasks\n", stopped.len()));
        for d in stopped {
            let file_path = d.files.first().map(|f| f.path.clone()).unwrap_or_default();
            if file_path.contains("[METADATA]") { continue; }
            let status = if d.status == "complete" { "completed" } else { "failed" };
            conn.execute("UPDATE episodes SET status=?1, file_path=?2 WHERE gid=?3", rusqlite::params![status, file_path, d.gid]).ok();
            let download_dir: String = app.base_download_dir.to_string_lossy().to_string();
            if !download_dir.is_empty() {
                let _ = std::fs::remove_file(&format!("{}/{}.torrent", download_dir, d.gid));
                let _ = std::fs::remove_file(&format!("{}/{}.torrent.aria2", download_dir, d.gid));
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
        let gid: String = conn.query_row("SELECT gid FROM episodes WHERE id=?1", rusqlite::params![&id], |r| r.get(0)).map_err(|e| e.to_string())?;
        let port = app.aria2.lock().map_err(|e| e.to_string())?.port();
        (gid, port)
    };
    if !gid.is_empty() && Aria2Manager::port_is_open(port) {
        let _ = Aria2Manager::new(port).pause(&gid);
    }
    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE episodes SET status='paused' WHERE id=?1", rusqlite::params![&id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn resume_download(state: State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    let (gid, port) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let gid: String = conn.query_row("SELECT gid FROM episodes WHERE id=?1", rusqlite::params![&id], |r| r.get(0)).map_err(|e| e.to_string())?;
        let port = app.aria2.lock().map_err(|e| e.to_string())?.port();
        (gid, port)
    };
    if !gid.is_empty() && Aria2Manager::port_is_open(port) {
        let _ = Aria2Manager::new(port).unpause(&gid);
    }
    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE episodes SET status='downloading' WHERE id=?1", rusqlite::params![&id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn remove_download(state: State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    // Gather data under lock
    let (gid, file_path, torrent_url, download_dir, auto_delete) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let gid: String = conn.query_row("SELECT gid FROM episodes WHERE id=?1", rusqlite::params![&id], |r| r.get(0)).unwrap_or_default();
        let file_path: String = conn.query_row("SELECT file_path FROM episodes WHERE id=?1", rusqlite::params![&id], |r| r.get(0)).unwrap_or_default();
        let torrent_url: String = conn.query_row("SELECT torrent_url FROM episodes WHERE id=?1", rusqlite::params![&id], |r| r.get(0)).unwrap_or_default();
        let download_dir = app.base_download_dir.to_string_lossy().to_string();
        let auto_delete = app.db.get_setting("auto_delete_torrent").unwrap_or("true".into()) == "true";
        // Delete DB row while we have the lock
        conn.execute("DELETE FROM episodes WHERE id=?1", rusqlite::params![&id]).map_err(|e| e.to_string())?;
        (gid, file_path, torrent_url, download_dir, auto_delete)
    };
    // ── everything below uses owned Strings, no borrows on state ──

    // File cleanup
    if !file_path.is_empty() {
        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_file(&format!("{}.aria2", file_path));
    }
    if auto_delete && !torrent_url.is_empty() && torrent_url.ends_with(".torrent") {
        let _ = std::fs::remove_file(&torrent_url);
    }
    if !download_dir.is_empty() && !gid.is_empty() {
        let _ = std::fs::remove_file(&format!("{}/{}.torrent", download_dir, gid));
        let _ = std::fs::remove_file(&format!("{}/{}.torrent.aria2", download_dir, gid));
    }

    Ok(())
}

#[tauri::command]
pub fn add_torrent_download(
    state: State<'_, Mutex<AppState>>,
    torrent_url: String,
    title: String,
) -> Result<DownloadItem, String> {
    let (port, download_dir) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let p = app.aria2.lock().map_err(|e| e.to_string())?.port();
        let d = app.base_download_dir.to_string_lossy().to_string();
        (p, d)
    };
    let manual_dir = format!("{}/手动下载", download_dir);
    std::fs::create_dir_all(&manual_dir).ok();

    let mut gid = None;
    let mut last_err = String::new();
    for i in 0..5 {
        let tmp = Aria2Manager::new(port);
        let r = if torrent_url.starts_with("magnet:") {
            tmp.add_uri_with_dir(&torrent_url, &manual_dir)
        } else if torrent_url.ends_with(".torrent") && std::path::Path::new(&torrent_url).exists() {
            std::fs::read(&torrent_url)
                .map_err(|e| format!("Cannot read torrent: {}", e))
                .and_then(|data| tmp.add_torrent_b64(&crate::aria2::base64_encode(&data)))
        } else {
            tmp.add_torrent_with_dir(&torrent_url, &manual_dir)
        };
        match r {
            Ok(g) if !g.is_empty() => { gid = Some(g); break; }
            Ok(_) => last_err = "Empty GID".into(),
            Err(e) => { last_err = e; if i < 4 { std::thread::sleep(std::time::Duration::from_millis(500)); } }
        }
    }
    let gid = gid.ok_or_else(|| format!("Failed after 5 retries: {}", last_err))?;
    let id = uuid::Uuid::new_v4().to_string();
    {
        let app = state.lock().map_err(|e| e.to_string())?;
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("INSERT INTO episodes (id,subscription_id,title,torrent_url,status,gid,progress) VALUES (?1,'manual',?2,?3,'active',?4,0)", rusqlite::params![id,title,torrent_url,gid]).map_err(|e| e.to_string())?;
    }
    Ok(DownloadItem { id, episode_title: title, status: "active".into(), progress: 0.0, file_path: String::new(), subscription_title: None, gid })
}

#[tauri::command]
pub fn clean_download_dir(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let download_dir = {
        let app = state.lock().map_err(|e| e.to_string())?;
        app.base_download_dir.to_string_lossy().to_string()
    };
    if download_dir.is_empty() { return Ok("No download directory configured".into()); }
    let dir = std::path::Path::new(&download_dir);
    if !dir.exists() { return Ok(format!("Does not exist: {}", download_dir)); }
    let mut cleaned = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("Cannot read dir: {}", e))?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".torrent") || name.ends_with(".torrent.aria2") || name.ends_with(".aria2") {
            let _ = std::fs::remove_file(entry.path());
            cleaned.push(name);
        }
    }
    Ok(format!("Cleaned {} files: {}", cleaned.len(), cleaned.join(", ")))
}
