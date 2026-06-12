use std::time::Duration;
use std::sync::Mutex;
use tauri::Manager;

use crate::AppState;

pub fn start_scheduler(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        // Wait one interval before first refresh
        let initial = {
            let state = app_handle.state::<Mutex<AppState>>();
            let app = state.lock().unwrap();
            app.db.get_setting("refresh_interval")
                .unwrap_or("30".to_string())
                .parse::<u64>()
                .unwrap_or(30)
        };
        std::thread::sleep(Duration::from_secs(initial * 60));

        loop {
            // Read interval and subscriptions in one lock
            let (current_interval, subs, known_titles, base_dir, port) = {
                let state = app_handle.state::<Mutex<AppState>>();
                let app = state.lock().unwrap();
                let interval_val = app.db.get_setting("refresh_interval")
                    .unwrap_or("30".to_string())
                    .parse::<u64>()
                    .unwrap_or(30);
                let subs = app.db.get_enabled_subscriptions().unwrap_or_default();
                let known = app.db.get_all_episode_titles().unwrap_or_default();
                let dir = app.base_download_dir.clone();
                let p = app.aria2.lock().unwrap().port();
                (interval_val, subs, known, dir, p)
            };

            for sub in subs {
                let (_sub_id, sub_title, rss_url, auto_download) = (&sub.0, &sub.1, &sub.2, sub.3);
                let safe_name = crate::commands::rss::sanitize_dir_name(sub_title);
                let sub_dir = base_dir.join(&safe_name);
                std::fs::create_dir_all(&sub_dir).ok();

                if let Ok(xml) = reqwest::blocking::get(rss_url)
                    .and_then(|r| r.text())
                {
                    if let Ok(feed) = crate::rss_parser::parse_rss(&xml) {
                        let new_eps = crate::rss_parser::extract_new_episodes(&feed, &known_titles);
                        for ep in new_eps {
                            let gid: Option<String> = if auto_download {
                                let aria2 = crate::aria2::Aria2Manager::new(port);
                                if !ep.torrent_url.is_empty() {
                                    aria2.add_torrent_with_dir(&ep.torrent_url, &sub_dir.to_string_lossy()).ok()
                                } else if !ep.magnet_uri.is_empty() {
                                    aria2.add_uri_with_dir(&ep.magnet_uri, &sub_dir.to_string_lossy()).ok()
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            {
                                let state = app_handle.state::<Mutex<AppState>>();
                                let app = state.lock().unwrap();
                                let _ = app.db.insert_episode(
                                    &sub.0,
                                    &ep.title,
                                    ep.episode_number,
                                    &ep.torrent_url,
                                    &ep.magnet_uri,
                                    &ep.pub_date,
                                    gid.as_deref(),
                                );
                            }
                        }
                    }
                }
            }

            sleep_interval(current_interval);
        }
    });
}

fn sleep_interval(minutes: u64) {
    // Sleep in 10-second chunks so shutdown can interrupt, and interval
    // changes are picked up more quickly (next loop re-reads from DB)
    let chunks = minutes * 6; // 6 chunks of 10s per minute
    for _ in 0..chunks {
        std::thread::sleep(Duration::from_secs(10));
    }
}
