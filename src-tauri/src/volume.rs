use crate::core::audio_output::{AudioOutputDeviceState, AudioOutputState};
use tauri::{Emitter, Manager};

static AUDIO_OUTPUT_PLATFORM_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquires the platform-operation gate shared by enumeration and switching.
fn lock_audio_output_platform() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    AUDIO_OUTPUT_PLATFORM_LOCK
        .lock()
        .map_err(|_| "audio output platform lock poisoned".to_string())
}

/// Serializes one platform-only operation.
fn run_audio_output_operation<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let _guard = lock_audio_output_platform()?;
    operation()
}

/// Returns audio-output preferences as owned values so no lock crosses an await.
fn audio_output_preferences(
    state: &crate::AppState,
) -> Result<Vec<(String, String, AudioOutputDeviceState)>, String> {
    state
        .preferences
        .lock()
        .map(|preferences| {
            preferences
                .audio_output_configs
                .iter()
                .map(|config| (config.id.clone(), config.label.clone(), config.state))
                .collect()
        })
        .map_err(|_| "preferences lock poisoned".to_string())
}

/// Formats one audio-output snapshot for bounded, support-friendly debug logs.
fn audio_output_state_summary(context: &str, output_state: &AudioOutputState) -> String {
    let mut lines = vec![format!(
        "audio output snapshot ({}): selected_device_id={:?} device_count={}",
        context,
        output_state.selected_device_id,
        output_state.devices.len()
    )];
    for (index, device) in output_state.devices.iter().enumerate() {
        lines.push(format!(
            "  [{}] id={:?} name={:?} original_name={:?} state={:?} built_in={} selected={}",
            index,
            device.id,
            device.name,
            device.original_name,
            device.state,
            device.is_built_in,
            output_state.selected_device_id.as_deref() == Some(device.id.as_str())
        ));
    }
    lines.join("\n")
}

/// Loads a platform snapshot and overlays persisted labels and states.
fn load_audio_output_state_unlocked(
    app: &tauri::AppHandle,
) -> Result<AudioOutputState, String> {
    let state = app
        .try_state::<crate::AppState>()
        .ok_or_else(|| "app state unavailable before audio output refresh".to_string())?;
    let preferences = audio_output_preferences(&state)?;
    let output_state = crate::core::audio_output::get_audio_output_state()?;
    Ok(crate::core::audio_output::apply_preferences(
        output_state,
        &preferences,
    ))
}

/// Replaces the shared audio-output snapshot and reports whether it changed.
pub(crate) fn cache_audio_output_state(
    app: &tauri::AppHandle,
    output_state: &AudioOutputState,
) -> Result<bool, String> {
    let state = app
        .try_state::<crate::AppState>()
        .ok_or_else(|| "app state unavailable before audio output cache update".to_string())?;
    let mut cached = state
        .audio_output_state
        .lock()
        .map_err(|_| "audio output state lock poisoned".to_string())?;
    if cached.as_ref() == Some(output_state) {
        return Ok(false);
    }
    *cached = Some(output_state.clone());
    Ok(true)
}

/// Refreshes and commits one snapshot while holding the platform-operation gate.
pub(crate) fn refresh_audio_output_state(
    app: &tauri::AppHandle,
) -> Result<(AudioOutputState, bool), String> {
    let _guard = lock_audio_output_platform()?;
    let output_state = load_audio_output_state_unlocked(app)?;
    let changed = cache_audio_output_state(app, &output_state)?;
    if changed {
        log::info!(
            "{}",
            audio_output_state_summary("refresh changed", &output_state)
        );
    }
    Ok((output_state, changed))
}

/// Updates tray and popup consumers after an audio-output snapshot changes.
pub(crate) fn notify_audio_output_state_changed(
    app: &tauri::AppHandle,
    output_state: &AudioOutputState,
) {
    let snapshot_is_current = app
        .try_state::<crate::AppState>()
        .and_then(|state| {
            state
                .audio_output_state
                .lock()
                .ok()
                .map(|cached| cached.as_ref() == Some(output_state))
        })
        .unwrap_or(false);
    if !snapshot_is_current {
        return;
    }

    crate::tray::schedule_tray_menu_rebuild(app);
    if let Err(error) = app.emit("audio-output-changed", output_state.clone()) {
        log::warn!("failed to emit audio-output-changed: {}", error);
    }
}

