use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
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
// macOS implementation — DDC/CI via ddc-macos for external monitors,
// CoreGraphics gamma fallback for monitors without DDC support,
// IOKit for built-in display.
// ===========================================================================

#[cfg(target_os = "macos")]
const VCP_BRIGHTNESS: u8 = 0x10;
#[cfg(target_os = "macos")]
const VCP_CONTRAST: u8 = 0x12;

// CoreGraphics FFI for gamma table control (software brightness fallback).
#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGSetDisplayTransferByFormula(
        display: u32,
        red_min: f32, red_max: f32, red_gamma: f32,
        green_min: f32, green_max: f32, green_gamma: f32,
        blue_min: f32, blue_max: f32, blue_gamma: f32,
    ) -> i32;
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayIsBuiltin(display: u32) -> i32;
}

// DisplayServices private framework FFI for built-in display brightness.
#[cfg(target_os = "macos")]
type DisplayServicesGetBrightnessFn = unsafe extern "C" fn(u32, *mut f32) -> i32;
#[cfg(target_os = "macos")]
type DisplayServicesSetBrightnessFn = unsafe extern "C" fn(u32, f32) -> i32;

#[cfg(target_os = "macos")]
struct DisplayServicesFns {
    get_brightness: DisplayServicesGetBrightnessFn,
    set_brightness: DisplayServicesSetBrightnessFn,
}

#[cfg(target_os = "macos")]
fn display_services() -> Option<&'static DisplayServicesFns> {
    use std::sync::OnceLock;
    static FUNCS: OnceLock<Option<DisplayServicesFns>> = OnceLock::new();
    FUNCS.get_or_init(|| {
        unsafe {
            let path = std::ffi::CString::new(
                "/System/Library/PrivateFrameworks/DisplayServices.framework/DisplayServices"
            ).ok()?;
            let handle = libc::dlopen(path.as_ptr(), libc::RTLD_NOW);
            if handle.is_null() { return None; }
            let get_sym = libc::dlsym(handle, b"DisplayServicesGetBrightness\0".as_ptr() as *const _);
            let set_sym = libc::dlsym(handle, b"DisplayServicesSetBrightness\0".as_ptr() as *const _);
            if get_sym.is_null() || set_sym.is_null() { return None; }
            Some(DisplayServicesFns {
                get_brightness: std::mem::transmute(get_sym),
                set_brightness: std::mem::transmute(set_sym),
            })
        }
    }).as_ref()
}

/// Per-monitor state tracked across detect/set calls.
#[cfg(target_os = "macos")]
struct ExternalMonitorInfo {
    cg_display_id: u32,
    ddc_supported: bool,
    gamma_brightness: u32,
}

#[cfg(target_os = "macos")]
fn mac_state() -> &'static std::sync::Mutex<std::collections::HashMap<String, ExternalMonitorInfo>> {
    use std::sync::OnceLock;
    static STATE: OnceLock<std::sync::Mutex<std::collections::HashMap<String, ExternalMonitorInfo>>> =
        OnceLock::new();
    STATE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

#[cfg(target_os = "macos")]
fn detect_monitors() -> Vec<Monitor> {
    let mut monitors: Vec<Monitor> = Vec::new();

    if let Some(builtin) = detect_builtin_monitor_macos() {
        monitors.push(builtin);
    }

    detect_external_monitors_ddc(&mut monitors);

    monitors
}

/// Find the CGDirectDisplayID of the built-in display.
#[cfg(target_os = "macos")]
fn find_builtin_display_id() -> Option<u32> {
    unsafe {
        let mut displays = [0u32; 10];
        let mut count: u32 = 0;
        CGGetActiveDisplayList(10, displays.as_mut_ptr(), &mut count);
        for i in 0..count as usize {
            if CGDisplayIsBuiltin(displays[i]) != 0 {
                return Some(displays[i]);
            }
        }
    }
    None
}

