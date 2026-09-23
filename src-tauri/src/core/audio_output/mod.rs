//! Cross-platform system audio-output enumeration and selection.

use serde::{Deserialize, Serialize};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

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
    resolve_device(&current.devices, device_id)?;

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

/// Applies saved aliases while preserving each native endpoint name.
pub fn apply_aliases(
    mut state: AudioOutputState,
    aliases: &[(String, String)],
) -> AudioOutputState {
    for device in &mut state.devices {
        if let Some((_, label)) = aliases
            .iter()
            .find(|(id, label)| id == &device.id && !label.trim().is_empty())
        {
            device.name = label.clone();
        }
    }
    state
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
    state.devices.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
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
        assert!(json.contains("\"selectedDeviceId\":\"speaker\""));
        assert_eq!(
            serde_json::from_str::<AudioOutputState>(&json).unwrap(),
            state
        );
    }

    /// Endpoint ordering stays deterministic when platform order changes.
    #[test]
    fn sorts_by_name_then_stable_id() {
        let state = sort_audio_output_state(AudioOutputState {
            devices: vec![
                device("z", "Speakers"),
                device("a", "speakers"),
                device("b", "AirPods"),
            ],
            selected_device_id: None,
        });

        assert_eq!(
            state
                .devices
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "a", "z"]
        );
    }

    /// Selection keys use exact IDs even when human-readable names collide.
    #[test]
    fn resolves_duplicate_names_by_exact_id() {
        let devices = vec![device("left", "Speakers"), device("right", "Speakers")];

        assert_eq!(resolve_device(&devices, "right").unwrap().id, "right");
        assert!(resolve_device(&devices, "RIGHT").is_err());
    }

    /// Saved aliases replace only display names and retain native names.
    #[test]
    fn applies_non_empty_aliases() {
        let state = apply_aliases(
            AudioOutputState {
                devices: vec![device("speaker", "MacBook Pro Speakers")],
                selected_device_id: Some("speaker".into()),
            },
            &[("speaker".into(), "Desk".into())],
        );

        assert_eq!(state.devices[0].name, "Desk");
        assert_eq!(state.devices[0].original_name, "MacBook Pro Speakers");
    }

    /// Empty aliases fall back to the native endpoint name.
    #[test]
    fn ignores_empty_aliases() {
        let state = apply_aliases(
            AudioOutputState {
                devices: vec![device("speaker", "MacBook Pro Speakers")],
                selected_device_id: Some("speaker".into()),
            },
            &[("speaker".into(), "  ".into())],
        );

        assert_eq!(state.devices[0].name, "MacBook Pro Speakers");
    }
}
