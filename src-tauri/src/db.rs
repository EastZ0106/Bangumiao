use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use std::collections::HashSet;

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new(app_dir: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(app_dir)?;
        let db_path = app_dir.join("bangumiao.db");
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Database { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS subscriptions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                rss_url TEXT NOT NULL UNIQUE,
                mikan_url TEXT DEFAULT '',
                cover_url TEXT DEFAULT '',
                enabled INTEGER DEFAULT 1,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS episodes (
                id TEXT PRIMARY KEY,
                subscription_id TEXT NOT NULL,
                title TEXT NOT NULL,
                episode_number REAL,
                torrent_url TEXT DEFAULT '',
                magnet_uri TEXT DEFAULT '',
                pub_date TEXT DEFAULT '',
                status TEXT DEFAULT 'pending',
                file_path TEXT DEFAULT '',
                progress REAL DEFAULT 0,
                gid TEXT DEFAULT '',
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS watch_records (
                id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL UNIQUE,
                anime_title TEXT NOT NULL,
                episode_number REAL,
                downloaded INTEGER DEFAULT 1,
                watched INTEGER DEFAULT 0,
                watched_at TEXT DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            INSERT OR IGNORE INTO settings (key, value) VALUES ('refresh_interval', '30');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('aria2_port', '6800');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('max_concurrent_downloads', '3');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('download_dir', '');
            "
        )?;

        // Ensure a placeholder subscription exists for manual downloads
        conn.execute(
            "INSERT OR IGNORE INTO subscriptions (id, title, rss_url) VALUES ('manual', '手动下载', 'manual://')",
            [],
        )?;

        Ok(())
    }

    pub fn get_subscriptions(&self) -> Result<Vec<crate::commands::rss::Subscription>, Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, rss_url, mikan_url, cover_url, enabled, created_at FROM subscriptions ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(crate::commands::rss::Subscription {
                id: row.get(0)?,
                title: row.get(1)?,
                rss_url: row.get(2)?,
                mikan_url: row.get(3)?,
                cover_url: row.get(4)?,
                enabled: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn insert_subscription(
        &self,
        id: &str,
        title: &str,
        rss_url: &str,
        mikan_url: &str,
        cover_url: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO subscriptions (id, title, rss_url, mikan_url, cover_url) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, title, rss_url, mikan_url, cover_url],
        )?;
        Ok(())
    }

    pub fn remove_subscription(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM subscriptions WHERE id = ?1", rusqlite::params![id])?;
        conn.execute("DELETE FROM episodes WHERE subscription_id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn toggle_subscription(&self, id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE subscriptions SET enabled = CASE WHEN enabled = 1 THEN 0 ELSE 1 END WHERE id = ?1",
            rusqlite::params![id],
        )?;
        let enabled: i32 = conn.query_row(
            "SELECT enabled FROM subscriptions WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )?;
        Ok(enabled != 0)
    }

    pub fn get_enabled_subscriptions(&self) -> Result<Vec<(String, String, String)>, Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, rss_url FROM subscriptions WHERE enabled = 1"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_all_episode_titles(&self) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT title FROM episodes")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut set = HashSet::new();
        for row in rows {
            set.insert(row?);
        }
        Ok(set)
    }

    pub fn insert_episode(
        &self,
        subscription_id: &str,
        title: &str,
        episode_number: Option<f64>,
        torrent_url: &str,
        magnet_uri: &str,
        pub_date: &str,
        gid: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT OR IGNORE INTO episodes (id, subscription_id, title, episode_number, torrent_url, magnet_uri, pub_date, gid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, subscription_id, title, episode_number, torrent_url, magnet_uri, pub_date, gid.unwrap_or("")],
        )?;
        Ok(id)
    }

    pub fn get_setting(&self, key: &str) -> Result<String, Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .map_err(|e| e.into())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }
}
