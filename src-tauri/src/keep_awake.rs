/// Returns whether the keep-awake guard is currently active.
#[tauri::command]
pub fn get_keep_awake(state: tauri::State<'_, crate::AppState>) -> Result<bool, String> {
    let guard = state.keep_awake.lock().map_err(|e| e.to_string())?;
    Ok(guard.is_some())
}

/// Enables or disables keep-awake. When enabled, prevents the system from
/// idle-sleeping and the display from turning off. When disabled, drops the
/// guard and restores normal sleep behavior.
///
/// This MUST remain `async` — sync Tauri commands that access `AppState`
/// starve the macOS main-thread run-loop and break tray icon clicks.
#[tauri::command]
pub async fn set_keep_awake(
    enabled: bool,
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
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_keep_awake_toggle() {
        // Create a KeepAwake guard and verify it can be created and dropped
        let awake = keepawake::Builder::default()
            .display(true)
            .idle(true)
            .reason("test")
            .app_name("test")
            .app_reverse_domain("com.test.app")
            .create();
        assert!(awake.is_ok(), "KeepAwake should be creatable");
        // Dropping it should not panic
        drop(awake);
    }

    #[test]
    fn test_keep_awake_guard_in_mutex() {
        // Simulate the AppState pattern: Mutex<Option<KeepAwake>>
        let guard: std::sync::Mutex<Option<keepawake::KeepAwake>> =
            std::sync::Mutex::new(None);

        // Initially None
        assert!(guard.lock().unwrap().is_none());

        // Enable
        let awake = keepawake::Builder::default()
            .display(true)
            .idle(true)
            .reason("test")
            .app_name("test")
            .app_reverse_domain("com.test.app")
            .create()
            .unwrap();
        *guard.lock().unwrap() = Some(awake);
        assert!(guard.lock().unwrap().is_some());

        // Disable (drop by setting to None)
        *guard.lock().unwrap() = None;
        assert!(guard.lock().unwrap().is_none());
    }
}
