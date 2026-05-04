use tauri::Manager;

/// Returns the current OS theme (true = dark mode) via the in-process platform layer.
/// Uses a 5-minute TTL cache to avoid re-probing on every poll.
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

    let is_dark = tauri::async_runtime::spawn_blocking(crate::core::theme::get_dark_mode)
        .await
        .map_err(|e| format!("get_dark_mode task join failed: {}", e))?
        .unwrap_or(false);
    state.sidecar_cache.set_dark_mode(is_dark);

    crate::config::write_debug_log(
        &state,
        &format!(
            "benchmark: get_dark_mode — {:.1}ms (probe, is_dark={})",
            t0.elapsed().as_secs_f64() * 1000.0, is_dark,
        ),
    );
    Ok(is_dark)
}

/// Switches the OS theme to dark or light mode via the in-process platform layer.
/// Updates the cached is_dark_mode state and refreshes the tray icon.
#[tauri::command]
pub async fn set_dark_mode(
    enabled: bool,
    app: tauri::AppHandle,
) -> Result<(), String> {
    log::info!("set_dark_mode: enabled={}", enabled);
    let ok = tauri::async_runtime::spawn_blocking(move || {
        crate::core::theme::set_dark_mode(enabled)
    })
    .await
    .map_err(|e| format!("set_dark_mode task join failed: {}", e))?;
    if !ok {
        log::warn!("set_dark_mode: platform layer reported failure");
    }
    log::info!("set_dark_mode: done");
    // Invalidate cache since dark mode changed
    if let Some(state) = app.try_state::<crate::AppState>() {
        state.sidecar_cache.invalidate_dark_mode();
    }
    crate::tray_icon::set_dark_mode_state(&app, enabled);
    Ok(())
}