/// Drops no-op entries and keeps persisted audio-output IDs deterministic.
fn normalize_audio_output_configs(configs: &mut Vec<crate::config::AudioOutputMetadata>) {
    configs.retain(|config| {
        !config.label.trim().is_empty() || config.state != AudioOutputDeviceState::Enabled
    });
    configs.sort_by(|left, right| left.id.cmp(&right.id));
}

/// Replaces one saved audio-output alias; empty labels clear the saved entry.
fn update_audio_output_alias(
    configs: &mut Vec<crate::config::AudioOutputMetadata>,
    id: String,
    label: &str,
) {
    if let Some(config) = configs.iter_mut().find(|config| config.id == id) {
        config.label = label.to_string();
    } else if !label.is_empty() {
        configs.push(crate::config::AudioOutputMetadata {
            id,
            label: label.to_string(),
            state: AudioOutputDeviceState::Enabled,
        });
    }
    normalize_audio_output_configs(configs);
}

/// Replaces one saved audio-output state while preserving any alias.
fn update_audio_output_state(
    configs: &mut Vec<crate::config::AudioOutputMetadata>,
    id: String,
    device_state: AudioOutputDeviceState,
) {
    if let Some(config) = configs.iter_mut().find(|config| config.id == id) {
        config.state = device_state;
    } else if device_state != AudioOutputDeviceState::Enabled {
        configs.push(crate::config::AudioOutputMetadata {
            id,
            label: String::new(),
            state: device_state,
        });
    }
    normalize_audio_output_configs(configs);
}

/// Lists configurable audio outputs and overlays persisted labels and states.
#[tauri::command]
pub async fn get_audio_output_devices(
    app: tauri::AppHandle,
) -> Result<AudioOutputState, String> {
    if let Some(cached) = app
        .try_state::<crate::AppState>()
        .and_then(|state| state.audio_output_state.lock().ok()?.clone())
    {
        return Ok(cached);
    }

    let app_for_refresh = app.clone();
    let (output_state, changed) = tauri::async_runtime::spawn_blocking(move || {
        refresh_audio_output_state(&app_for_refresh)
    })
    .await
    .map_err(|error| format!("get_audio_output_devices task join failed: {}", error))??;
    if changed {
        notify_audio_output_state_changed(&app, &output_state);
    }
    Ok(output_state)
}

