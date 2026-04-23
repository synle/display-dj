use serde::Deserialize;
use tauri::Manager;

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
/// Uses a 2-minute TTL cache to avoid hitting the sidecar on every poll.
#[tauri::command]
pub async fn get_volume(
    state: tauri::State<'_, crate::AppState>,
) -> Result<u32, String> {
    let t0 = std::time::Instant::now();
    crate::config::write_debug_log(&state, "benchmark: get_volume — START");

    if let Some(cached) = state.sidecar_cache.get_volume() {
        crate::config::write_debug_log(
            &state,
            &format!("benchmark: get_volume — {:.1}ms (cache hit)", t0.elapsed().as_secs_f64() * 1000.0),
        );
        return Ok(cached);
    }

    let url = format!("{}/get_volume", base_url());
    let resp: VolumeResponse = reqwest::get(&url).await
        .map_err(|e| format!("Failed to get volume: {}", e))?
        .json().await
        .map_err(|e| format!("Failed to parse volume response: {}", e))?;
    state.sidecar_cache.set_volume(resp.volume);

    crate::config::write_debug_log(
        &state,
        &format!(
            "benchmark: get_volume — {:.1}ms (sidecar, volume={})",
            t0.elapsed().as_secs_f64() * 1000.0, resp.volume,
        ),
    );
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
    // Invalidate cache since volume changed
    if let Some(state) = app.try_state::<crate::AppState>() {
        state.sidecar_cache.invalidate_volume();
    }
    crate::tray_icon::set_muted_state(&app, clamped == 0);
    Ok(())
}
