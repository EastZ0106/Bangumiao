use rusqlite::Connection;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

pub struct Database {
    pub conn: Mutex<Connection>,
}

pub type EnabledSubscription = (String, String, String, bool);

pub struct NewEpisode<'a> {
    pub subscription_id: &'a str,
    pub title: &'a str,
    pub episode_number: Option<f64>,
    pub torrent_url: &'a str,
    pub magnet_uri: &'a str,
    pub pub_date: &'a str,
    pub gid: Option<&'a str>,
}

impl Database {
    fn connection(&self) -> Result<MutexGuard<'_, Connection>, Box<dyn std::error::Error>> {
        self.conn.lock().map_err(|_| {
            Box::new(std::io::Error::other("database connection lock poisoned"))
                as Box<dyn std::error::Error>
        })
    }

    pub fn new(app_dir: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(app_dir)?;
        let db_path = app_dir.join("bangumiao.db");
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Database {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.connection()?;
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
            INSERT OR IGNORE INTO settings (key, value) VALUES ('auto_delete_torrent', 'true');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('close_to_tray', 'true');
            ",
        )?;

        // Migration: add auto_download column (safe — ignored if already exists)
        conn.execute(
            "ALTER TABLE subscriptions ADD COLUMN auto_download INTEGER DEFAULT 1",
            [],
        )
        .ok();

        // Migration: remove pre-existing duplicate episodes before adding the
        // uniqueness guarantee used by RSS refresh and the scheduler.
        conn.execute(
            "DELETE FROM episodes
             WHERE rowid NOT IN (
                 SELECT MIN(rowid)
                 FROM episodes
                 GROUP BY subscription_id, title
             )",
            [],
        )
        .ok();
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_episodes_subscription_title
             ON episodes(subscription_id, title)",
            [],
        )?;

        // Ensure a placeholder subscription exists for manual downloads
        conn.execute(
            "INSERT OR IGNORE INTO subscriptions (id, title, rss_url) VALUES ('manual', '手动下载', 'manual://')",
            [],
        )?;

        Ok(())
    }

    pub fn get_subscriptions(
        &self,
    ) -> Result<Vec<crate::commands::rss::Subscription>, Box<dyn std::error::Error>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, rss_url, mikan_url, cover_url, enabled, auto_download, created_at FROM subscriptions ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(crate::commands::rss::Subscription {
                id: row.get(0)?,
                title: row.get(1)?,
                rss_url: row.get(2)?,
                mikan_url: row.get(3)?,
                cover_url: row.get(4)?,
                enabled: row.get::<_, i32>(5)? != 0,
                auto_download: row.get::<_, i32>(6).unwrap_or(1) != 0,
                created_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn insert_subscription(
        &self,
        id: &str,
        title: &str,
        rss_url: &str,
        mikan_url: &str,
        cover_url: &str,
        auto_download: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT OR IGNORE INTO subscriptions (id, title, rss_url, mikan_url, cover_url, auto_download) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, title, rss_url, mikan_url, cover_url, auto_download as i32],
        )?;
        Ok(())
    }

    pub fn remove_subscription(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM subscriptions WHERE id = ?1",
            rusqlite::params![id],
        )?;
        conn.execute(
            "DELETE FROM episodes WHERE subscription_id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub fn toggle_subscription(&self, id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let conn = self.connection()?;
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

    pub fn get_enabled_subscriptions(
        &self,
    ) -> Result<Vec<EnabledSubscription>, Box<dyn std::error::Error>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, rss_url, auto_download FROM subscriptions WHERE enabled = 1",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i32>(3).unwrap_or(1) != 0,
            ))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_all_episode_titles(&self) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
        let conn = self.connection()?;
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
        episode: NewEpisode<'_>,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let conn = self.connection()?;
        let id = uuid::Uuid::new_v4().to_string();
        let gid = episode.gid.unwrap_or("");
        let status = if gid.is_empty() { "pending" } else { "active" };
        let rows = conn.execute(
            "INSERT OR IGNORE INTO episodes (id, subscription_id, title, episode_number, torrent_url, magnet_uri, pub_date, status, gid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                &id,
                episode.subscription_id,
                episode.title,
                episode.episode_number,
                episode.torrent_url,
                episode.magnet_uri,
                episode.pub_date,
                status,
                gid
            ],
        )?;
        if rows == 0 {
            Ok(None)
        } else {
            Ok(Some(id))
        }
    }

    pub fn update_episode_download_started(
        &self,
        id: &str,
        gid: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE episodes SET status = 'active', gid = ?1 WHERE id = ?2",
            rusqlite::params![gid, id],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<String, Box<dyn std::error::Error>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .map_err(|e| e.into())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    pub fn update_auto_download(
        &self,
        id: &str,
        auto_download: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE subscriptions SET auto_download = ?1 WHERE id = ?2",
            rusqlite::params![auto_download as i32, id],
        )?;
        Ok(())
    }

    pub fn get_subscription_by_id(
        &self,
        id: &str,
    ) -> Result<Option<crate::commands::rss::Subscription>, Box<dyn std::error::Error>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, rss_url, mikan_url, cover_url, enabled, auto_download, created_at FROM subscriptions WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map(rusqlite::params![id], |row| {
            Ok(crate::commands::rss::Subscription {
                id: row.get(0)?,
                title: row.get(1)?,
                rss_url: row.get(2)?,
                mikan_url: row.get(3)?,
                cover_url: row.get(4)?,
                enabled: row.get::<_, i32>(5)? != 0,
                auto_download: row.get::<_, i32>(6).unwrap_or(1) != 0,
                created_at: row.get(7)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_app_dir(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("bangumiao-test-{}-{}", name, uuid::Uuid::new_v4()));
        dir
    }

    #[test]
    fn insert_episode_ignores_duplicate_title_for_same_subscription() {
        let dir = temp_app_dir("duplicate-episode");
        let db = Database::new(&dir).expect("database should initialize");
        db.insert_subscription("sub-1", "Test", "https://example.test/rss", "", "", false)
            .expect("subscription should insert");

        let first = db
            .insert_episode(NewEpisode {
                subscription_id: "sub-1",
                title: "Episode 01",
                episode_number: Some(1.0),
                torrent_url: "https://example.test/1.torrent",
                magnet_uri: "",
                pub_date: "",
                gid: None,
            })
            .expect("first episode insert should succeed");
        let second = db
            .insert_episode(NewEpisode {
                subscription_id: "sub-1",
                title: "Episode 01",
                episode_number: Some(1.0),
                torrent_url: "https://example.test/1.torrent",
                magnet_uri: "",
                pub_date: "",
                gid: None,
            })
            .expect("duplicate insert should be ignored");

        assert!(first.is_some());
        assert!(second.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }
}