/// Selects the system audio output, then refreshes volume and tray mute state.
pub(crate) async fn select_audio_output_device(
    id: String,
    app: tauri::AppHandle,
) -> Result<crate::core::audio_output::AudioOutputState, String> {
    log::info!("select_audio_output_device: entry target_id={}", id);
    let preferences = app
        .try_state::<crate::AppState>()
        .ok_or_else(|| "app state unavailable before audio output selection".to_string())
        .and_then(|state| audio_output_preferences(&state))?;

    log::info!(
        "select_audio_output_device: checking preferences for target_id={}, found={} entries",
        id,
        preferences.len()
    );
    if let Some((_, label, device_state)) = preferences.iter().find(|(device_id, _, _)| device_id == &id) {
        log::info!("select_audio_output_device: preference entry for target_id={}/label={}/state={:?}", id, label, device_state);
    } else {
        log::warn!("select_audio_output_device: NO preference entry found for target_id={}", id);
    }

    if let Some((_, _, device_state)) = preferences.iter().find(|(device_id, _, _)| device_id == &id)
    {
        if *device_state != AudioOutputDeviceState::Enabled {
            log::warn!("select_audio_output_device: device is {} for target_id={}", device_state, id);
            return Err(format!("audio output device is not enabled: {}", id));
        }
    }

    let id_for_switch = id.clone();
    let app_for_switch = app.clone();
    let (output_state, output_state_changed) = tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock_audio_output_platform()?;
        let output_state =
            crate::core::audio_output::set_audio_output_device(&id_for_switch).map_err(|error| {
                log::error!(
                    "audio output selection failed: requested_id={:?} error={}",
                    id_for_switch,
                    error
                );
                error
            })?;
        let output_state =
            crate::core::audio_output::apply_preferences(output_state, &preferences);
        let changed = cache_audio_output_state(&app_for_switch, &output_state)?;
        Ok::<_, String>((output_state, changed))
    })
    .await
    .map_err(|error| format!("set_audio_output_device task join failed: {}", error))??;

    log::info!(
        "{}",
        audio_output_state_summary("selection after", &output_state)
    );
    match output_state.selected_device_id.as_deref() {
        Some(selected_id) if selected_id == id => {
            log::info!("audio output selection confirmed: id={:?}", id)
        }
        Some(selected_id) => log::warn!(
            "audio output selection mismatch: requested_id={:?} active_id={:?}",
            id,
            selected_id
        ),
        None => log::warn!(
            "audio output selection mismatch: requested_id={:?} active_id=<none>",
            id
        ),
    }

    let volume_info =
        tauri::async_runtime::spawn_blocking(crate::core::volume::get_volume)
            .await
            .map_err(|error| format!("volume refresh task join failed: {}", error))?;

    if let Some(state) = app.try_state::<crate::AppState>() {
        state.sidecar_cache.invalidate_volume();
        if let Some(info) = &volume_info {
            state.sidecar_cache.set_volume(info.volume);
        }
    }
    if let Some(info) = volume_info {
        log::info!(
            "audio output post-selection volume: requested_id={:?} volume={} muted={}",
            id,
            info.volume,
            info.muted
        );
        crate::tray_icon::set_muted_state(&app, info.muted || info.volume == 0);
        if let Err(error) = app.emit("volume-changed", info.volume) {
            log::warn!("failed to emit volume-changed after output selection: {}", error);
        }
    } else {
        log::warn!(
            "audio output post-selection volume unavailable: requested_id={:?}",
            id
        );
    }
    if output_state_changed {
        notify_audio_output_state_changed(&app, &output_state);
    }
    Ok(output_state)
}

/// Selects the system audio output from the popup UI.
#[tauri::command]
pub async fn set_audio_output_device(
    id: String,
    app: tauri::AppHandle,
) -> Result<AudioOutputState, String> {
    select_audio_output_device(id, app).await
}

/// Persists or clears a user-defined audio-output label after validating the device ID.
#[tauri::command]
pub async fn rename_audio_output_device(
    id: String,
    label: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let id_for_validation = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_audio_output_operation(|| {
            crate::core::audio_output::ensure_device_exists(&id_for_validation)
        })
    })
    .await
    .map_err(|error| format!("rename_audio_output_device task join failed: {}", error))??;

    {
        let trimmed_label = label.trim();
        let mut preferences = state
            .preferences
            .lock()
            .map_err(|_| "preferences lock poisoned".to_string())?;
        update_audio_output_alias(&mut preferences.audio_output_configs, id, trimmed_label);
        crate::config::save_preferences_to_disk(&preferences);
    }

    let app_for_refresh = app.clone();
    let (output_state, changed) = tauri::async_runtime::spawn_blocking(move || {
        refresh_audio_output_state(&app_for_refresh)
    })
    .await
    .map_err(|error| format!("rename_audio_output_device refresh failed: {}", error))??;
    if changed {
        notify_audio_output_state_changed(&app, &output_state);
    }
    Ok(())
}

