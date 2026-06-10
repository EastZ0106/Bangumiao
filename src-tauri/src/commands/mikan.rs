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

/// One item returned by the RSS scanner
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct RssCandidate {
    pub anime_title: String,
    pub subgroup_name: String,
    pub rss_url: String,
    pub bangumi_id: String,
    pub subgroup_id: String,
}

/// Scan current mikanani page for all RSS subscription links
#[tauri::command]
pub async fn scan_mikan_rss(window: tauri::Window) -> Result<Vec<RssCandidate>, String> {
    // Step 1: inject scan JS; it stores result in window.__mikanRssResult
    let inject = r#"
(function() {
function findAnimeTitle(el) {
    var cur = el;
    for (var i = 0; i < 10; i++) {
        if (!cur || cur === document.body) break;
        var t = cur.querySelector('a.anime-title, a[class*="title"], h3, h2, .bangumi-title');
        if (t) return t.textContent.trim();
        if (cur.previousElementSibling) {
            var pt = cur.previousElementSibling.querySelector('a.anime-title');
            if (pt) return pt.textContent.trim();
            var ptx = cur.previousElementSibling.textContent.trim();
            if (ptx.length > 0 && ptx.length < 100) return ptx;
        }
        cur = cur.parentElement;
    }
    var h1 = document.querySelector('h1, .page-title, .bangumi-page-title');
    if (h1) return h1.textContent.trim();
    return 'Unknown Anime';
}

var results = [];
var seen = {};

// Strategy 1: scan for RSS icon images and grab the parent <a> href
var rssIcons = document.querySelectorAll('img[src*="rss"], img[src*="RSS"], img[alt*="rss"], img[alt*="RSS"], i.rss, .rss-icon, [class*="rss"]');
rssIcons.forEach(function(icon) {
    var a = icon.closest('a');
    if (!a) return;
    var href = a.getAttribute('href') || '';
    if (!href) return;
    var bm = href.match(/bangumiId=(\d+)/);
    var sg = href.match(/subgroupid=(\d+)/);
    if (!bm || !sg) {
        if (href.indexOf('/RSS/') === -1) return;
    }
    var key = (bm&&sg) ? (bm[1]+':'+sg[1]) : href;
    if (seen[key]) return;
    seen[key] = true;
    var fullUrl = href.startsWith('http') ? href : ('https://mikanani.me'+href);
    results.push({
        animeTitle: findAnimeTitle(a),
        subgroupName: ((a.textContent||'').trim() || (icon.getAttribute('alt')||'') || 'RSS'),
        rssUrl: fullUrl,
        bangumiId: bm ? bm[1] : '',
        subgroupId: sg ? sg[1] : ''
    });
});

// Strategy 2: scan all <a> tags with /RSS/ in href (the original approach)
var rssLinks = document.querySelectorAll('a[href*="/RSS/"]');
rssLinks.forEach(function(a) {
    var href = a.getAttribute('href');
    var fullUrl = href.startsWith('http') ? href : ('https://mikanani.me'+href);
    if (seen[fullUrl]) return;
    seen[fullUrl] = true;

    var bm = href.match(/bangumiId=(\d+)/);
    var sg = href.match(/subgroupid=(\d+)/);

    var sgName = (a.textContent||'').trim() || a.getAttribute('title') || '';
    if (!sgName || sgName.length < 2) {
        var row = a.closest('tr, li, [class*=subgroup]');
        if (row) sgName = (row.textContent||'').trim().substring(0,50);
    }
    if (!sgName || sgName.length < 2) sgName = sg ? ('Subgroup '+sg[1]) : 'RSS Feed';

    results.push({
        animeTitle: findAnimeTitle(a),
        subgroupName: sgName.trim().replace(/\n/g, ' ').replace(/已订阅/g, '').replace(/\s+/g, ' ').trim().substring(0, 30),
        rssUrl: fullUrl,
        bangumiId: bm ? bm[1] : '',
        subgroupId: sg ? sg[1] : ''
    });
});

// Strategy 3: DEBUG — dump ALL links on the page for diagnosis
var debugAllRss = [];
var allLinks = document.querySelectorAll('a');
allLinks.forEach(function(a) {
    var h = a.getAttribute('href') || '';
    if (h.indexOf('RSS') !== -1 || h.indexOf('rss') !== -1 || h.indexOf('Bangumi') !== -1 || h.indexOf('bangumi') !== -1) {
        debugAllRss.push({href: h, text: (a.textContent||'').trim().substring(0, 60), outerHTML: a.outerHTML.substring(0, 200)});
    }
});

// Also dump the page title and URL for context
var pageInfo = {
    title: document.title,
    url: window.location.href,
    allRssLinks: debugAllRss,
    totalLinksOnPage: allLinks.length,
    rssIconCount: rssIcons.length
};

window.__mikanRssResult = JSON.stringify(results);
window.__mikanPageInfo = JSON.stringify(pageInfo);
})();
"#;

    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.eval(inject).map_err(|e| format!("eval error: {}", e))?;
    } else {
        return Ok(vec![]);
    }

    // Step 2: read back page info for diagnostics
    let (tx, rx) = std::sync::mpsc::channel();
    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.eval_with_callback(
            "JSON.stringify({result: window.__mikanRssResult, pageInfo: window.__mikanPageInfo})",
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

    // Parse the combined diagnostic + result JSON
    #[derive(serde::Deserialize)]
    struct ScanResult {
        result: String,
        page_info: serde_json::Value,
    }

    let combined: ScanResult = loop_unwrap_json(&raw)?;

    // Parse the actual RSS results from the inner JSON string
    let parsed: Vec<RssCandidate> = loop_unwrap_json(&combined.result)?;

    // If no results, include page diagnostics in the error message
    if parsed.is_empty() {
        let title = combined.page_info.get("title").and_then(|v| v.as_str()).unwrap_or("?");
        let url = combined.page_info.get("url").and_then(|v| v.as_str()).unwrap_or("?");
        let total_links = combined.page_info.get("totalLinksOnPage").and_then(|v| v.as_u64()).unwrap_or(0);
        let rss_links = &combined.page_info["allRssLinks"];
        return Err(format!(
            "未发现 RSS 链接。\n页面: {}\nURL: {}\n总链接数: {}\n包含 RSS/Bangumi 的链接: {}",
            title, url, total_links,
            serde_json::to_string_pretty(rss_links).unwrap_or_default()
        ));
    }

    Ok(parsed)
}

/// Helper: unwrap multiple levels of JSON string encoding from eval_with_callback
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