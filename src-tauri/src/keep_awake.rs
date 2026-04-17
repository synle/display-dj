/// Returns whether the keep-awake guard is currently active.
#[tauri::command]
pub fn get_keep_awake(state: tauri::State<'_, crate::AppState>) -> Result<bool, String> {
    let guard = state.keep_awake.lock().map_err(|e| e.to_string())?;
    Ok(guard.is_some())
}

/// Enables or disables keep-awake. When enabled, prevents the system from
/// idle-sleeping and the display from turning off. When disabled, drops the
/// guard and restores normal sleep behavior. Refreshes the tray icon indicator.
///
/// This MUST remain `async` — sync Tauri commands that access `AppState`
/// starve the macOS main-thread run-loop and break tray icon clicks.
#[tauri::command]
pub async fn set_keep_awake(
    enabled: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let mut guard = state.keep_awake.lock().map_err(|e| e.to_string())?;
    if enabled {
        if guard.is_none() {
            let awake = keepawake::Builder::default()
                .display(true)
                .idle(true)
                .reason("Display DJ - Keep Awake")
                .app_name("Display DJ")
                .app_reverse_domain("com.synle.display-dj")
                .create()
                .map_err(|e| format!("Failed to enable keep awake: {}", e))?;
            *guard = Some(awake);
            log::info!("keep_awake: enabled");
        }
    } else {
        if guard.is_some() {
            *guard = None; // Dropping KeepAwake releases the assertion
            log::info!("keep_awake: disabled");
        }
    }
    // Must drop guard before updating tray icon (which also locks keep_awake)
    drop(guard);
    crate::tray_icon::update_tray_icon(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Helper: attempt to create a KeepAwake guard. Returns None in headless CI
    /// environments (e.g. Linux without D-Bus ScreenSaver service).
    fn try_create_guard() -> Option<keepawake::KeepAwake> {
        keepawake::Builder::default()
            .display(true)
            .idle(true)
            .reason("test")
            .app_name("test")
            .app_reverse_domain("com.test.app")
            .create()
            .ok()
    }

    #[test]
    fn test_keep_awake_toggle() {
        // On headless CI (Linux without D-Bus), creation fails gracefully.
        // On desktop environments, verify the guard can be created and dropped.
        let awake = try_create_guard();
        if awake.is_some() {
            // Guard created — dropping should not panic
            drop(awake);
        }
        // Either way: the builder doesn't panic, and Err is handled gracefully
    }

    #[test]
    fn test_keep_awake_guard_in_mutex() {
        // Simulate the AppState pattern: Mutex<Option<KeepAwake>>
        let guard: std::sync::Mutex<Option<keepawake::KeepAwake>> =
            std::sync::Mutex::new(None);

        // Initially None
        assert!(guard.lock().unwrap().is_none());

        // Try to enable — may fail in headless CI
        if let Some(awake) = try_create_guard() {
            *guard.lock().unwrap() = Some(awake);
            assert!(guard.lock().unwrap().is_some());

            // Disable (drop by setting to None)
            *guard.lock().unwrap() = None;
            assert!(guard.lock().unwrap().is_none());
        }
    }
}
