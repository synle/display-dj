use serde::{Deserialize, Serialize};
#[cfg(not(target_os = "windows"))]
use std::path::PathBuf;

/// Create a PowerShell command that runs without a visible console window.
#[cfg(target_os = "windows")]
fn powershell_hidden(ps_command: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", ps_command])
        .creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub brightness: u32,
    pub contrast: u32,
    pub supports_brightness: bool,
    pub supports_contrast: bool,
    pub is_built_in: bool,
}

// ===========================================================================
// macOS implementation
// ===========================================================================

#[cfg(target_os = "macos")]
fn get_binary_path(name: &str) -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();

    // 1. Next to executable (production bundle)
    let beside_exe = exe_dir.join(name);
    if beside_exe.exists() {
        return beside_exe;
    }

    // 2. macOS .app bundle Resources directory
    let resources = exe_dir.join("../Resources").join(name);
    if resources.exists() {
        return resources;
    }

    // 3. Homebrew (Apple Silicon)
    let homebrew_arm = PathBuf::from(format!("/opt/homebrew/bin/{}", name));
    if homebrew_arm.exists() {
        return homebrew_arm;
    }

    // 4. Homebrew (Intel)
    let homebrew_intel = PathBuf::from(format!("/usr/local/bin/{}", name));
    if homebrew_intel.exists() {
        return homebrew_intel;
    }

    // 5. Fallback: hope it's in PATH
    PathBuf::from(name)
}

#[cfg(target_os = "macos")]
fn is_apple_silicon() -> bool {
    std::env::consts::ARCH == "aarch64"
}

#[cfg(target_os = "macos")]
fn detect_monitors() -> Vec<Monitor> {
    let mut monitors: Vec<Monitor> = Vec::new();

    if let Some(builtin) = detect_builtin_monitor_macos() {
        monitors.push(builtin);
    }

    if is_apple_silicon() {
        detect_external_monitors_m1ddc(&mut monitors);
    } else {
        detect_external_monitors_ddcctl(&mut monitors);
    }

    monitors
}

