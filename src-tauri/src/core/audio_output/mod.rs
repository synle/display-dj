//! Cross-platform system audio-output enumeration and selection.

use serde::{Deserialize, Serialize};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Display DJ availability state for one selectable playback endpoint.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum AudioOutputDeviceState {
    #[default]
    Enabled,
    Disabled,
    Hidden,
}

/// Playback endpoint exposed to the frontend.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutputDevice {
    /// Stable platform identifier used for selection and persisted aliases.
    pub id: String,
    /// Display DJ label after applying a saved alias.
    pub name: String,
    /// Native operating-system endpoint name.
    pub original_name: String,
    /// User-controlled availability in Display DJ.
    #[serde(default)]
    pub state: AudioOutputDeviceState,
    /// Whether the operating system identifies this as an integrated output.
    #[serde(default)]
    pub is_built_in: bool,
}

/// Current playback endpoints and the operating system's selected default.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutputState {
    pub devices: Vec<AudioOutputDevice>,
    pub selected_device_id: Option<String>,
}

/// Returns current playback endpoints and selected system default.
pub fn get_audio_output_state() -> Result<AudioOutputState, String> {
    #[cfg(target_os = "macos")]
    let state = macos::get_audio_output_state()?;
    #[cfg(target_os = "windows")]
    let state = windows::get_audio_output_state()?;
    #[cfg(target_os = "linux")]
    let state = linux::get_audio_output_state()?;

    Ok(sort_audio_output_state(state))
}

/// Verifies that an exact stable audio-output identifier currently exists.
pub fn ensure_device_exists(device_id: &str) -> Result<(), String> {
    let device_id = device_id.trim();
    if device_id.is_empty() {
        return Err("audio output device id cannot be empty".into());
    }
    let state = get_audio_output_state()?;
    resolve_device(&state.devices, device_id)?;
    Ok(())
}

/// Selects a playback endpoint by exact stable identifier and returns fresh state.
pub fn set_audio_output_device(device_id: &str) -> Result<AudioOutputState, String> {
    let device_id = device_id.trim();
    if device_id.is_empty() {
        return Err("audio output device id cannot be empty".into());
    }

    let current = get_audio_output_state()?;
    let device = resolve_device(&current.devices, device_id)?;
    if device.state != AudioOutputDeviceState::Enabled {
        return Err(format!("audio output device is not enabled: {}", device_id));
    }

    #[cfg(target_os = "macos")]
    macos::set_audio_output_device(device_id)?;
    #[cfg(target_os = "windows")]
    windows::set_audio_output_device(device_id)?;
    #[cfg(target_os = "linux")]
    linux::set_audio_output_device(device_id)?;

    get_audio_output_state()
}

/// Returns volume and mute state for the selected Windows playback endpoint.
#[cfg(target_os = "windows")]
pub fn get_default_volume() -> Result<(u32, bool), String> {
    windows::get_default_volume()
}

/// Sets volume for the selected Windows playback endpoint.
#[cfg(target_os = "windows")]
pub fn set_default_volume(level: u16) -> Result<(), String> {
    windows::set_default_volume(level)
}

/// Sets mute state for the selected Windows playback endpoint.
#[cfg(target_os = "windows")]
pub fn set_default_mute(mute: bool) -> Result<(), String> {
    windows::set_default_mute(mute)
}

/// Applies saved labels and availability while preserving native endpoint data.
pub fn apply_preferences(
    mut state: AudioOutputState,
    preferences: &[(String, String, AudioOutputDeviceState)],
) -> AudioOutputState {
    for device in &mut state.devices {
        if let Some((_, label, device_state)) =
            preferences.iter().find(|(id, _, _)| id == &device.id)
        {
            if !label.trim().is_empty() {
                device.name = label.clone();
            }
            device.state = *device_state;
        }
    }
    sort_audio_output_state(state)
}

/// Finds an endpoint by exact stable identifier.
fn resolve_device<'a>(
    devices: &'a [AudioOutputDevice],
    device_id: &str,
) -> Result<&'a AudioOutputDevice, String> {
    devices
        .iter()
        .find(|device| device.id == device_id)
        .ok_or_else(|| format!("audio output device not found: {}", device_id))
}

