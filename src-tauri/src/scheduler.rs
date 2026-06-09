use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::interval;

use crate::AppState;

pub fn start_scheduler(state: Arc<Mutex<AppState>>) {
    tokio::spawn(async move {
        let refresh_interval = {
            let app = state.lock().unwrap();
            app.db.get_setting("refresh_interval")
                .unwrap_or("30".to_string())
                .parse::<u64>()
                .unwrap_or(30)
        };

        let mut tick = interval(Duration::from_secs(refresh_interval * 60));
        tick.tick().await; // Skip first immediate tick

        loop {
            tick.tick().await;
            let app = state.lock().unwrap();
            let subs = app.db.get_enabled_subscriptions().unwrap_or_default();
            let known_titles = app.db.get_all_episode_titles().unwrap_or_default();

            for sub in subs {
                if let Ok(xml) = reqwest::blocking::get(&sub.2)
                    .and_then(|r| r.text())
                {
                    if let Ok(feed) = crate::rss_parser::parse_rss(&xml) {
                        let new_eps = crate::rss_parser::extract_new_episodes(&feed, &known_titles);
                        for ep in new_eps {
                            let gid = if !ep.torrent_url.is_empty() {
                                // Start download via aria2
                                // TODO: implement when aria2 is ready
                                Some(String::new())
                            } else {
                                None
                            };
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
    });
}
