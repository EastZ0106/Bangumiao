use tauri::{Manager, WebviewUrl};
use tauri::webview::WebviewBuilder;

/// Open a child WebView positioned exactly where the React container is
#[tauri::command]
pub async fn open_mikan_browser(
    window: tauri::Window,
    _app_handle: tauri::AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<String, String> {
    if let Some(existing) = window.get_webview("mikan-browser") {
        let _ = existing.close();
    }

    let url: url::Url = "https://mikanani.me".parse().map_err(|e: url::ParseError| e.to_string())?;

    let builder = WebviewBuilder::new("mikan-browser", WebviewUrl::External(url))
        .focused(true)
        .on_navigation(|_url| true)
        .initialization_script(
            r#"
(function() {
    window.open = function(url) {
        window.location.href = url;
        return null;
    };
    document.addEventListener('click', function(e) {
        var a = e.target.closest('a');
        if (a && a.target === '_blank') {
            e.preventDefault();
            window.location.href = a.href;
        }
    }, true);
})();
"#,
        )
        .on_new_window(move |_url, _features| {
            tauri::webview::NewWindowResponse::Deny
        });

    window.add_child(
        builder,
        tauri::LogicalPosition::new(x, y),
        tauri::LogicalSize::new(width, height),
    ).map_err(|e| format!("Failed to create child webview: {}", e))?;

    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.set_focus().ok();
    }

    Ok("mikan-browser".into())
}

#[tauri::command]
pub async fn close_mikan_browser(window: tauri::Window) -> Result<(), String> {
    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn update_mikan_browser_bounds(
    window: tauri::Window,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.set_position(tauri::LogicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
        webview.set_size(tauri::LogicalSize::new(width, height))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn mikan_eval(window: tauri::Window, js: String) -> Result<(), String> {
    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.eval(&*js).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// A subgroup found on a bangumi detail page
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct SubgroupInfo {
    pub subgroup_id: String,
    pub subgroup_name: String,
    pub anime_title: String,
    pub bangumi_id: String,
}

/// An episode item from a subgroup's RSS feed
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct RssEpisode {
    pub title: String,
    pub torrent_url: String,
    pub magnet_uri: String,
    pub pub_date: String,
    pub episode_number: Option<f64>,
}

/// Scan the current mikanani page for all available subgroups.
///
/// Extracts bangumiId from the page URL and subgroup info from
/// `class="subgroup-name subgroup-XXX"` elements in the DOM.
#[tauri::command]
pub async fn scan_mikan_rss(window: tauri::Window) -> Result<Vec<SubgroupInfo>, String> {
    let inject = r#"
(function() {
    // Extract bangumiId from current page URL
    function getBangumiId() {
        var m = window.location.pathname.match(/\/Home\/Bangumi\/(\d+)/);
        if (m) return m[1];
        m = window.location.pathname.match(/\/Home\/Episode\/(\d+)/);
        if (m) return m[1];
        return '';
    }

    var bangumiId = getBangumiId();
    var results = [];
    var seen = {};

    // Find all subgroup-name elements: <span class="subgroup-name subgroup-611">北宇治字幕组</span>
    var spans = document.querySelectorAll('[class*="subgroup-name subgroup-"]');
    spans.forEach(function(span) {
        var cls = span.className || '';
        var m = cls.match(/subgroup-(\d+)/);
        if (!m) return;
        var sid = m[1];
        if (seen[sid]) return;
        seen[sid] = true;

        var name = (span.textContent || '').trim().replace(/\s+/g, ' ');
        if (!name || name === '订阅') return;

        // Try to get the anime title from the page
        var titleEl = document.querySelector('.anime-title, h1, .bangumi-title, .page-title');
        var animeTitle = titleEl ? titleEl.textContent.trim().replace(/\s+/g, ' ') : document.title.split(' - ').pop().trim();

        results.push({
            subgroup_id: sid,
            subgroup_name: name,
            anime_title: animeTitle,
            bangumi_id: bangumiId
        });
    });

    // If we found subgroups but no bangumiId, look harder in the page
    if (results.length === 0 && !bangumiId) {
        // Maybe we're on a page that has subgroup links but it's not a bangumi detail page
        // Scan ALL <a> tags for subgroupid= patterns
        var anchors = document.querySelectorAll('a[href*="subgroupid="]');
        var extraSeen = {};
        anchors.forEach(function(a) {
            var href = a.getAttribute('href') || '';
            var sg = href.match(/subgroupid=(\d+)/);
            var bm = href.match(/bangumiId=(\d+)/);
            if (!sg || !bm) return;
            var sid = sg[1];
            if (extraSeen[sid]) return;
            extraSeen[sid] = true;

            var sgSpan = document.querySelector('.subgroup-name.subgroup-' + sid);
            var name = sgSpan ? sgSpan.textContent.trim().replace(/\s+/g, ' ') : ('Subgroup ' + sid);

            results.push({
                subgroup_id: sid,
                subgroup_name: name,
                anime_title: document.title.split(' - ').pop().trim(),
                bangumi_id: bm[1]
            });
        });
    }

    window.__mikanScanData = JSON.stringify({
        result: results,
        info: {
            title: document.title,
            url: window.location.href,
            pathname: window.location.pathname,
            bangumiIdFromUrl: bangumiId,
            totalSubgroups: results.length
        }
    });
})();
"#;

    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.eval(inject).map_err(|e| format!("eval error: {}", e))?;
    } else {
        return Ok(vec![]);
    }

    // Read back result via eval_with_callback
    let (tx, rx) = std::sync::mpsc::channel();
    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.eval_with_callback(
            "window.__mikanScanData",
            move |r| { let _ = tx.send(r); },
        ).map_err(|e| format!("eval_with_callback error: {}", e))?;
    } else {
        return Ok(vec![]);
    }

    let raw = rx.recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| format!("Timeout waiting for RSS scan: {}", e))?;

    if raw.is_empty() || raw == "null" || raw == "undefined" {
        return Ok(vec![]);
    }

    #[derive(serde::Deserialize)]
    struct ScanResponse {
        result: Vec<SubgroupInfo>,
        info: serde_json::Value,
    }

    // eval_with_callback returns the raw value serialized as JSON.
    // If window.__mikanScanData is already a JSON string, eval_with_callback
    // returns it as a quoted JSON string (double-escaped).
    // Try direct parse first (for when eval returns the object directly),
    // then try unwrapping a JSON string (for when it returns a stringified object).
    let combined: ScanResponse = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            // It might be double-stringified — unwrap once
            let inner: String = serde_json::from_str(&raw)
                .map_err(|e| format!("JSON parse error: {} — raw: {}", e, &raw[..raw.len().min(300)]))?;
            serde_json::from_str(&inner)
                .map_err(|e| format!("JSON parse error (inner): {} — inner: {}", e, &inner[..inner.len().min(300)]))?
        }
    };

    if combined.result.is_empty() {
        let title = combined.info.get("title").and_then(|v| v.as_str()).unwrap_or("?");
        let pathname = combined.info.get("pathname").and_then(|v| v.as_str()).unwrap_or("?");
        let bangumi_id = combined.info.get("bangumiIdFromUrl").and_then(|v| v.as_str()).unwrap_or("none");
        return Err(format!(
            "未发现字幕组信息。\n页面标题: {}\n路径: {}\nbangumiId: {}\n\n请确保当前浏览的是一个番剧详情页（地址包含 /Home/Bangumi/数字）。",
            title, pathname, bangumi_id
        ));
    }

    Ok(combined.result)
}

