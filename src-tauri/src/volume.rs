use serde::Deserialize;

#[derive(Deserialize)]
struct VolumeResponse {
    volume: u32,
}

/// Returns the base URL of the display-dj sidecar HTTP server.
fn base_url() -> String {
    let port = crate::server_port();
    format!("http://127.0.0.1:{}", port)
}

/// Returns the current system volume (0-100) from the sidecar.
#[tauri::command]
pub async fn get_volume() -> Result<u32, String> {
    let url = format!("{}/get_volume", base_url());
    log::info!("get_volume: GET {}", url);
    let resp: VolumeResponse = reqwest::get(&url).await
        .map_err(|e| format!("Failed to get volume: {}", e))?
        .json().await
        .map_err(|e| format!("Failed to parse volume response: {}", e))?;
    log::info!("get_volume: volume={}", resp.volume);
    Ok(resp.volume)
}

/// Sets the system volume (clamped to 0-100) via the sidecar.
/// Updates the cached is_muted state and refreshes the tray icon.
#[tauri::command]
pub async fn set_volume(value: u32, app: tauri::AppHandle) -> Result<(), String> {
    let clamped = value.min(100);
    let url = format!("{}/set_volume/{}", base_url(), clamped);
    log::info!("set_volume: value={} GET {}", clamped, url);
    reqwest::get(&url).await
        .map_err(|e| format!("Failed to set volume: {}", e))?;
    log::info!("set_volume: done");
    crate::tray_icon::set_muted_state(&app, clamped == 0);
    Ok(())
}
