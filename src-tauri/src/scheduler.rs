use std::time::Duration;
use tauri::Manager;

use crate::AppState;

pub fn start_scheduler(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        // Wait one interval before first refresh
        let initial = {
            let app = app_handle.state::<AppState>();
            app.db
                .get_setting("refresh_interval")
                .unwrap_or("30".to_string())
                .parse::<u64>()
                .unwrap_or(30)
        };
        std::thread::sleep(Duration::from_secs(sleep_minutes(initial) * 60));

        loop {
            // Read interval and subscriptions in one lock
            let (current_interval, subs, known_titles, base_dir, aria2_client) = {
                let app = app_handle.state::<AppState>();
                let interval_val = app
                    .db
                    .get_setting("refresh_interval")
                    .unwrap_or("30".to_string())
                    .parse::<u64>()
                    .unwrap_or(30);
                let subs = app.db.get_enabled_subscriptions().unwrap_or_default();
                let known = app.db.get_all_episode_titles().unwrap_or_default();
                let dir = app
                    .base_download_dir
                    .lock()
                    .map(|d| d.clone())
                    .unwrap_or_else(|e| e.into_inner().clone());
                let aria = app
                    .aria2
                    .lock()
                    .map(|a| a.rpc_client())
                    .unwrap_or_else(|e| e.into_inner().rpc_client());
                (interval_val, subs, known, dir, aria)
            };

            for sub in subs {
                let (_sub_id, sub_title, rss_url, auto_download) = (&sub.0, &sub.1, &sub.2, sub.3);
                let safe_name = crate::commands::rss::sanitize_dir_name(sub_title);
                let sub_dir = base_dir.join(&safe_name);
                std::fs::create_dir_all(&sub_dir).ok();

                if let Ok(xml) = reqwest::blocking::get(rss_url).and_then(|r| r.text()) {
                    if let Ok(feed) = crate::rss_parser::parse_rss(&xml) {
                        let new_eps = crate::rss_parser::extract_new_episodes(&feed, &known_titles);
                        for ep in new_eps {
                            let inserted_id = {
                                let app = app_handle.state::<AppState>();
                                app.db
                                    .insert_episode(crate::db::NewEpisode {
                                        subscription_id: &sub.0,
                                        title: &ep.title,
                                        episode_number: ep.episode_number,
                                        torrent_url: &ep.torrent_url,
                                        magnet_uri: &ep.magnet_uri,
                                        pub_date: &ep.pub_date,
                                        gid: None,
                                    })
                                    .ok()
                                    .flatten()
                            };

                            let Some(episode_id) = inserted_id else {
                                continue;
                            };

                            if auto_download {
                                let gid: Option<String> = if !ep.torrent_url.is_empty() {
                                    aria2_client
                                        .add_torrent_with_dir(
                                            &ep.torrent_url,
                                            &sub_dir.to_string_lossy(),
                                        )
                                        .ok()
                                } else if !ep.magnet_uri.is_empty() {
                                    aria2_client
                                        .add_uri_with_dir(
                                            &ep.magnet_uri,
                                            &sub_dir.to_string_lossy(),
                                        )
                                        .ok()
                                } else {
                                    None
                                };
                                if let Some(gid) = gid {
                                    let app = app_handle.state::<AppState>();
                                    let _ =
                                        app.db.update_episode_download_started(&episode_id, &gid);
                                }
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
    let chunks = sleep_minutes(minutes) * 6; // 6 chunks of 10s per minute
    for _ in 0..chunks {
        std::thread::sleep(Duration::from_secs(10));
    }
}

fn sleep_minutes(minutes: u64) -> u64 {
    minutes.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_minutes_clamps_zero_to_one() {
        assert_eq!(sleep_minutes(0), 1);
        assert_eq!(sleep_minutes(15), 15);
    }
}
