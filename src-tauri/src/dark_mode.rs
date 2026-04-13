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
    let resp: ThemeResponse = reqwest::get(&url).await
        .map_err(|e| format!("Failed to get theme: {}", e))?
        .json().await
        .map_err(|e| format!("Failed to parse theme response: {}", e))?;
    Ok(resp.theme == "dark")
}

/// Switches the OS theme to dark or light mode via the sidecar.
#[tauri::command]
pub async fn set_dark_mode(enabled: bool) -> Result<(), String> {
    let route = if enabled { "dark" } else { "light" };
    let url = format!("{}/{}", base_url(), route);
    reqwest::get(&url).await
        .map_err(|e| format!("Failed to set dark mode: {}", e))?;
    Ok(())
}
