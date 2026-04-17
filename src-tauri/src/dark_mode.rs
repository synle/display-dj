use serde::Deserialize;

#[derive(Deserialize)]
struct ThemeResponse {
    theme: String,
}

/// Returns the base URL of the display-dj sidecar HTTP server.
fn base_url() -> String {
    let port = crate::server_port();
    format!("http://127.0.0.1:{}", port)
}

/// Queries the sidecar for the current OS theme and returns true if dark mode is active.
#[tauri::command]
pub async fn get_dark_mode() -> Result<bool, String> {
    let url = format!("{}/theme", base_url());
    log::info!("get_dark_mode: GET {}", url);
    let resp: ThemeResponse = reqwest::get(&url).await
        .map_err(|e| format!("Failed to get theme: {}", e))?
        .json().await
        .map_err(|e| format!("Failed to parse theme response: {}", e))?;
    let is_dark = resp.theme == "dark";
    log::info!("get_dark_mode: theme={} is_dark={}", resp.theme, is_dark);
    Ok(is_dark)
}

/// Switches the OS theme to dark or light mode via the sidecar.
/// Updates the cached is_dark_mode state and refreshes the tray icon.
#[tauri::command]
pub async fn set_dark_mode(
    enabled: bool,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let route = if enabled { "dark" } else { "light" };
    let url = format!("{}/{}", base_url(), route);
    log::info!("set_dark_mode: enabled={} GET {}", enabled, url);
    reqwest::get(&url).await
        .map_err(|e| format!("Failed to set dark mode: {}", e))?;
    log::info!("set_dark_mode: done");
    crate::tray_icon::set_dark_mode_state(&app, enabled);
    Ok(())
}
