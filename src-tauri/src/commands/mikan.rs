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
