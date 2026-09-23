//! Linux PulseAudio/PipeWire playback-endpoint support through `pactl`.

use super::{AudioOutputDevice, AudioOutputState};
use serde::Deserialize;
use std::process::{Command, Output};

#[derive(Deserialize)]
struct PactlSink {
    name: String,
    description: Option<String>,
}

/// Enumerates PulseAudio/PipeWire sinks and the current default.
pub fn get_audio_output_state() -> Result<AudioOutputState, String> {
    let sinks_output = run_pactl(&["--format=json", "list", "sinks"])?;
    let devices = parse_sinks(&sinks_output.stdout)?;
    let selected_device_id = read_default_sink().ok();

    Ok(AudioOutputState {
        devices,
        selected_device_id,
    })
}

/// Sets the default sink and best-effort moves current playback streams.
pub fn set_audio_output_device(device_id: &str) -> Result<(), String> {
    let output = run_pactl(&["set-default-sink", device_id])?;
    require_success(output, "set default audio output")?;

    if let Ok(input_output) = run_pactl(&["list", "short", "sink-inputs"]) {
        for input_id in parse_sink_input_ids(&input_output.stdout) {
            match run_pactl(&["move-sink-input", &input_id, device_id]) {
                Ok(move_output) if move_output.status.success() => {}
                Ok(move_output) => log::warn!(
                    "move-sink-input {} failed: {}",
                    input_id,
                    String::from_utf8_lossy(&move_output.stderr).trim()
                ),
                Err(error) => log::warn!("move-sink-input {} failed: {}", input_id, error),
            }
        }
    }

    Ok(())
}

/// Executes `pactl` without involving a shell.
fn run_pactl(args: &[&str]) -> Result<Output, String> {
    Command::new("pactl")
        .args(args)
        .output()
        .map_err(|error| format!("pactl is unavailable: {}", error))
}

/// Requires successful process completion and includes stderr on failure.
fn require_success(output: Output, operation: &str) -> Result<Output, String> {
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "{} failed: {}",
            operation,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Parses the JSON sink subset needed by the frontend.
fn parse_sinks(bytes: &[u8]) -> Result<Vec<AudioOutputDevice>, String> {
    let sinks: Vec<PactlSink> = serde_json::from_slice(bytes)
        .map_err(|error| format!("failed to parse pactl sinks: {}", error))?;
    Ok(sinks
        .into_iter()
        .filter(|sink| !sink.name.trim().is_empty())
        .map(|sink| {
            let name = sink
                .description
                .filter(|description| !description.trim().is_empty())
                .unwrap_or_else(|| sink.name.clone());
            AudioOutputDevice {
                id: sink.name,
                name: name.clone(),
                original_name: name,
            }
        })
        .collect())
}

/// Reads the default sink with compatibility fallback for older `pactl`.
fn read_default_sink() -> Result<String, String> {
    if let Ok(output) = run_pactl(&["get-default-sink"]) {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !value.is_empty() {
                return Ok(value);
            }
        }
    }

    let output = require_success(run_pactl(&["info"])?, "read default audio output")?;
    parse_default_sink_info(&output.stdout)
        .ok_or_else(|| "pactl info did not report a default sink".into())
}

/// Parses `Default Sink: <name>` from `pactl info`.
fn parse_default_sink_info(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes).lines().find_map(|line| {
        line.strip_prefix("Default Sink:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

/// Parses sink-input numeric IDs from `pactl list short sink-inputs`.
fn parse_sink_input_ids(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|value| value.parse::<u32>().is_ok())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sink JSON keeps stable names as IDs and descriptions as labels.
    #[test]
    fn parses_sink_json() {
        let devices = parse_sinks(
            br#"[
                {"name":"alsa_output.usb","description":"USB Speakers","extra":true},
                {"name":"bluez_output.headset","description":null}
            ]"#,
        )
        .unwrap();

        assert_eq!(
            devices,
            vec![
                AudioOutputDevice {
                    id: "alsa_output.usb".into(),
                    name: "USB Speakers".into(),
                    original_name: "USB Speakers".into(),
                },
                AudioOutputDevice {
                    id: "bluez_output.headset".into(),
                    name: "bluez_output.headset".into(),
                    original_name: "bluez_output.headset".into(),
                },
            ]
        );
    }

    /// Malformed sink JSON fails explicitly.
    #[test]
    fn rejects_malformed_sink_json() {
        assert!(parse_sinks(b"not-json").is_err());
    }

    /// Older pactl info output yields the default sink name.
    #[test]
    fn parses_default_sink_from_info() {
        assert_eq!(
            parse_default_sink_info(b"Server Name: PulseAudio\nDefault Sink: desk\n"),
            Some("desk".into())
        );
    }

    /// Sink-input parsing ignores malformed lines.
    #[test]
    fn parses_sink_input_ids() {
        assert_eq!(
            parse_sink_input_ids(b"42\t1\t2\nbad line\n9\t1\t2\n"),
            vec!["42", "9"]
        );
    }
}
