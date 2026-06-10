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

    if !Aria2Manager::port_is_open(6800) {
        return Ok("aria2 not reachable".into());
    }

    let tmp_aria2 = Aria2Manager::new(6800);
    let active_result = tmp_aria2.tell_active();
    let stopped_result = tmp_aria2.tell_stopped(0, 100);

    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;

    if let Ok(ref active) = active_result {
        log.push_str(&format!("Active: {} tasks\n", active.len()));
        for d in active {
            let completed = d.completed_length.parse::<u64>().unwrap_or(0) as f64;
            let total = d.total_length.parse::<u64>().unwrap_or(1) as f64;
            let progress = if total > 0.0 { completed / total } else { 0.0 };
            let file_path = d.files.first().map(|f| f.path.clone()).unwrap_or_default();
            eprintln!("[sync] active gid={} aria2_status={} progress={:.3}", d.gid, d.status, progress);
            // Only update if user hasn't explicitly paused this episode
            let current: String = conn.query_row(
                "SELECT status FROM episodes WHERE gid=?1",
                rusqlite::params![d.gid],
                |r| r.get(0),
            ).unwrap_or_default();
            if current == "paused" {
                eprintln!("[sync] active gid={} — skipping update (user paused)", d.gid);
                continue;
            }
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

            eprintln!(
                "[sync] stopped gid={} aria2_status={} new_status={}",
                d.gid, d.status, new_status
            );

            if !new_status.is_empty() {
                conn.execute(
                    "UPDATE episodes SET status=?1, file_path=?2 WHERE gid=?3",
                    rusqlite::params![new_status, file_path, d.gid],
                ).ok();
            }

            // Clean up orphaned .torrent / .torrent.aria2 files
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
    eprintln!("[download] pause_download id={}", id);
    // Gather data first, release locks before doing aria2 RPC
    let (gid, port) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let port = app.aria2.lock().map_err(|e| e.to_string())?.port();
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let gid: String = conn.query_row("SELECT gid FROM episodes WHERE id=?1", rusqlite::params![&id], |r| r.get(0)).map_err(|e| e.to_string())?;
        (gid, port)
    };
    // RPC without holding any locks
    if !gid.is_empty() && Aria2Manager::port_is_open(port) {
        match Aria2Manager::new(port).pause(&gid) {
            Ok(_) => eprintln!("[download] pause aria2 ok"),
            Err(e) => eprintln!("[download] pause aria2 failed: {}", e),
        }
    }
    // Update DB — always, this is the source of truth for the UI
    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE episodes SET status='paused' WHERE id=?1", rusqlite::params![&id]).map_err(|e| e.to_string())?;
    eprintln!("[download] pause_download done");
    Ok(())
}

#[tauri::command]
pub fn resume_download(state: State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    eprintln!("[download] resume_download id={}", id);
    let (gid, port) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        let port = app.aria2.lock().map_err(|e| e.to_string())?.port();
        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let gid: String = conn.query_row("SELECT gid FROM episodes WHERE id=?1", rusqlite::params![&id], |r| r.get(0)).map_err(|e| e.to_string())?;
        (gid, port)
    };
    if !gid.is_empty() && Aria2Manager::port_is_open(port) {
        match Aria2Manager::new(port).unpause(&gid) {
            Ok(_) => eprintln!("[download] unpause aria2 ok"),
            Err(e) => eprintln!("[download] unpause aria2 failed: {}", e),
        }
    }
    let app = state.lock().map_err(|e| e.to_string())?;
    let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE episodes SET status='active' WHERE id=?1", rusqlite::params![&id]).map_err(|e| e.to_string())?;
    eprintln!("[download] resume_download done");
    Ok(())
}

#[tauri::command]
pub fn remove_download(state: State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    eprintln!("[download] remove_download id={}", id);
    let (gid, file_path, torrent_url, download_dir, auto_delete, port) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        // Get setting BEFORE locking conn to avoid deadlock
        let download_dir = app.base_download_dir.to_string_lossy().to_string();
        let auto_delete = app.db.get_setting("auto_delete_torrent").unwrap_or("true".into()) == "true";
        let port = app.aria2.lock().map_err(|e| e.to_string())?.port();

        let conn = app.db.conn.lock().map_err(|e| e.to_string())?;
        let gid: String = conn.query_row("SELECT gid FROM episodes WHERE id=?1", rusqlite::params![&id], |r| r.get(0)).unwrap_or_default();
        let file_path: String = conn.query_row("SELECT file_path FROM episodes WHERE id=?1", rusqlite::params![&id], |r| r.get(0)).unwrap_or_default();
        let torrent_url: String = conn.query_row("SELECT torrent_url FROM episodes WHERE id=?1", rusqlite::params![&id], |r| r.get(0)).unwrap_or_default();
        conn.execute("DELETE FROM episodes WHERE id=?1", rusqlite::params![&id]).map_err(|e| e.to_string())?;
        eprintln!("[download] remove DB row deleted, gid={} port={} file_path={}", gid, port, file_path);
        (gid, file_path, torrent_url, download_dir, auto_delete, port)
    };

    // Offload aria2 cleanup + file deletion to a background thread
    std::thread::spawn(move || {
        eprintln!("[download] remove bg-thread start");
        if !gid.is_empty() {
            if crate::aria2::Aria2Manager::port_is_open(port) {
                let aria = crate::aria2::Aria2Manager::new(port);
                match aria.force_remove(&gid) {
                    Ok(_) => eprintln!("[download] remove bg aria2 forceRemove ok"),
                    Err(e) => eprintln!("[download] remove bg aria2 forceRemove err: {}", e),
                }
                match aria.remove_download_result(&gid) {
                    Ok(_) => eprintln!("[download] remove bg aria2 removeDownloadResult ok"),
                    Err(e) => eprintln!("[download] remove bg aria2 removeDownloadResult err: {}", e),
                }
            } else {
                eprintln!("[download] remove bg aria2 port not open, skip RPC");
            }
        }
        eprintln!("[download] remove bg waiting 500ms for file handles...");
        std::thread::sleep(std::time::Duration::from_millis(500));
        if !file_path.is_empty() {
            match std::fs::remove_file(&file_path) {
                Ok(_) => eprintln!("[download] remove bg deleted {}", file_path),
                Err(e) => eprintln!("[download] remove bg delete {} err: {}", file_path, e),
            }
            let aria2_file = format!("{}.aria2", file_path);
            let _ = std::fs::remove_file(&aria2_file);
        }
        if auto_delete && !torrent_url.is_empty() && torrent_url.ends_with(".torrent") {
            match std::fs::remove_file(&torrent_url) {
                Ok(_) => eprintln!("[download] remove bg deleted torrent {}", torrent_url),
                Err(e) => eprintln!("[download] remove bg delete torrent {} err: {}", torrent_url, e),
            }
        }
        if !download_dir.is_empty() && !gid.is_empty() {
            let _ = std::fs::remove_file(&format!("{}/{}.torrent", download_dir, gid));
            let _ = std::fs::remove_file(&format!("{}/{}.torrent.aria2", download_dir, gid));
        }
        eprintln!("[download] remove bg-thread done");
    });

    eprintln!("[download] remove_download returning Ok");
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
