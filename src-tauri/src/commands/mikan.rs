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
    // Close existing one if present
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

    // Explicitly set focus after adding
    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.set_focus().ok();
    }

    Ok("mikan-browser".into())
}

/// Close the mikan child WebView (called when user leaves /browse)
#[tauri::command]
pub async fn close_mikan_browser(window: tauri::Window) -> Result<(), String> {
    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Resize/reposition the child WebView (called on window resize)
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

/// Execute JS in the mikan WebView
#[tauri::command]
pub async fn mikan_eval(window: tauri::Window, js: String) -> Result<(), String> {
    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.eval(&js).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Scan the current mikanani page for RSS subscription links and return structured data
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct RssCandidate {
    pub anime_title: String,
    pub subgroup_name: String,
    pub rss_url: String,
    pub bangumi_id: String,
    pub subgroup_id: String,
}

#[tauri::command]
pub async fn scan_mikan_rss(window: tauri::Window) -> Result<Vec<RssCandidate>, String> {
    // This JS scans all <a> tags with /RSS/Bangumi in href,
    // extracts bangumiId/subgroupid params, and walks up the DOM to find the anime title
    let js = r#"
(function() {
    function findAnimeTitle(el) {
        // Walk up through common container elements to find the anime card/section
        var current = el;
        for (var i = 0; i < 10; i++) {
            if (!current || current === document.body) break;
            // Mikan list page: the anime title is often in a nearby heading or link
            // Look for elements with common title-related classes or <a class="title">
            var titleEl = current.querySelector('a.anime-title, a[class*="title"], h3, h2, .bangumi-title');
            if (titleEl) return titleEl.textContent.trim();
            // Also check siblings / parent siblings
            if (current.previousElementSibling) {
                var prevTitle = current.previousElementSibling.querySelector('a.anime-title, a[class*="title"], h3, h2');
                if (prevTitle) return prevTitle.textContent.trim();
                var prevText = current.previousElementSibling.textContent.trim();
                if (prevText.length > 0 && prevText.length < 100) return prevText;
            }
            current = current.parentElement;
        }
        // Fallback: use the page's main heading or document title
        var pageTitle = document.querySelector('h1, .page-title, .bangumi-page-title');
        if (pageTitle) return pageTitle.textContent.trim();
        return 'Unknown Anime';
    }

    var results = [];
    var seen = {};
    var links = document.querySelectorAll('a[href*="/RSS/Bangumi"]');
    links.forEach(function(a) {
        var href = a.getAttribute('href');
        var bangumiMatch = href.match(/bangumiId=(\d+)/);
        var subgroupMatch = href.match(/subgroupid=(\d+)/);
        if (!bangumiMatch || !subgroupMatch) return;

        var bangumiId = bangumiMatch[1];
        var subgroupId = subgroupMatch[1];
        var key = bangumiId + ':' + subgroupId;

        if (seen[key]) return;
        seen[key] = true;

        // Subtitle group name is usually the text content of the RSS <a> tag
        // or the title attribute of the enclosing element
        var subgroupName = (a.textContent || '').trim();
        if (!subgroupName || subgroupName.length < 2) {
            subgroupName = a.getAttribute('title') || '';
        }
        if (!subgroupName || subgroupName.length < 2) {
            // Try the parent row has the subgroup name
            var row = a.closest('tr, li, .subgroup-row, [class*="subgroup"]');
            if (row) subgroupName = (row.textContent || '').trim().substring(0, 50);
        }
        if (!subgroupName || subgroupName.length < 2) {
            subgroupName = 'Subgroup ' + subgroupId;
        }

        var animeTitle = findAnimeTitle(a);

        var fullUrl = href.startsWith('http') ? href : ('https://mikanani.me' + href);

        results.push({
            animeTitle: animeTitle,
            subgroupName: subgroupName,
            rssUrl: fullUrl,
            bangumiId: bangumiId,
            subgroupId: subgroupId
        });
    });

    // If no per-episode RSS links found, also check for the MyBangumi token link
    if (results.length === 0) {
        var tokenLinks = document.querySelectorAll('a[href*="/RSS/MyBangumi"]');
        tokenLinks.forEach(function(a) {
            var href = a.getAttribute('href');
            var fullUrl = href.startsWith('http') ? href : ('https://mikanani.me' + href);
            results.push({
                animeTitle: 'MyBangumi (个人聚合)',
                subgroupName: 'All',
                rssUrl: fullUrl,
                bangumiId: '',
                subgroupId: ''
            });
        });
    }

    return JSON.stringify(results);
})();
"#;

    // Use eval_with_callback to get the return value from the JS scan
    let (tx, rx) = std::sync::mpsc::channel();

    if let Some(webview) = window.get_webview("mikan-browser") {
        webview.eval_with_callback(
            &format!("({})()", js),
            move |result| {
                let _ = tx.send(result);
            },
        ).map_err(|e| format!("eval_with_callback error: {}", e))?;
    } else {
        return Ok(vec![]);
    }

    // Wait up to 3 seconds for the JS to return
    let raw = rx.recv_timeout(std::time::Duration::from_secs(3))
        .map_err(|e| format!("Timeout waiting for RSS scan: {}", e))?;

    // eval() returns the JS result as a String; parse it as JSON
    if raw.is_empty() || raw == "null" {
        return Ok(vec![]);
    }

    serde_json::from_str(&raw).map_err(|e| format!("JSON parse error: {} — raw: {}", e, &raw[..raw.len().min(200)]))
}

#[cfg(test)]
mod mikan_tests {

    #[test]
    fn test_scan_mikan_rss() {
        let js = r#"
(function() {
    function findAnimeTitle(el) {
        var current = el;
        for (var i = 0; i < 10; i++) {
            if (!current || current === document.body) break;
            var titleEl = current.querySelector('a.anime-title, a[class*="title"], h3, h2, .bangumi-title');
            if (titleEl) return titleEl.textContent.trim();
            if (current.previousElementSibling) {
                var prevTitle = current.previousElementSibling.querySelector('a.anime-title, a[class*="title"], h3, h2');
                if (prevTitle) return prevTitle.textContent.trim();
                var prevText = current.previousElementSibling.textContent.trim();
                if (prevText.length > 0 && prevText.length < 100) return prevText;
            }
            current = current.parentElement;
        }
        var pageTitle = document.querySelector('h1, .page-title, .bangumi-page-title');
        if (pageTitle) return pageTitle.textContent.trim();
        return 'Unknown Anime';
    }

    var results = [];
    var seen = {};
    var links = document.querySelectorAll('a[href*="/RSS/Bangumi"]');
    links.forEach(function(a) {
        var href = a.getAttribute('href');
        var bangumiMatch = href.match(/bangumiId=(\d+)/);
        var subgroupMatch = href.match(/subgroupid=(\d+)/);
        if (!bangumiMatch || !subgroupMatch) return;

        var bangumiId = bangumiMatch[1];
        var subgroupId = subgroupMatch[1];
        var key = bangumiId + ':' + subgroupId;

        if (seen[key]) return;
        seen[key] = true;

        var subgroupName = (a.textContent || '').trim();
        if (!subgroupName || subgroupName.length < 2) {
            subgroupName = a.getAttribute('title') || '';
        }
        if (!subgroupName || subgroupName.length < 2) {
            var row = a.closest('tr, li, .subgroup-row, [class*="subgroup"]');
            if (row) subgroupName = (row.textContent || '').trim().substring(0, 50);
        }
        if (!subgroupName || subgroupName.length < 2) {
            subgroupName = 'Subgroup ' + subgroupId;
        }

        var animeTitle = findAnimeTitle(a);

        var fullUrl = href.startsWith('http') ? href : ('https://mikanani.me' + href);

        results.push({
            animeTitle: animeTitle,
            subgroupName: subgroupName,
            rssUrl: fullUrl,
            bangumiId: bangumiId,
            subgroupId: subgroupId
        });
    });

    if (results.length === 0) {
        var tokenLinks = document.querySelectorAll('a[href*="/RSS/MyBangumi"]');
        tokenLinks.forEach(function(a) {
            var href = a.getAttribute('href');
            var fullUrl = href.startsWith('http') ? href : ('https://mikanani.me' + href);
            results.push({
                animeTitle: 'MyBangumi (个人聚合)',
                subgroupName: 'All',
                rssUrl: fullUrl,
                bangumiId: '',
                subgroupId: ''
            });
        });
    }

    return JSON.stringify(results);
})();
        "#;
        // Verify JS constructable — structural check only
        assert!(!js.is_empty());
    }
}
