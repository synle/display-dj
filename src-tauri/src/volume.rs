#[tauri::command]
pub fn get_volume() -> Result<u32, String> {
    get_system_volume()
}

#[tauri::command]
pub fn set_volume(value: u32) -> Result<(), String> {
    set_system_volume(value.min(100))
}

// ===========================================================================
// macOS: osascript (AppleScript -> CoreAudio)
// ===========================================================================

#[cfg(target_os = "macos")]
fn get_system_volume() -> Result<u32, String> {
    let output = std::process::Command::new("osascript")
        .args(["-e", "output volume of (get volume settings)"])
        .output()
        .map_err(|e| format!("Failed to get volume: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("Failed to parse volume: {}", e))
}

#[cfg(target_os = "macos")]
fn set_system_volume(value: u32) -> Result<(), String> {
    let script = format!("set volume output volume {}", value);
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| format!("Failed to set volume: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

// ===========================================================================
// Windows: PowerShell + WASAPI COM interop
// ===========================================================================

#[cfg(target_os = "windows")]
const AUDIO_TYPE_DEF: &str = r#"
using System;
using System.Runtime.InteropServices;

[Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IAudioEndpointVolume {
    int NotImpl1(); int NotImpl2(); int NotImpl3(); int NotImpl4();
    int NotImpl5(); int NotImpl6(); int NotImpl7();
    int SetMasterVolumeLevelScalar(float fLevel, System.Guid pguidEventContext);
    int NotImpl8();
    int GetMasterVolumeLevelScalar(out float pfLevel);
    int NotImpl9();
    int SetMute([MarshalAs(UnmanagedType.Bool)] bool bMute, System.Guid pguidEventContext);
    int GetMute(out bool pbMute);
}

[Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IMMDevice {
    int Activate(ref System.Guid iid, int dwClsCtx, IntPtr pActivationParams,
                 [MarshalAs(UnmanagedType.IUnknown)] out object ppInterface);
}

[Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IMMDeviceEnumerator {
    int EnumAudioEndpoints(int dataFlow, int dwStateMask, out IntPtr ppDevices);
    int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice ppEndpoint);
}

[ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")]
class MMDeviceEnumeratorComObject { }

public class Audio {
    static IAudioEndpointVolume Vol() {
        var enumerator = new MMDeviceEnumeratorComObject() as IMMDeviceEnumerator;
        IMMDevice dev = null;
        Marshal.ThrowExceptionForHR(enumerator.GetDefaultAudioEndpoint(0, 1, out dev));
        Guid epvid = typeof(IAudioEndpointVolume).GUID;
        object o;
        Marshal.ThrowExceptionForHR(dev.Activate(ref epvid, 23, IntPtr.Zero, out o));
        return (IAudioEndpointVolume)o;
    }
    public static float Volume {
        get {
            float v = -1;
            Marshal.ThrowExceptionForHR(Vol().GetMasterVolumeLevelScalar(out v));
            return v;
        }
        set {
            Marshal.ThrowExceptionForHR(Vol().SetMasterVolumeLevelScalar(value, System.Guid.Empty));
        }
    }
}
"#;

#[cfg(target_os = "windows")]
fn powershell_hidden(ps_command: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", ps_command])
        .creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(target_os = "windows")]
fn get_system_volume() -> Result<u32, String> {
    let ps_script = format!(
        "Add-Type -TypeDefinition @'\n{}\n'@\n[Math]::Round([Audio]::Volume * 100)",
        AUDIO_TYPE_DEF
    );

    let output = powershell_hidden(&ps_script)
        .output()
        .map_err(|e| format!("Failed to get volume: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "PowerShell volume get failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("Failed to parse volume: {}", e))
}

#[cfg(target_os = "windows")]
fn set_system_volume(value: u32) -> Result<(), String> {
    let ps_script = format!(
        "Add-Type -TypeDefinition @'\n{}\n'@\n[Audio]::Volume = {:.2}",
        AUDIO_TYPE_DEF,
        value as f64 / 100.0
    );

    let output = powershell_hidden(&ps_script)
        .output()
        .map_err(|e| format!("Failed to set volume: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "PowerShell volume set failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

// ===========================================================================
// Linux: pactl (PulseAudio / PipeWire compatible)
// ===========================================================================

#[cfg(target_os = "linux")]
fn get_system_volume() -> Result<u32, String> {
    let output = std::process::Command::new("pactl")
        .args(["get-sink-volume", "@DEFAULT_SINK@"])
        .output()
        .map_err(|e| format!("pactl not found: {}. Install pulseaudio-utils or pipewire-pulse.", e))?;

    if !output.status.success() {
        return Err(format!(
            "pactl get volume failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output: "Volume: front-left: 32768 /  50% / -18.06 dB,   front-right: ..."
    for part in stdout.split('/') {
        let part = part.trim();
        if part.ends_with('%') {
            if let Ok(vol) = part.trim_end_matches('%').trim().parse::<u32>() {
                return Ok(vol.min(100));
            }
        }
    }
    Err("Failed to parse pactl volume output".into())
}

#[cfg(target_os = "linux")]
fn set_system_volume(value: u32) -> Result<(), String> {
    let output = std::process::Command::new("pactl")
        .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", value)])
        .output()
        .map_err(|e| format!("pactl not found: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "pactl set volume failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
