// =========================================================================
// Volume control — adjusts the default/currently-selected audio output.
// Cross-platform: macOS (osascript), Windows (PowerShell), Linux (pactl/amixer).
// Vendored from display-dj-cli main.rs.
// =========================================================================

use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
use super::win_cmd::hidden_command;

/// System audio volume state.
#[derive(Serialize, Deserialize, Clone)]
pub struct VolumeInfo {
    pub volume: u32, // 0-100 percentage
    pub muted: bool, // true if the default output is muted
}

// --- macOS volume: osascript wrapping AppleScript commands ---

/// Get current volume and mute state on macOS via `osascript`.
/// Makes two osascript calls: one for volume level, one for mute state.
#[cfg(target_os = "macos")]
pub fn get_volume() -> Option<VolumeInfo> {
    let output = std::process::Command::new("osascript")
        .args(["-e", "output volume of (get volume settings)"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let volume: u32 = String::from_utf8_lossy(&output.stdout).trim().parse().ok()?;

    let output = std::process::Command::new("osascript")
        .args(["-e", "output muted of (get volume settings)"])
        .output().ok()?;
    let muted = String::from_utf8_lossy(&output.stdout).trim().to_lowercase() == "true";

    Some(VolumeInfo { volume, muted })
}

/// Set system volume on macOS via osascript. Level is 0-100.
#[cfg(target_os = "macos")]
pub fn set_volume(level: u16) -> bool {
    std::process::Command::new("osascript")
        .args(["-e", &format!("set volume output volume {}", level)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Toggle mute on macOS via osascript.
#[cfg(target_os = "macos")]
pub fn set_mute(mute: bool) -> bool {
    let val = if mute { "true" } else { "false" };
    std::process::Command::new("osascript")
        .args(["-e", &format!("set volume output muted {}", val)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- Windows volume: AudioDeviceCmdlets PowerShell module ---
// Requires one-time setup: Install-Module -Name AudioDeviceCmdlets
// https://www.powershellgallery.com/packages/AudioDeviceCmdlets

/// Get current volume and mute state on Windows via AudioDeviceCmdlets PowerShell module.
/// Reads both playback volume and mute state in a single PowerShell invocation.
#[cfg(target_os = "windows")]
pub fn get_volume() -> Option<VolumeInfo> {
    let output = hidden_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "Import-Module AudioDeviceCmdlets; $v = Get-AudioDevice -PlaybackVolume; $m = Get-AudioDevice -PlaybackMute; Write-Output \"$v,$m\""])
        .output().ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut parts = stdout.split(',');
    let volume: f64 = parts.next()?.parse().ok()?;
    let muted = parts.next()?.trim().to_lowercase() == "true";
    Some(VolumeInfo { volume: volume.round() as u32, muted })
}

/// Set system volume on Windows via AudioDeviceCmdlets. Level is 0-100.
#[cfg(target_os = "windows")]
pub fn set_volume(level: u16) -> bool {
    hidden_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command",
            &format!("Import-Module AudioDeviceCmdlets; Set-AudioDevice -PlaybackVolume {}", level)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Toggle mute on Windows via AudioDeviceCmdlets. Uses 1/0 instead of true/false.
#[cfg(target_os = "windows")]
pub fn set_mute(mute: bool) -> bool {
    let val = if mute { "1" } else { "0" };
    hidden_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command",
            &format!("Import-Module AudioDeviceCmdlets; Set-AudioDevice -PlaybackMute {}", val)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- Linux volume: pactl (PulseAudio/PipeWire) with amixer (ALSA) fallback ---

/// Get current volume on Linux. Tries pactl (PulseAudio/PipeWire) first,
/// falls back to amixer (raw ALSA) for minimal setups without PulseAudio.
#[cfg(target_os = "linux")]
pub fn get_volume() -> Option<VolumeInfo> {
    // Try pactl first (PulseAudio / PipeWire)
    if let Some(info) = get_volume_pactl() { return Some(info); }
    // Fallback to amixer (ALSA)
    get_volume_amixer()
}

/// Get volume via pactl (PulseAudio/PipeWire). Parses the percentage from
/// pactl's output format: "Volume: front-left: 32768 /  50% / -17.50 dB".
#[cfg(target_os = "linux")]
fn get_volume_pactl() -> Option<VolumeInfo> {
    let output = std::process::Command::new("pactl")
        .args(["get-sink-volume", "@DEFAULT_SINK@"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "Volume: front-left: 32768 /  50% / ..."
    let volume = stdout.split('/')
        .find(|s| s.contains('%'))
        .and_then(|s| s.trim().trim_end_matches('%').parse::<u32>().ok())?;

    let mute_output = std::process::Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output().ok()?;
    let muted = String::from_utf8_lossy(&mute_output.stdout)
        .to_lowercase().contains("yes");

    Some(VolumeInfo { volume, muted })
}

/// Get volume via amixer (ALSA fallback). Parses "[75%]" and "[on]/[off]" from output.
#[cfg(target_os = "linux")]
fn get_volume_amixer() -> Option<VolumeInfo> {
    let output = std::process::Command::new("amixer")
        .args(["get", "Master"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let volume = stdout.split('[')
        .find(|s| s.contains("%]"))
        .and_then(|s| s.split('%').next())
        .and_then(|s| s.parse::<u32>().ok())?;
    let muted = stdout.contains("[off]");
    Some(VolumeInfo { volume, muted })
}

/// Set volume on Linux. Tries pactl first, falls back to amixer.
#[cfg(target_os = "linux")]
pub fn set_volume(level: u16) -> bool {
    // Try pactl first
    if std::process::Command::new("pactl")
        .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", level)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    { return true; }
    // Fallback to amixer
    std::process::Command::new("amixer")
        .args(["set", "Master", &format!("{}%", level)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Toggle mute on Linux. Tries pactl first, falls back to amixer.
#[cfg(target_os = "linux")]
pub fn set_mute(mute: bool) -> bool {
    let val = if mute { "1" } else { "0" };
    // Try pactl
    if std::process::Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", val])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    { return true; }
    // Fallback to amixer
    let toggle = if mute { "mute" } else { "unmute" };
    std::process::Command::new("amixer")
        .args(["set", "Master", toggle])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