/// Persists one audio-output enabled, disabled, or hidden state.
#[tauri::command]
pub async fn set_audio_output_device_state(
    id: String,
    device_state: AudioOutputDeviceState,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<AudioOutputState, String> {
    let id_for_validation = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_audio_output_operation(|| {
            crate::core::audio_output::ensure_device_exists(&id_for_validation)
        })
    })
    .await
    .map_err(|error| format!("set_audio_output_device_state task join failed: {}", error))??;

    {
        let mut preferences = state
            .preferences
            .lock()
            .map_err(|_| "preferences lock poisoned".to_string())?;
        update_audio_output_state(
            &mut preferences.audio_output_configs,
            id.clone(),
            device_state,
        );
        crate::config::save_preferences_to_disk(&preferences);
    }

    let app_for_refresh = app.clone();
    let (output_state, changed) = tauri::async_runtime::spawn_blocking(move || {
        refresh_audio_output_state(&app_for_refresh)
    })
    .await
    .map_err(|error| format!("set_audio_output_device_state refresh failed: {}", error))??;
    log::info!(
        "audio output state updated: id={} state={:?}",
        id,
        device_state
    );
    if changed {
        notify_audio_output_state_changed(&app, &output_state);
    }
    Ok(output_state)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Audio-output diagnostics list each endpoint and identify the active one.
    #[test]
    fn audio_output_state_summary_lists_devices_and_selection() {
        let output_state = AudioOutputState {
            devices: vec![
                crate::core::audio_output::AudioOutputDevice {
                    id: "speaker-a".into(),
                    name: "Desk".into(),
                    original_name: "Speakers".into(),
                    state: AudioOutputDeviceState::Enabled,
                    is_built_in: true,
                },
                crate::core::audio_output::AudioOutputDevice {
                    id: "speaker-b".into(),
                    name: "Monitor".into(),
                    original_name: "HDMI".into(),
                    state: AudioOutputDeviceState::Disabled,
                    is_built_in: false,
                },
            ],
            selected_device_id: Some("speaker-b".into()),
        };

        assert_eq!(
            audio_output_state_summary("test", &output_state),
            "audio output snapshot (test): selected_device_id=Some(\"speaker-b\") device_count=2\n  [0] id=\"speaker-a\" name=\"Desk\" original_name=\"Speakers\" state=Enabled built_in=true selected=false\n  [1] id=\"speaker-b\" name=\"Monitor\" original_name=\"HDMI\" state=Disabled built_in=false selected=true"
        );
    }

    /// Saving an audio-output alias replaces its prior value and sorts stable IDs.
    #[test]
    fn update_audio_output_alias_replaces_and_sorts() {
        let mut configs = vec![
            crate::config::AudioOutputMetadata {
                id: "z-device".into(),
                label: "Z".into(),
                state: AudioOutputDeviceState::Disabled,
            },
            crate::config::AudioOutputMetadata {
                id: "a-device".into(),
                label: "Old".into(),
                state: AudioOutputDeviceState::Hidden,
            },
        ];

        update_audio_output_alias(&mut configs, "a-device".into(), "Desk");

        assert_eq!(
            configs,
            vec![
                crate::config::AudioOutputMetadata {
                    id: "a-device".into(),
                    label: "Desk".into(),
                    state: AudioOutputDeviceState::Hidden,
                },
                crate::config::AudioOutputMetadata {
                    id: "z-device".into(),
                    label: "Z".into(),
                    state: AudioOutputDeviceState::Disabled,
                },
            ]
        );
    }

    /// Saving an empty audio-output alias removes the persisted entry.
    #[test]
    fn update_audio_output_alias_clears_empty_label() {
        let mut configs = vec![crate::config::AudioOutputMetadata {
            id: "device".into(),
            label: "Desk".into(),
            state: AudioOutputDeviceState::Enabled,
        }];

        update_audio_output_alias(&mut configs, "device".into(), "");

        assert!(configs.is_empty());
    }

    /// Changing state preserves aliases and removes restored defaults without aliases.
    #[test]
    fn update_audio_output_state_preserves_alias_and_prunes_defaults() {
        let mut configs = vec![crate::config::AudioOutputMetadata {
            id: "speaker".into(),
            label: "Desk".into(),
            state: AudioOutputDeviceState::Enabled,
        }];

        update_audio_output_state(
            &mut configs,
            "speaker".into(),
            AudioOutputDeviceState::Hidden,
        );
        assert_eq!(configs[0].label, "Desk");
        assert_eq!(configs[0].state, AudioOutputDeviceState::Hidden);

        update_audio_output_alias(&mut configs, "speaker".into(), "");
        assert_eq!(configs[0].state, AudioOutputDeviceState::Hidden);

        update_audio_output_state(
            &mut configs,
            "speaker".into(),
            AudioOutputDeviceState::Enabled,
        );
        assert!(configs.is_empty());
    }
}
