use tauri::Manager;

/// Returns the current system volume (0-100) via the in-process platform layer.
/// Uses a 5-minute TTL cache to avoid re-probing on every poll.
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

    let volume = tauri::async_runtime::spawn_blocking(crate::core::volume::get_volume)
        .await
        .map_err(|e| format!("get_volume task join failed: {}", e))?
        .map(|info| info.volume)
        .unwrap_or(0);
    state.sidecar_cache.set_volume(volume);

    crate::config::write_debug_log(
        &state,
        &format!(
            "benchmark: get_volume — {:.1}ms (probe, volume={})",
            t0.elapsed().as_secs_f64() * 1000.0, volume,
        ),
    );
    Ok(volume)
}

/// Sets the system volume (clamped to 0-100) via the in-process platform layer.
/// Updates the cached is_muted state and refreshes the tray icon.
#[tauri::command]
pub async fn set_volume(value: u32, app: tauri::AppHandle) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    let clamped = value.min(100);
    log::info!("set_volume: value={} clamped={}", value, clamped);
    if let Some(state) = app.try_state::<crate::AppState>() {
        crate::config::write_debug_log(
            &state,
            &format!("set_volume: value={} clamped={} — START", value, clamped),
        );
    }
    let ok = tauri::async_runtime::spawn_blocking(move || {
        crate::core::volume::set_volume(clamped as u16)
    })
    .await
    .map_err(|e| format!("set_volume task join failed: {}", e))?;
    if !ok {
        log::warn!("set_volume: platform layer reported failure");
    }
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
    log::info!("set_volume: done platform_ok={} elapsed={:.1}ms", ok, elapsed);
    // Invalidate cache since volume changed
    if let Some(state) = app.try_state::<crate::AppState>() {
        state.sidecar_cache.invalidate_volume();
        crate::config::write_debug_log(
            &state,
            &format!("set_volume: value={} platform_ok={} — {:.1}ms", clamped, ok, elapsed),
        );
    }
    crate::tray_icon::set_muted_state(&app, clamped == 0);
    Ok(())
}
