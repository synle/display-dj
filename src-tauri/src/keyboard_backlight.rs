use tauri::{Emitter, Manager};

/// Returns the current keyboard backlight level (0..100) via the in-process
/// platform layer, or `None` if no supported backend reports a value on this
/// device. Uses the 5-minute SidecarCache to avoid re-probing on every poll
/// (parallel to the volume / dark-mode commands).
#[tauri::command]
pub async fn get_keyboard_backlight(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Option<u32>, String> {
    let t0 = std::time::Instant::now();
    crate::config::write_debug_log(&state, "benchmark: get_keyboard_backlight — START");

    if let Some(cached) = state.sidecar_cache.get_keyboard_backlight() {
        crate::config::write_debug_log(
            &state,
            &format!(
                "benchmark: get_keyboard_backlight — {:.1}ms (cache hit)",
                t0.elapsed().as_secs_f64() * 1000.0,
            ),
        );
        return Ok(Some(cached));
    }

    let level = tauri::async_runtime::spawn_blocking(
        crate::core::keyboard_backlight::get_keyboard_backlight,
    )
    .await
    .map_err(|e| format!("get_keyboard_backlight task join failed: {}", e))?;

    if let Some(v) = level {
        state.sidecar_cache.set_keyboard_backlight(v);
    }

    crate::config::write_debug_log(
        &state,
        &format!(
            "benchmark: get_keyboard_backlight — {:.1}ms (probe, level={:?})",
            t0.elapsed().as_secs_f64() * 1000.0,
            level,
        ),
    );
    Ok(level)
}

/// Sets the keyboard backlight level. Clamped to 0..100 and snapped to the
/// nearest 25% step (so the only reachable levels are 0/25/50/75/100, matching
/// the slider step on the frontend). Invalidates the cache and emits
/// `keyboard-backlight-changed` so the UI refetches.
#[tauri::command]
pub async fn set_keyboard_backlight(value: u32, app: tauri::AppHandle) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    let snapped = crate::core::keyboard_backlight::snap_to_25(value);
    log::info!(
        "set_keyboard_backlight: value={} snapped={}",
        value,
        snapped
    );
    if let Some(state) = app.try_state::<crate::AppState>() {
        crate::config::write_debug_log(
            &state,
            &format!(
                "set_keyboard_backlight: value={} snapped={} — START",
                value, snapped
            ),
        );
    }

    let ok = tauri::async_runtime::spawn_blocking(move || {
        crate::core::keyboard_backlight::set_keyboard_backlight(snapped)
    })
    .await
    .map_err(|e| format!("set_keyboard_backlight task join failed: {}", e))?;

    if !ok {
        log::warn!("set_keyboard_backlight: platform layer reported failure");
    }
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "set_keyboard_backlight: done platform_ok={} elapsed={:.1}ms",
        ok,
        elapsed,
    );

    if let Some(state) = app.try_state::<crate::AppState>() {
        state.sidecar_cache.invalidate_keyboard_backlight();
        crate::config::write_debug_log(
            &state,
            &format!(
                "set_keyboard_backlight: value={} platform_ok={} — {:.1}ms",
                snapped, ok, elapsed,
            ),
        );
    }

    let _ = app.emit("keyboard-backlight-changed", ());
    Ok(())
}

/// Returns true if any backend on this device can read the keyboard backlight.
/// Called once on app startup so the frontend can decide whether to render the
/// slider. Result NOT cached — the probe is cheap (one IOKit / WMI call) and
/// hardware can be plugged in/out at runtime.
#[tauri::command]
pub async fn get_keyboard_backlight_supported() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(crate::core::keyboard_backlight::is_supported)
        .await
        .map_err(|e| format!("get_keyboard_backlight_supported task join failed: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;
    use tauri::Manager;

    /// Build a fully-managed Tauri test app for command invocation.
    fn make_test_app() -> tauri::App<tauri::test::MockRuntime> {
        let app = tauri::test::mock_app();
        app.manage(AppState::default());
        app
    }

    /// get_keyboard_backlight returns Ok regardless of platform support.
    #[test]
    fn test_get_keyboard_backlight_returns_value() {
        let app = make_test_app();
        let state = app.state::<AppState>();
        let result = tauri::async_runtime::block_on(get_keyboard_backlight(state));
        assert!(result.is_ok());
    }

    /// get_keyboard_backlight hits the cache on the second call.
    #[test]
    fn test_get_keyboard_backlight_uses_cache() {
        let app = make_test_app();
        app.state::<AppState>().sidecar_cache.set_keyboard_backlight(75);
        let state = app.state::<AppState>();
        let result = tauri::async_runtime::block_on(get_keyboard_backlight(state)).unwrap();
        assert_eq!(result, Some(75));
    }

    /// get_keyboard_backlight_supported returns Ok(bool).
    #[test]
    fn test_get_keyboard_backlight_supported_returns_bool() {
        let result =
            tauri::async_runtime::block_on(get_keyboard_backlight_supported());
        assert!(result.is_ok());
    }
}