/// Detect built-in display via DisplayServices private framework.
#[cfg(target_os = "macos")]
fn detect_builtin_monitor_macos() -> Option<Monitor> {
    let ds = display_services()?;
    let display_id = find_builtin_display_id()?;
    let mut brightness_f: f32 = 0.0;
    let result = unsafe { (ds.get_brightness)(display_id, &mut brightness_f) };
    if result != 0 {
        return None;
    }
    let brightness = (brightness_f * 100.0).round() as u32;
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

/// Detect external monitors via ddc-macos. Monitors that fail DDC reads
/// are still included but use gamma fallback for brightness control.
#[cfg(target_os = "macos")]
fn detect_external_monitors_ddc(monitors: &mut Vec<Monitor>) {
    use ddc::Ddc;

    let ddc_monitors = match ddc_macos::Monitor::enumerate() {
        Ok(m) => m,
        Err(e) => {
            log::warn!("Failed to enumerate DDC monitors: {}", e);
            return;
        }
    };

    let mut state = mac_state().lock().unwrap_or_else(|e| e.into_inner());

    for (idx, mut ddc_mon) in ddc_monitors.into_iter().enumerate() {
        let monitor_id = format!("external-{}", idx + 1);
        let cg_display_id = ddc_mon.handle().id;
        let name = ddc_mon
            .product_name()
            .unwrap_or_else(|| format!("External Display {}", idx + 1));

        let ddc_brightness = ddc_mon.get_vcp_feature(VCP_BRIGHTNESS).ok().map(|val| {
            let max = val.maximum() as f64;
            let cur = val.value() as f64;
            if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 50 }
        });

        let ddc_contrast = ddc_mon.get_vcp_feature(VCP_CONTRAST).ok().map(|val| {
            let max = val.maximum() as f64;
            let cur = val.value() as f64;
            if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 50 }
        });

        let ddc_supported = ddc_brightness.is_some();
        let existing_gamma = state.get(&monitor_id).map(|s| s.gamma_brightness).unwrap_or(100);
        let brightness = ddc_brightness.unwrap_or(existing_gamma);

        state.insert(monitor_id.clone(), ExternalMonitorInfo {
            cg_display_id,
            ddc_supported,
            gamma_brightness: if ddc_supported { 100 } else { existing_gamma },
        });

        monitors.push(Monitor {
            id: monitor_id,
            name,
            brightness,
            contrast: ddc_contrast.unwrap_or(50),
            supports_brightness: true,
            supports_contrast: ddc_supported && ddc_contrast.is_some(),
            is_built_in: false,
        });
    }
}

/// Set brightness via gamma table (software dimming, 0-100).
#[cfg(target_os = "macos")]
fn set_gamma_brightness(cg_display_id: u32, value: u32) {
    let val = (value.min(100) as f32) / 100.0;
    unsafe {
        CGSetDisplayTransferByFormula(
            cg_display_id,
            0.0, val, 1.0,
            0.0, val, 1.0,
            0.0, val, 1.0,
        );
    }
}

#[cfg(target_os = "macos")]
fn set_monitor_brightness(monitor_id: &str, value: u32) -> Result<(), String> {
    let value = value.min(100);

    if monitor_id.starts_with("builtin") {
        return set_builtin_brightness(value);
    }

    let mut state = mac_state().lock().unwrap_or_else(|e| e.into_inner());
    let info = state.get(monitor_id)
        .ok_or_else(|| format!("Monitor {} not found in state", monitor_id))?;

    if info.ddc_supported {
        let target_index = extract_display_number(monitor_id)?;
        drop(state);
        set_ddc_value_macos(target_index, VCP_BRIGHTNESS, value)
    } else {
        let cg_id = info.cg_display_id;
        if let Some(info) = state.get_mut(monitor_id) {
            info.gamma_brightness = value;
        }
        drop(state);
        set_gamma_brightness(cg_id, value);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn set_monitor_contrast(monitor_id: &str, value: u32) -> Result<(), String> {
    let value = value.min(100);

    if monitor_id.starts_with("builtin") {
        return Err("Built-in display does not support contrast control".into());
    }

    let state = mac_state().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(info) = state.get(monitor_id) {
        if !info.ddc_supported {
            return Err("Contrast not available (gamma-only monitor)".into());
        }
    }
    drop(state);

    let target_index = extract_display_number(monitor_id)?;
    set_ddc_value_macos(target_index, VCP_CONTRAST, value)
}

/// Set a DDC VCP value on an external monitor via IOKit I2C.
#[cfg(target_os = "macos")]
fn set_ddc_value_macos(target_index: u32, vcp_code: u8, value: u32) -> Result<(), String> {
    use ddc::Ddc;

    let ddc_monitors = ddc_macos::Monitor::enumerate()
        .map_err(|e| format!("Failed to enumerate DDC monitors: {}", e))?;

    let idx = (target_index as usize).saturating_sub(1);
    let mut ddc_mon = ddc_monitors
        .into_iter()
        .nth(idx)
        .ok_or_else(|| format!("Monitor index {} not found", target_index))?;

    // Get the monitor's actual max value to map our 0-100 percentage.
    // Clamp brightness minimum to 1 — some monitors turn off/freeze at 0.
    let max_val = ddc_mon
        .get_vcp_feature(vcp_code)
        .map(|v| v.maximum())
        .unwrap_or(100);

    let raw_value = if max_val == 100 {
        value.max(1) as u16
    } else {
        (value.max(1) as f64 / 100.0 * max_val as f64).round() as u16
    };

    ddc_mon
        .set_vcp_feature(vcp_code, raw_value)
        .map_err(|e| format!("DDC set VCP 0x{:02X} failed: {}", vcp_code, e))
}

/// Set built-in display brightness via DisplayServices private framework.
#[cfg(target_os = "macos")]
fn set_builtin_brightness(value: u32) -> Result<(), String> {
    let ds = display_services()
        .ok_or("DisplayServices framework not available")?;
    let display_id = find_builtin_display_id()
        .ok_or("No built-in display found")?;
    let float_val = value.min(100) as f32 / 100.0;
    let result = unsafe { (ds.set_brightness)(display_id, float_val) };
    if result != 0 {
        return Err(format!("DisplayServicesSetBrightness failed: {}", result));
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
