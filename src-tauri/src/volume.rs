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
    let resp: VolumeResponse = reqwest::get(&url).await
        .map_err(|e| format!("Failed to get volume: {}", e))?
        .json().await
        .map_err(|e| format!("Failed to parse volume response: {}", e))?;
    Ok(resp.volume)
}

/// Sets the system volume (clamped to 0-100) via the sidecar.
#[tauri::command]
pub async fn set_volume(value: u32) -> Result<(), String> {
    let url = format!("{}/set_volume/{}", base_url(), value.min(100));
    reqwest::get(&url).await
        .map_err(|e| format!("Failed to set volume: {}", e))?;
    Ok(())
}
