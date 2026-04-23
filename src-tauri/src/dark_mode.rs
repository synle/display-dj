use serde::Deserialize;
use tauri::Manager;

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
/// Uses a 2-minute TTL cache to avoid hitting the sidecar on every poll.
#[tauri::command]
pub async fn get_dark_mode(
    state: tauri::State<'_, crate::AppState>,
) -> Result<bool, String> {
    let t0 = std::time::Instant::now();
    crate::config::write_debug_log(&state, "benchmark: get_dark_mode — START");

    if let Some(cached) = state.sidecar_cache.get_dark_mode() {
        crate::config::write_debug_log(
            &state,
            &format!("benchmark: get_dark_mode — {:.1}ms (cache hit)", t0.elapsed().as_secs_f64() * 1000.0),
        );
        return Ok(cached);
    }

    let url = format!("{}/theme", base_url());
    let resp: ThemeResponse = reqwest::get(&url).await
        .map_err(|e| format!("Failed to get theme: {}", e))?
        .json().await
        .map_err(|e| format!("Failed to parse theme response: {}", e))?;
    let is_dark = resp.theme == "dark";
    state.sidecar_cache.set_dark_mode(is_dark);

    crate::config::write_debug_log(
        &state,
        &format!(
            "benchmark: get_dark_mode — {:.1}ms (sidecar, is_dark={})",
            t0.elapsed().as_secs_f64() * 1000.0, is_dark,
        ),
    );
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
    // Invalidate cache since dark mode changed
    if let Some(state) = app.try_state::<crate::AppState>() {
        state.sidecar_cache.invalidate_dark_mode();
    }
    crate::tray_icon::set_dark_mode_state(&app, enabled);
    Ok(())
}
