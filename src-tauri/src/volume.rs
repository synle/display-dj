use crate::core::audio_output::AudioOutputDeviceState;
use tauri::{Emitter, Manager};

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

/// Drops no-op entries and keeps persisted audio-output IDs deterministic.
fn normalize_audio_output_configs(configs: &mut Vec<crate::config::AudioOutputMetadata>) {
    configs.retain(|config| {
        !config.label.trim().is_empty() || config.state != AudioOutputDeviceState::Enabled
    });
    configs.sort_by(|left, right| left.id.cmp(&right.id));
}

/// Replaces one saved alias; empty labels preserve only non-default states.
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
    state: tauri::State<'_, crate::AppState>,
) -> Result<crate::core::audio_output::AudioOutputState, String> {
    let preferences = audio_output_preferences(&state)?;
    let output_state =
        tauri::async_runtime::spawn_blocking(crate::core::audio_output::get_audio_output_state)
            .await
            .map_err(|error| format!("get_audio_output_devices task join failed: {}", error))??;
    Ok(crate::core::audio_output::apply_preferences(
        output_state,
        &preferences,
    ))
}

/// Selects the system audio output, then refreshes volume and tray mute state.
#[tauri::command]
pub async fn set_audio_output_device(
    id: String,
    app: tauri::AppHandle,
) -> Result<crate::core::audio_output::AudioOutputState, String> {
    let preferences = app
        .try_state::<crate::AppState>()
        .ok_or_else(|| "app state unavailable before audio output selection".to_string())
        .and_then(|state| audio_output_preferences(&state))?;
    if let Some((_, _, device_state)) = preferences
        .iter()
        .find(|(device_id, _, _)| device_id == &id)
    {
        if *device_state != AudioOutputDeviceState::Enabled {
            return Err(format!("audio output device is not enabled: {}", id));
        }
    }

    let id_for_switch = id.clone();
    let output_state = tauri::async_runtime::spawn_blocking(move || {
        crate::core::audio_output::set_audio_output_device(&id_for_switch)
    })
    .await
    .map_err(|error| format!("set_audio_output_device task join failed: {}", error))??;

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
        crate::tray_icon::set_muted_state(&app, info.muted || info.volume == 0);
        if let Err(error) = app.emit("volume-changed", info.volume) {
            log::warn!("failed to emit volume-changed after output selection: {}", error);
        }
    }
    log::info!("audio output selected: id={}", id);
    Ok(crate::core::audio_output::apply_preferences(
        output_state,
        &preferences,
    ))
}

/// Persists or clears a user-defined audio-output label after validating the device ID.
#[tauri::command]
pub async fn rename_audio_output_device(
    id: String,
    label: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let id_for_validation = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::core::audio_output::ensure_device_exists(&id_for_validation)
    })
    .await
    .map_err(|error| format!("rename_audio_output_device task join failed: {}", error))??;

    let trimmed_label = label.trim();
    let mut preferences = state
        .preferences
        .lock()
        .map_err(|_| "preferences lock poisoned".to_string())?;
    update_audio_output_alias(&mut preferences.audio_output_configs, id, trimmed_label);
    crate::config::save_preferences_to_disk(&preferences);
    Ok(())
}

/// Persists one audio-output enabled, disabled, or hidden state.
#[tauri::command]
pub async fn set_audio_output_device_state(
    id: String,
    device_state: AudioOutputDeviceState,
    state: tauri::State<'_, crate::AppState>,
) -> Result<crate::core::audio_output::AudioOutputState, String> {
    let id_for_validation = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::core::audio_output::ensure_device_exists(&id_for_validation)
    })
    .await
    .map_err(|error| format!("set_audio_output_device_state task join failed: {}", error))??;

    let saved_preferences = {
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
        preferences
            .audio_output_configs
            .iter()
            .map(|config| (config.id.clone(), config.label.clone(), config.state))
            .collect::<Vec<_>>()
    };

    let output_state =
        tauri::async_runtime::spawn_blocking(crate::core::audio_output::get_audio_output_state)
            .await
            .map_err(|error| {
                format!("set_audio_output_device_state refresh failed: {}", error)
            })??;
    log::info!(
        "audio output state updated: id={} state={:?}",
        id,
        device_state
    );
    Ok(crate::core::audio_output::apply_preferences(
        output_state,
        &saved_preferences,
    ))
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