/// Keeps polling snapshots stable across platform enumeration order changes.
fn sort_audio_output_state(mut state: AudioOutputState) -> AudioOutputState {
    let selected_device_id = state.selected_device_id.as_deref();
    state.devices.sort_by(|left, right| {
        let left_selected = Some(left.id.as_str()) == selected_device_id;
        let right_selected = Some(right.id.as_str()) == selected_device_id;
        left.state
            .cmp(&right.state)
            .then(right.is_built_in.cmp(&left.is_built_in))
            .then(right_selected.cmp(&left_selected))
            .then(left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then(left.id.cmp(&right.id))
    });
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a playback endpoint for pure shared-helper tests.
    fn device(id: &str, name: &str) -> AudioOutputDevice {
        AudioOutputDevice {
            id: id.into(),
            name: name.into(),
            original_name: name.into(),
            state: AudioOutputDeviceState::Enabled,
            is_built_in: false,
        }
    }

    /// Audio output state serializes with the frontend's camelCase fields.
    #[test]
    fn audio_output_state_uses_camel_case() {
        let state = AudioOutputState {
            devices: vec![device("speaker", "Speakers")],
            selected_device_id: Some("speaker".into()),
        };

        let json = serde_json::to_string(&state).unwrap();

        assert!(json.contains("\"originalName\":\"Speakers\""));
        assert!(json.contains("\"state\":\"enabled\""));
        assert!(json.contains("\"isBuiltIn\":false"));
        assert!(json.contains("\"selectedDeviceId\":\"speaker\""));
        assert_eq!(
            serde_json::from_str::<AudioOutputState>(&json).unwrap(),
            state
        );
    }

    /// Enabled built-in outputs lead disabled and hidden outputs deterministically.
    #[test]
    fn sorts_by_state_built_in_selected_name_and_stable_id() {
        let mut built_in = device("built-in", "MacBook Pro Speakers");
        built_in.is_built_in = true;
        let mut disabled = device("teams", "Microsoft Teams Audio");
        disabled.state = AudioOutputDeviceState::Disabled;
        let mut hidden = device("zoom", "ZoomAudioDevice");
        hidden.state = AudioOutputDeviceState::Hidden;
        let state = sort_audio_output_state(AudioOutputState {
            devices: vec![hidden, disabled, device("dock", "TYPEC"), built_in],
            selected_device_id: Some("dock".into()),
        });

        assert_eq!(
            state
                .devices
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["built-in", "dock", "teams", "zoom"]
        );
    }

    /// Selection keys use exact IDs even when human-readable names collide.
    #[test]
    fn resolves_duplicate_names_by_exact_id() {
        let devices = vec![device("left", "Speakers"), device("right", "Speakers")];

        assert_eq!(resolve_device(&devices, "right").unwrap().id, "right");
        assert!(resolve_device(&devices, "RIGHT").is_err());
    }

    /// Saved output preferences replace labels and state while retaining native names.
    #[test]
    fn applies_non_empty_output_preferences() {
        let state = apply_preferences(
            AudioOutputState {
                devices: vec![device("speaker", "MacBook Pro Speakers")],
                selected_device_id: Some("speaker".into()),
            },
            &[(
                "speaker".into(),
                "Desk".into(),
                AudioOutputDeviceState::Disabled,
            )],
        );

        assert_eq!(state.devices[0].name, "Desk");
        assert_eq!(state.devices[0].original_name, "MacBook Pro Speakers");
        assert_eq!(state.devices[0].state, AudioOutputDeviceState::Disabled);
    }

    /// Empty saved labels retain the native endpoint name while applying state.
    #[test]
    fn ignores_empty_labels() {
        let state = apply_preferences(
            AudioOutputState {
                devices: vec![device("speaker", "MacBook Pro Speakers")],
                selected_device_id: Some("speaker".into()),
            },
            &[(
                "speaker".into(),
                "  ".into(),
                AudioOutputDeviceState::Hidden,
            )],
        );

        assert_eq!(state.devices[0].name, "MacBook Pro Speakers");
        assert_eq!(state.devices[0].state, AudioOutputDeviceState::Hidden);
    }
}