/// Fetch and parse the RSS feed for a given bangumi + subgroup combination.
/// Returns the list of episodes from the feed.
#[tauri::command]
pub async fn fetch_mikan_rss(
    _window: tauri::Window,
    bangumi_id: String,
    subgroup_id: String,
) -> Result<Vec<RssEpisode>, String> {
    let url = format!(
        "https://mikanani.me/RSS/Bangumi?bangumiId={}&subgroupid={}",
        bangumi_id, subgroup_id
    );

    let xml = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to fetch RSS from {}: {}", url, e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read RSS body: {}", e))?;

    let feed = crate::rss_parser::parse_rss(&xml)
        .map_err(|e| format!("RSS parse error: {}", e))?;

    let episodes: Vec<RssEpisode> = feed.channel.items.iter().map(|item| {
        let torrent_url = item.enclosure.as_ref()
            .map(|e| e.url.clone())
            .unwrap_or_default();
        let magnet_uri = if item.link.starts_with("magnet:") {
            item.link.clone()
        } else {
            String::new()
        };

        // Try to parse episode number from title
        let episode_number = crate::rss_parser::extract_episode_number(&item.title);

        RssEpisode {
            title: item.title.clone(),
            torrent_url,
            magnet_uri,
            pub_date: item.pub_date.clone(),
            episode_number,
        }
    }).collect();

    if episodes.is_empty() {
        return Err(format!(
            "RSS feed returned no episodes.\nURL: {}\nFeed title: {}",
            url, feed.channel.title
        ));
    }

    Ok(episodes)
}

/// Helper: unwrap multiple levels of JSON string encoding from eval_with_callback
/// (kept for potential future use)
#[allow(dead_code)]
fn loop_unwrap_json<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T, String> {
    let mut candidate = raw.trim().to_string();
    loop {
        match serde_json::from_str::<T>(&candidate) {
            Ok(v) => return Ok(v),
            Err(_) => {
                match serde_json::from_str::<String>(&candidate) {
                    Ok(inner) => { candidate = inner; continue; }
                    Err(e) => {
                        return Err(format!(
                            "JSON parse error: {} — raw: {}",
                            e, &candidate[..candidate.len().min(300)]
                        ));
                    }
                }
            }
        }
    }
}