#[cfg(target_os = "macos")]
fn detect_builtin_monitor_macos() -> Option<Monitor> {
    let brightness_bin = get_binary_path("brightness");
    let output = std::process::Command::new(&brightness_bin)
        .arg("-l")
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("display 0:") {
            let brightness = line
                .split("brightness")
                .nth(1)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .map(|v| (v * 100.0).round() as u32)
                .unwrap_or(50);

            return Some(Monitor {
                id: "builtin-0".into(),
                name: "Built-in Display".into(),
                brightness,
                contrast: 50,
                supports_brightness: true,
                supports_contrast: false,
                is_built_in: true,
            });
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn detect_external_monitors_m1ddc(monitors: &mut Vec<Monitor>) {
    let m1ddc = get_binary_path("m1ddc");

    let list_output = match std::process::Command::new(&m1ddc)
        .args(["display", "list"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log::warn!("Failed to run m1ddc display list: {}", e);
            return;
        }
    };

    let stdout = String::from_utf8_lossy(&list_output.stdout);
    let mut display_numbers: Vec<u32> = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(num_str) = line.split_whitespace().next() {
            if let Ok(num) = num_str.parse::<u32>() {
                display_numbers.push(num);
            }
        }
    }

    if display_numbers.is_empty() {
        display_numbers = vec![1, 2, 3, 4];
    }

    for display_num in display_numbers {
        let brightness = get_m1ddc_value(&m1ddc, "luminance", display_num);
        let contrast = get_m1ddc_value(&m1ddc, "contrast", display_num);

        let brightness = match brightness {
            Some(v) => v,
            None => continue,
        };

        let name = get_m1ddc_display_name(&m1ddc, display_num)
            .unwrap_or_else(|| format!("External Display {}", display_num));

        monitors.push(Monitor {
            id: format!("external-{}", display_num),
            name,
            brightness,
            contrast: contrast.unwrap_or(50),
            supports_brightness: true,
            supports_contrast: contrast.is_some(),
            is_built_in: false,
        });
    }
}

#[cfg(target_os = "macos")]
fn get_m1ddc_value(m1ddc: &PathBuf, property: &str, display_num: u32) -> Option<u32> {
    let output = std::process::Command::new(m1ddc)
        .args(["get", property, "-d", &display_num.to_string()])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .ok()
}

#[cfg(target_os = "macos")]
fn get_m1ddc_display_name(m1ddc: &PathBuf, display_num: u32) -> Option<String> {
    let output = std::process::Command::new(m1ddc)
        .args(["display", "list"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with(&display_num.to_string()) {
            if let Some(rest) = line.splitn(2, " - ").nth(1) {
                let name = if let Some(idx) = rest.rfind('(') {
                    rest[..idx].trim()
                } else {
                    rest.trim()
                };
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn detect_external_monitors_ddcctl(monitors: &mut Vec<Monitor>) {
    let ddcctl = get_binary_path("ddcctl");

    for display_num in 1..=4 {
        let brightness = get_ddcctl_value(&ddcctl, "-b", display_num);
        let brightness = match brightness {
            Some(v) => v,
            None => continue,
        };

        let contrast = get_ddcctl_value(&ddcctl, "-con", display_num);

        monitors.push(Monitor {
            id: format!("external-{}", display_num),
            name: format!("External Display {}", display_num),
            brightness,
            contrast: contrast.unwrap_or(50),
            supports_brightness: true,
            supports_contrast: contrast.is_some(),
            is_built_in: false,
        });
    }
}

#[cfg(target_os = "macos")]
fn get_ddcctl_value(ddcctl: &PathBuf, flag: &str, display_num: u32) -> Option<u32> {
    let output = std::process::Command::new(ddcctl)
        .args(["-d", &display_num.to_string(), flag, "?"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("current:") {
            if let Some(val_str) = line.split("current:").nth(1) {
                let val_str = val_str.split(',').next().unwrap_or("").trim();
                return val_str.parse::<u32>().ok();
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn set_monitor_brightness(monitor_id: &str, value: u32) -> Result<(), String> {
    let value = value.min(100);

    if monitor_id.starts_with("builtin") {
        let brightness_bin = get_binary_path("brightness");
        let float_val = value as f64 / 100.0;
        let output = std::process::Command::new(&brightness_bin)
            .args(["-d", "0", &format!("{:.4}", float_val)])
            .output()
            .map_err(|e| format!("Failed to set built-in brightness: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "brightness set failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        return Ok(());
    }

    let display_num = extract_display_number(monitor_id)?;

    if is_apple_silicon() {
        let m1ddc = get_binary_path("m1ddc");
        let output = std::process::Command::new(&m1ddc)
            .args(["set", "luminance", &value.to_string(), "-d", &display_num.to_string()])
            .output()
            .map_err(|e| format!("m1ddc error: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "m1ddc set brightness failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    } else {
        let ddcctl = get_binary_path("ddcctl");
        let output = std::process::Command::new(&ddcctl)
            .args(["-d", &display_num.to_string(), "-b", &value.to_string()])
            .output()
            .map_err(|e| format!("ddcctl error: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "ddcctl set brightness failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_monitor_contrast(monitor_id: &str, value: u32) -> Result<(), String> {
    let value = value.min(100);

    if monitor_id.starts_with("builtin") {
        return Err("Built-in display does not support contrast control".into());
    }

    let display_num = extract_display_number(monitor_id)?;

    if is_apple_silicon() {
        let m1ddc = get_binary_path("m1ddc");
        let output = std::process::Command::new(&m1ddc)
            .args(["set", "contrast", &value.to_string(), "-d", &display_num.to_string()])
            .output()
            .map_err(|e| format!("m1ddc error: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "m1ddc set contrast failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    } else {
        let ddcctl = get_binary_path("ddcctl");
        let output = std::process::Command::new(&ddcctl)
            .args(["-d", &display_num.to_string(), "-con", &value.to_string()])
            .output()
            .map_err(|e| format!("ddcctl error: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "ddcctl set contrast failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_all_monitors_brightness(value: u32) -> Result<(), String> {
    let monitors = detect_monitors();
    let mut errors: Vec<String> = Vec::new();
    for m in &monitors {
        if m.supports_brightness {
            if let Err(e) = set_monitor_brightness(&m.id, value) {
                errors.push(format!("{}: {}", m.name, e));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

#[cfg(target_os = "macos")]
fn set_all_monitors_contrast(value: u32) -> Result<(), String> {
    let monitors = detect_monitors();
    let mut errors: Vec<String> = Vec::new();
    for m in &monitors {
        if m.supports_contrast {
            if let Err(e) = set_monitor_contrast(&m.id, value) {
                errors.push(format!("{}: {}", m.name, e));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

// ===========================================================================
// Windows implementation
// ===========================================================================

#[cfg(target_os = "windows")]
fn detect_monitors() -> Vec<Monitor> {
    let mut monitors: Vec<Monitor> = Vec::new();

    if let Some(builtin) = detect_builtin_monitor_windows() {
        monitors.push(builtin);
    }

    detect_external_monitors_win32(&mut monitors);

    monitors
}

#[cfg(target_os = "windows")]
fn detect_builtin_monitor_windows() -> Option<Monitor> {
    let output = powershell_hidden(
            "(Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightness -ErrorAction SilentlyContinue).CurrentBrightness",
        )
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let brightness = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .ok()?;

    Some(Monitor {
        id: "builtin-0".into(),
        name: "Built-in Display".into(),
        brightness,
        contrast: 50,
        supports_brightness: true,
        supports_contrast: false,
        is_built_in: true,
    })
}

#[cfg(target_os = "windows")]
fn detect_external_monitors_win32(monitors: &mut Vec<Monitor>) {
    use windows::Win32::Devices::Display::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;

    unsafe {
        let mut hmonitors: Vec<HMONITOR> = Vec::new();

        unsafe extern "system" fn enum_proc(
            hmonitor: HMONITOR,
            _hdc: HDC,
            _lprect: *mut RECT,
            lparam: LPARAM,
        ) -> BOOL {
            let monitors = &mut *(lparam.0 as *mut Vec<HMONITOR>);
            monitors.push(hmonitor);
            BOOL(1)
        }

        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut hmonitors as *mut Vec<HMONITOR> as isize),
        );

        let mut ext_index = 0u32;
        for hmonitor in hmonitors {
            let mut physical_count: u32 = 0;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(hmonitor, &mut physical_count).is_err()
                || physical_count == 0
            {
                continue;
            }

            let mut physical_monitors = vec![
                PHYSICAL_MONITOR {
                    hPhysicalMonitor: HANDLE::default(),
                    szPhysicalMonitorDescription: [0u16; 128],
                };
                physical_count as usize
            ];

            if GetPhysicalMonitorsFromHMONITOR(hmonitor, &mut physical_monitors).is_err() {
                continue;
            }

            for pm in &physical_monitors {
                ext_index += 1;

                let desc_buf = pm.szPhysicalMonitorDescription;
                let description = String::from_utf16_lossy(&desc_buf)
                    .trim_end_matches('\0')
                    .to_string();

                let name = if description.is_empty() {
                    format!("External Display {}", ext_index)
                } else {
                    description
                };

                let mut min_b: u32 = 0;
                let mut cur_b: u32 = 50;
                let mut max_b: u32 = 100;
                let supports_brightness =
                    GetMonitorBrightness(pm.hPhysicalMonitor, &mut min_b, &mut cur_b, &mut max_b)
                        != 0;

                let mut min_c: u32 = 0;
                let mut cur_c: u32 = 50;
                let mut max_c: u32 = 100;
                let supports_contrast =
                    GetMonitorContrast(pm.hPhysicalMonitor, &mut min_c, &mut cur_c, &mut max_c)
                        != 0;

                let brightness = if supports_brightness && max_b > min_b {
                    ((cur_b - min_b) as f64 / (max_b - min_b) as f64 * 100.0).round() as u32
                } else {
                    50
                };

                let contrast = if supports_contrast && max_c > min_c {
                    ((cur_c - min_c) as f64 / (max_c - min_c) as f64 * 100.0).round() as u32
                } else {
                    50
                };

                monitors.push(Monitor {
                    id: format!("external-{}", ext_index),
                    name,
                    brightness,
                    contrast,
                    supports_brightness,
                    supports_contrast,
                    is_built_in: false,
                });

                let _ = DestroyPhysicalMonitor(pm.hPhysicalMonitor);
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn set_monitor_brightness(monitor_id: &str, value: u32) -> Result<(), String> {
    let value = value.min(100);

    if monitor_id.starts_with("builtin") {
        let ps_cmd = format!(
            "(Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightnessMethods).WmiSetBrightness(1, {})",
            value
        );
        let output = powershell_hidden(&ps_cmd)
            .output()
            .map_err(|e| format!("Failed to set built-in brightness: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "WMI brightness set failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        return Ok(());
    }

    set_ddc_value_windows(monitor_id, value, true)
}

#[cfg(target_os = "windows")]
fn set_monitor_contrast(monitor_id: &str, value: u32) -> Result<(), String> {
    let value = value.min(100);

    if monitor_id.starts_with("builtin") {
        return Err("Built-in display does not support contrast control".into());
    }

    set_ddc_value_windows(monitor_id, value, false)
}

#[cfg(target_os = "windows")]
fn set_ddc_value_windows(monitor_id: &str, value: u32, is_brightness: bool) -> Result<(), String> {
    use windows::Win32::Devices::Display::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;

    let target_index = extract_display_number(monitor_id)?;

    unsafe {
        let mut hmonitors: Vec<HMONITOR> = Vec::new();

        unsafe extern "system" fn enum_proc(
            hmonitor: HMONITOR,
            _hdc: HDC,
            _lprect: *mut RECT,
            lparam: LPARAM,
        ) -> BOOL {
            let monitors = &mut *(lparam.0 as *mut Vec<HMONITOR>);
            monitors.push(hmonitor);
            BOOL(1)
        }

        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut hmonitors as *mut Vec<HMONITOR> as isize),
        );

        let mut ext_index = 0u32;
        for hmonitor in hmonitors {
            let mut physical_count: u32 = 0;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(hmonitor, &mut physical_count).is_err()
                || physical_count == 0
            {
                continue;
            }

            let mut physical_monitors = vec![
                PHYSICAL_MONITOR {
                    hPhysicalMonitor: HANDLE::default(),
                    szPhysicalMonitorDescription: [0u16; 128],
                };
                physical_count as usize
            ];

            if GetPhysicalMonitorsFromHMONITOR(hmonitor, &mut physical_monitors).is_err() {
                continue;
            }

            for pm in &physical_monitors {
                ext_index += 1;
                if ext_index == target_index {
                    let result = if is_brightness {
                        SetMonitorBrightness(pm.hPhysicalMonitor, value)
                    } else {
                        SetMonitorContrast(pm.hPhysicalMonitor, value)
                    };
                    let _ = DestroyPhysicalMonitor(pm.hPhysicalMonitor);
                    if result != 0 {
                        return Ok(());
                    } else {
                        return Err(format!(
                            "Failed to set {} on {}",
                            if is_brightness { "brightness" } else { "contrast" },
                            monitor_id,
                        ));
                    }
                }
                let _ = DestroyPhysicalMonitor(pm.hPhysicalMonitor);
            }
        }
    }

    Err(format!("Monitor {} not found", monitor_id))
}

#[cfg(target_os = "windows")]
fn set_all_monitors_brightness(value: u32) -> Result<(), String> {
    let monitors = detect_monitors();
    let mut errors: Vec<String> = Vec::new();
    for m in &monitors {
        if m.supports_brightness {
            if let Err(e) = set_monitor_brightness(&m.id, value) {
                errors.push(format!("{}: {}", m.name, e));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

#[cfg(target_os = "windows")]
fn set_all_monitors_contrast(value: u32) -> Result<(), String> {
    let monitors = detect_monitors();
    let mut errors: Vec<String> = Vec::new();
    for m in &monitors {
        if m.supports_contrast {
            if let Err(e) = set_monitor_contrast(&m.id, value) {
                errors.push(format!("{}: {}", m.name, e));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

// ===========================================================================
// Linux implementation (ddcutil + brightnessctl)
// ===========================================================================

#[cfg(target_os = "linux")]
fn detect_monitors() -> Vec<Monitor> {
    let mut monitors: Vec<Monitor> = Vec::new();

    if let Some(builtin) = detect_builtin_monitor_linux() {
        monitors.push(builtin);
    }

    detect_external_monitors_ddcutil(&mut monitors);

    monitors
}

/// Detect built-in laptop display via brightnessctl (reads /sys/class/backlight/).
#[cfg(target_os = "linux")]
fn detect_builtin_monitor_linux() -> Option<Monitor> {
    let output = std::process::Command::new("brightnessctl")
        .args(["-m", "-l", "-c", "backlight"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // machine-readable format: "device,class,current,percentage,max"
    // e.g. "intel_backlight,backlight,500,50%,1000"
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 4 {
            let percentage = parts[3].trim_end_matches('%').parse::<u32>().unwrap_or(50);
            return Some(Monitor {
                id: "builtin-0".into(),
                name: "Built-in Display".into(),
                brightness: percentage,
                contrast: 50,
                supports_brightness: true,
                supports_contrast: false,
                is_built_in: true,
            });
        }
    }
    None
}

/// Detect external monitors via ddcutil (DDC/CI over i2c-dev).
/// Requires: `sudo apt install ddcutil i2c-tools` and user in `i2c` group.
#[cfg(target_os = "linux")]
fn detect_external_monitors_ddcutil(monitors: &mut Vec<Monitor>) {
    let list_output = match std::process::Command::new("ddcutil")
        .args(["detect", "--brief"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log::warn!("ddcutil not found or failed: {}. Install with: sudo apt install ddcutil", e);
            return;
        }
    };

    if !list_output.status.success() {
        log::warn!(
            "ddcutil detect failed: {}",
            String::from_utf8_lossy(&list_output.stderr)
        );
        return;
    }

    let stdout = String::from_utf8_lossy(&list_output.stdout);
    let mut display_numbers: Vec<u32> = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Display ") {
            if let Ok(num) = rest.trim().parse::<u32>() {
                display_numbers.push(num);
            }
        }
    }

    for display_num in display_numbers {
        // VCP 0x10 = Luminance (Brightness)
        let brightness = get_ddcutil_vcp(display_num, 0x10);
        // VCP 0x12 = Contrast
        let contrast = get_ddcutil_vcp(display_num, 0x12);

        let brightness_val = match brightness {
            Some(v) => v,
            None => continue,
        };

        let name = get_ddcutil_model_name(display_num)
            .unwrap_or_else(|| format!("External Display {}", display_num));

        monitors.push(Monitor {
            id: format!("external-{}", display_num),
            name,
            brightness: brightness_val,
            contrast: contrast.unwrap_or(50),
            supports_brightness: true,
            supports_contrast: contrast.is_some(),
            is_built_in: false,
        });
    }
}

#[cfg(target_os = "linux")]
fn get_ddcutil_vcp(display_num: u32, vcp_code: u8) -> Option<u32> {
    let output = std::process::Command::new("ddcutil")
        .args([
            "getvcp",
            &format!("0x{:02x}", vcp_code),
            "--display",
            &display_num.to_string(),
            "--brief",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Brief format: "VCP 10 C 50 100" (code type current max)
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 && parts[0] == "VCP" {
            let current = parts[3].parse::<f64>().ok()?;
            let max = parts[4].parse::<f64>().ok().unwrap_or(100.0);
            if max > 0.0 {
                return Some(((current / max) * 100.0).round() as u32);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn get_ddcutil_model_name(display_num: u32) -> Option<String> {
    let output = std::process::Command::new("ddcutil")
        .args(["detect", "--display", &display_num.to_string()])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(model) = line.strip_prefix("Model:") {
            let name = model.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn set_monitor_brightness(monitor_id: &str, value: u32) -> Result<(), String> {
    let value = value.min(100);

    if monitor_id.starts_with("builtin") {
        let output = std::process::Command::new("brightnessctl")
            .args(["set", &format!("{}%", value)])
            .output()
            .map_err(|e| format!("brightnessctl error: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "brightnessctl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        return Ok(());
    }

    let display_num = extract_display_number(monitor_id)?;
    // DDC/CI VCP 0x10 = Brightness
    let output = std::process::Command::new("ddcutil")
        .args([
            "setvcp",
            "0x10",
            &value.to_string(),
            "--display",
            &display_num.to_string(),
        ])
        .output()
        .map_err(|e| format!("ddcutil error: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ddcutil set brightness failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_monitor_contrast(monitor_id: &str, value: u32) -> Result<(), String> {
    let value = value.min(100);

    if monitor_id.starts_with("builtin") {
        return Err("Built-in display does not support contrast control".into());
    }

    let display_num = extract_display_number(monitor_id)?;
    // DDC/CI VCP 0x12 = Contrast
    let output = std::process::Command::new("ddcutil")
        .args([
            "setvcp",
            "0x12",
            &value.to_string(),
            "--display",
            &display_num.to_string(),
        ])
        .output()
        .map_err(|e| format!("ddcutil error: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ddcutil set contrast failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_all_monitors_brightness(value: u32) -> Result<(), String> {
    let monitors = detect_monitors();
    let mut errors: Vec<String> = Vec::new();
    for m in &monitors {
        if m.supports_brightness {
            if let Err(e) = set_monitor_brightness(&m.id, value) {
                errors.push(format!("{}: {}", m.name, e));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

#[cfg(target_os = "linux")]
fn set_all_monitors_contrast(value: u32) -> Result<(), String> {
    let monitors = detect_monitors();
    let mut errors: Vec<String> = Vec::new();
    for m in &monitors {
        if m.supports_contrast {
            if let Err(e) = set_monitor_contrast(&m.id, value) {
                errors.push(format!("{}: {}", m.name, e));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

// ===========================================================================
// Common helpers
// ===========================================================================

fn extract_display_number(monitor_id: &str) -> Result<u32, String> {
    monitor_id
        .split('-')
        .last()
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or_else(|| format!("Invalid monitor id format: {}", monitor_id))
}

fn merge_with_configs(
    monitors: Vec<Monitor>,
    configs: &crate::config::MonitorConfigs,
) -> Vec<Monitor> {
    let mut result: Vec<Monitor> = Vec::new();

    for mut monitor in monitors {
        if let Some(config) = configs.get(&monitor.id) {
            if !config.name.is_empty() {
                monitor.name = config.name.clone();
            }
            if config.disabled {
                continue;
            }
        }
        result.push(monitor);
    }

    result.sort_by(|a, b| {
        let order_a = configs.get(&a.id).map(|c| c.sort_order).unwrap_or(i32::MAX);
        let order_b = configs.get(&b.id).map(|c| c.sort_order).unwrap_or(i32::MAX);
        order_a.cmp(&order_b).then(a.id.cmp(&b.id))
    });

    result
}

// ===========================================================================
// Tauri commands
// ===========================================================================

#[tauri::command]
pub async fn get_monitors(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<Monitor>, String> {
    let monitors = detect_monitors();
    let configs = state.monitor_configs.lock().map_err(|e| e.to_string())?;
    Ok(merge_with_configs(monitors, &configs))
}

#[tauri::command]
pub async fn set_brightness(monitor_id: String, value: u32) -> Result<(), String> {
    set_monitor_brightness(&monitor_id, value)
}

#[tauri::command]
pub async fn set_contrast(monitor_id: String, value: u32) -> Result<(), String> {
    set_monitor_contrast(&monitor_id, value)
}

#[tauri::command]
pub async fn set_all_brightness(value: u32) -> Result<(), String> {
    set_all_monitors_brightness(value)
}

#[tauri::command]
pub async fn set_all_contrast(value: u32) -> Result<(), String> {
    set_all_monitors_contrast(value)
}

#[tauri::command]
pub fn rename_monitor(
    state: tauri::State<'_, crate::AppState>,
    monitor_id: String,
    name: String,
) -> Result<(), String> {
    let mut configs = state.monitor_configs.lock().map_err(|e| e.to_string())?;
    let config = configs
        .entry(monitor_id.clone())
        .or_insert_with(|| crate::config::MonitorConfig {
            id: monitor_id,
            name: String::new(),
            sort_order: 0,
            disabled: false,
        });
    config.name = name;
    crate::config::save_monitor_configs_to_disk(&configs);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_monitor(id: &str, name: &str, is_built_in: bool) -> Monitor {
        Monitor {
            id: id.into(),
            name: name.into(),
            brightness: 50,
            contrast: 50,
            supports_brightness: true,
            supports_contrast: !is_built_in,
            is_built_in,
        }
    }

    #[test]
    fn test_extract_display_number_valid() {
        assert_eq!(extract_display_number("external-1").unwrap(), 1);
        assert_eq!(extract_display_number("external-42").unwrap(), 42);
    }

    #[test]
    fn test_extract_display_number_invalid() {
        assert!(extract_display_number("invalid").is_err());
        assert!(extract_display_number("external-abc").is_err());
        assert!(extract_display_number("").is_err());
    }

    #[test]
    fn test_monitor_serialization_camel_case() {
        let monitor = make_monitor("builtin-0", "Built-in", true);
        let json = serde_json::to_string(&monitor).unwrap();
        assert!(json.contains("\"supportsBrightness\""));
        assert!(json.contains("\"supportsContrast\""));
        assert!(json.contains("\"isBuiltIn\""));
        // Should NOT have snake_case
        assert!(!json.contains("supports_brightness"));
        assert!(!json.contains("is_built_in"));
    }

    #[test]
    fn test_monitor_deserialization() {
        let json = r#"{
            "id": "external-1",
            "name": "Dell U2723QE",
            "brightness": 80,
            "contrast": 50,
            "supportsBrightness": true,
            "supportsContrast": true,
            "isBuiltIn": false
        }"#;
        let monitor: Monitor = serde_json::from_str(json).unwrap();
        assert_eq!(monitor.id, "external-1");
        assert_eq!(monitor.name, "Dell U2723QE");
        assert_eq!(monitor.brightness, 80);
        assert!(monitor.supports_contrast);
        assert!(!monitor.is_built_in);
    }

    #[test]
    fn test_monitor_roundtrip_serialization() {
        let original = make_monitor("external-2", "LG 27UK850", false);
        let json = serde_json::to_string(&original).unwrap();
        let restored: Monitor = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, original.id);
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.brightness, original.brightness);
        assert_eq!(restored.contrast, original.contrast);
        assert_eq!(restored.supports_brightness, original.supports_brightness);
        assert_eq!(restored.supports_contrast, original.supports_contrast);
        assert_eq!(restored.is_built_in, original.is_built_in);
    }

    #[test]
    fn test_merge_with_configs_renames_monitor() {
        let monitors = vec![make_monitor("external-1", "External Display 1", false)];
        let mut configs: crate::config::MonitorConfigs = HashMap::new();
        configs.insert(
            "external-1".into(),
            crate::config::MonitorConfig {
                id: "external-1".into(),
                name: "My Dell".into(),
                sort_order: 0,
                disabled: false,
            },
        );

        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "My Dell");
    }

    #[test]
    fn test_merge_with_configs_filters_disabled() {
        let monitors = vec![
            make_monitor("external-1", "Monitor 1", false),
            make_monitor("external-2", "Monitor 2", false),
        ];
        let mut configs: crate::config::MonitorConfigs = HashMap::new();
        configs.insert(
            "external-2".into(),
            crate::config::MonitorConfig {
                id: "external-2".into(),
                name: "Monitor 2".into(),
                sort_order: 0,
                disabled: true,
            },
        );

        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "external-1");
    }

    #[test]
    fn test_merge_with_configs_sorts_by_sort_order() {
        let monitors = vec![
            make_monitor("external-1", "Monitor A", false),
            make_monitor("external-2", "Monitor B", false),
            make_monitor("builtin-0", "Built-in", true),
        ];
        let mut configs: crate::config::MonitorConfigs = HashMap::new();
        configs.insert(
            "external-2".into(),
            crate::config::MonitorConfig {
                id: "external-2".into(),
                name: "Monitor B".into(),
                sort_order: 1,
                disabled: false,
            },
        );
        configs.insert(
            "builtin-0".into(),
            crate::config::MonitorConfig {
                id: "builtin-0".into(),
                name: "Built-in".into(),
                sort_order: 0,
                disabled: false,
            },
        );

        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, "builtin-0");
        assert_eq!(result[1].id, "external-2");
        // external-1 has no config -> sort_order = i32::MAX
        assert_eq!(result[2].id, "external-1");
    }

    #[test]
    fn test_merge_with_configs_empty_name_keeps_original() {
        let monitors = vec![make_monitor("external-1", "Original Name", false)];
        let mut configs: crate::config::MonitorConfigs = HashMap::new();
        configs.insert(
            "external-1".into(),
            crate::config::MonitorConfig {
                id: "external-1".into(),
                name: "".into(),
                sort_order: 0,
                disabled: false,
            },
        );

        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result[0].name, "Original Name");
    }

    #[test]
    fn test_merge_with_configs_no_configs() {
        let monitors = vec![
            make_monitor("builtin-0", "Built-in", true),
            make_monitor("external-1", "External", false),
        ];
        let configs: crate::config::MonitorConfigs = HashMap::new();

        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 2);
    }
}
