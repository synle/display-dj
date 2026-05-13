use super::win_cmd::hidden_command;
use super::{DisplayControl, DisplayInfo, Platform, BUILTIN_ID, VCP_BRIGHTNESS, VCP_CONTRAST};
use ddc::Ddc; // trait providing get_vcp_feature / set_vcp_feature
use std::thread;
use std::time::Duration;

// Win32 API imports — the `windows` crate provides safe-ish Rust bindings to Win32.
// Each feature must be explicitly enabled in Cargo.toml.
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use winapi::um::wingdi::SetDeviceGammaRamp;

/// Cast a `windows::Win32::Graphics::Gdi::HMONITOR` (raw pointer wrapped in a
/// tuple struct) to the `winapi::shared::windef::HMONITOR` type expected by
/// `ddc-winapi` v0.2. Both are opaque-pointer aliases over the same Win32
/// handle — only the surrounding Rust wrapper type differs. Pulled out as a
/// helper so the unsafe-looking pointer cast is named and documented.
fn hmonitor_to_winapi(hm: HMONITOR) -> winapi::shared::windef::HMONITOR {
    hm.0 as winapi::shared::windef::HMONITOR
}

// DDC write retry parameters — mirrored from `core::macos` so that the two
// platforms behave identically. Some panels (Acer XZ322QU V3 family, several
// Samsung models) need repeated I2C writes before the hardware actually
// processes the command. Five attempts at 50 ms spacing was the empirically
// chosen baseline upstream — keeping the numbers in sync prevents one
// platform from regressing while the other stays correct.
const DDC_WRITE_RETRIES: u32 = 5;
const DDC_RETRY_DELAY_MS: u64 = 50;

/// Try writing a VCP feature with multiple attempts and delays.
/// Returns true on first successful write, false after `DDC_WRITE_RETRIES`
/// consecutive failures. Each successful write is followed by an extra
/// `DDC_RETRY_DELAY_MS` so the monitor's MCU has time to process before any
/// follow-up call (read-back, contrast, etc.). Direct port of the same-named
/// helper in `core::macos`.
fn ddc_write_with_retry(mon: &mut ddc_winapi::Monitor, vcp: u8, value: u16) -> bool {
    for attempt in 0..DDC_WRITE_RETRIES {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(DDC_RETRY_DELAY_MS));
        }
        if mon.set_vcp_feature(vcp, value).is_ok() {
            thread::sleep(Duration::from_millis(DDC_RETRY_DELAY_MS));
            return true;
        }
    }
    false
}

// =========================================================================
// Built-in display — WMI (Windows Management Instrumentation) via PowerShell.
// Laptops expose brightness through the WmiMonitorBrightness WMI class.
// This only works for the built-in panel, not external monitors.
// =========================================================================

/// Built-in (laptop) display controller.
/// Uses WMI (Windows Management Instrumentation) via PowerShell to read/write brightness.
/// Unit struct (no fields) because WMI is a system-wide service — no per-instance state needed.
struct BuiltinControl;

impl BuiltinControl {
    /// Read current brightness from WMI. Returns 0-100 or None if not a laptop.
    /// Queries WmiMonitorBrightness.CurrentBrightness — only available on laptops with
    /// an internal panel. Desktops return None here.
    fn wmi_get() -> Option<u32> {
        let output = hidden_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command",
                "(Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightness -ErrorAction SilentlyContinue).CurrentBrightness"])
            .output().ok()?; // .ok()? = convert Result->Option, return None on error
        if !output.status.success() { return None; }
        // from_utf8_lossy handles invalid UTF-8 gracefully (replaces with ?).
        // .trim() removes whitespace, .parse() converts "75" -> 75u32.
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }

    /// Set brightness via WMI. value is 0-100.
    /// Uses WmiMonitorBrightnessMethods.WmiSetBrightness — the timeout param (1) is
    /// in seconds and tells the driver how fast to ramp the backlight.
    fn wmi_set(value: u16) -> bool {
        let cmd = format!(
            "(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightnessMethods).WmiSetBrightness(1, {})",
            value
        );
        let out = hidden_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
            .output();
        let ok = out.as_ref().map(|o| o.status.success()).unwrap_or(false);
        let stderr = out.as_ref().map(|o| String::from_utf8_lossy(&o.stderr).trim().to_string()).unwrap_or_default();
        log::info!(
            "set_brightness[builtin/wmi]: value={} return_ok={} stderr={:?}",
            value, ok, stderr,
        );
        ok
    }
}

/// DisplayControl implementation for the built-in panel.
/// All operations delegate to WMI — mode parameter is ignored since the built-in
/// always uses the platform-native backlight API (no DDC, no gamma).
impl DisplayControl for BuiltinControl {
    fn get_brightness(&mut self) -> Option<u32> { Self::wmi_get() }
    fn get_contrast(&mut self) -> Option<u32> { None }  // WMI doesn't expose contrast
    fn set_brightness(&mut self, value: u16, _mode: &str) -> bool { Self::wmi_set(value) }
    fn set_contrast(&mut self, _value: u16) -> bool { false }
    fn reset_gamma(&self) {} // no gamma used for builtin — nothing to reset
}

// =========================================================================
// External monitor — DDC/CI via ddc-winapi + gamma ramp via GDI32.
// ddc-winapi uses the Dxva2.dll API to send I2C commands to monitors.
//
// TODO(soft-overlay-fallback): when **both** DDC and GDI gamma fail (Samsung
// Smart Monitors on Intel Iris Xe are the canonical case — the panel rejects
// VCP_BRIGHTNESS and the Intel driver silently rejects `SetDeviceGammaRamp`
// for external displays), there is no third hardware path that can dim the
// physical backlight. The remaining option is a *software dimming overlay*:
// spawn a per-monitor borderless, click-through, always-on-top transparent
// Tauri window sized to the monitor's full work area and modulate its black
// alpha to match `100 - brightness`. The OS compositor handles the blending
// at no measurable cost. This is what Twinkle Tray / Win10_BrightnessSlider
// / Lunar do for non-DDC monitors. Not in scope for v7.0.13 — file is the
// design note; implement when we revisit.
// =========================================================================

/// External monitor controller.
/// Combines two Windows APIs: DDC/CI via Dxva2 (hardware brightness) and
/// GDI32 gamma ramp (software brightness). Both are needed because DDC doesn't
/// work on all monitors, and gamma alone reduces color range.
struct ExternalControl {
    ddc_monitor: ddc_winapi::Monitor, // wraps a physical monitor handle from Dxva2
    hmonitor: HMONITOR,               // GDI monitor handle for gamma ramp access
    ddc_supported: bool,              // true if initial VCP brightness read succeeded
}

/// DisplayControl implementation for external monitors.
/// Supports DDC/CI (hardware), gamma ramp (software), or both stacked (force mode).
/// Mode selection: "ddc" = DDC only, "gamma" = gamma only, "force" = both,
/// "auto" = DDC if supported else gamma.
impl DisplayControl for ExternalControl {
    fn get_brightness(&mut self) -> Option<u32> {
        // Read VCP brightness register via DDC/CI, convert to 0-100 percentage
        self.ddc_monitor.get_vcp_feature(VCP_BRIGHTNESS).ok().map(|val| {
            let max = val.maximum() as f64;
            let cur = val.value() as f64;
            if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 50 }
        })
    }

    fn get_contrast(&mut self) -> Option<u32> {
        self.ddc_monitor.get_vcp_feature(VCP_CONTRAST).ok().map(|val| {
            let max = val.maximum() as f64;
            let cur = val.value() as f64;
            if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 50 }
        })
    }

    fn set_brightness(&mut self, value: u16, mode: &str) -> bool {
        // Parity with macOS (`core::macos::set_brightness`): in force mode, attempt
        // the DDC write **even when the initial VCP read failed** (`ddc_supported
        // == false`). Some panels — notably Samsung Smart Monitors — silently
        // reject the VCP_BRIGHTNESS read but accept the write. Without this, an
        // entire class of monitors falls back to gamma-only and never gets a real
        // hardware-level write attempt.
        let use_ddc = mode == "ddc" || mode == "force" || (mode == "auto" && self.ddc_supported);
        // `try_ddc` — actually attempt the DDC write, even if read suggested no support.
        // In force mode we always try (write may succeed where read failed).
        let try_ddc = use_ddc || mode == "force";
        let use_gamma = mode == "gamma" || mode == "force" || (mode == "auto" && !self.ddc_supported);
        let mut ddc_attempted = false;
        let mut ddc_ok = true;
        let mut ddc_err: Option<String> = None;
        let mut ddc_verify: Option<u32> = None; // read-back after write
        let mut gamma_ok = true;
        let mut ok = true;

        if try_ddc {
            ddc_attempted = true;
            let ddc_val = if value == 0 { 1 } else { value }; // clamp to 1 to avoid standby
            // Retry the write — some monitors (Acer XZ322QU V3 family, several
            // Samsung models) need repeated I2C writes before the hardware
            // actually processes the command. Same retry/delay constants as
            // macOS for consistency.
            if !ddc_write_with_retry(&mut self.ddc_monitor, VCP_BRIGHTNESS, ddc_val) {
                ddc_ok = false;
                ddc_err = Some("set_vcp_feature failed after retries".to_string());
                // Only flag the overall write as failed if gamma is not going
                // to be applied — otherwise gamma can still dim the panel via
                // the compositor and the user-visible operation succeeds.
                if !use_gamma {
                    ok = false;
                }
            } else {
                // Write succeeded at the API level — try a verify-read to confirm
                // the monitor actually accepted the value (not just acknowledged
                // the I2C transaction). Samsung Smart Monitors are known to ACK
                // VCP writes while silently ignoring the value; this read tells
                // us whether the firmware actually updated the register. A
                // 100 ms settle delay matches the upstream cli's empirical
                // baseline for "monitor has processed the write."
                thread::sleep(Duration::from_millis(100));
                ddc_verify = self.ddc_monitor.get_vcp_feature(VCP_BRIGHTNESS).ok().map(|val| {
                    let max = val.maximum() as f64;
                    let cur = val.value() as f64;
                    if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 0 }
                });
            }
        }

        if use_gamma {
            gamma_ok = set_gamma_for_hmonitor(self.hmonitor, value as u32);
            if !gamma_ok && !ddc_ok {
                // Both paths failed — surface a hard failure so the frontend
                // shows the brightness slider as not-honored. Previously we
                // silently returned `true` (because `SetDeviceGammaRamp`'s
                // BOOL return was discarded), which produced the "slider moves
                // but nothing dims" symptom Samsung+Intel users were hitting.
                ok = false;
            }
        }

        // Per-call diagnostic — surfaces in stderr / env_logger output AND
        // (via the `TeeLogger` installed in `lib.rs::run()`) in the debug log
        // file that `Dump Debug Info` exports. Shows which path(s) ran,
        // whether DDC actually accepted the write, the verify-read value
        // (panel-side confirmation that the firmware honored the write,
        // distinct from the I2C transaction returning OK), whether gamma was
        // applied, and the raw SetVCPFeature error string when present.
        // Critical for diagnosing displays that look "controllable" at
        // enumerate time but silently reject brightness writes (common on
        // USB-C panels and several Samsung models).
        log::info!(
            "set_brightness[external]: value={} mode={} ddc_supported={} use_ddc={} try_ddc={} use_gamma={} ddc_attempted={} ddc_ok={} ddc_err={:?} ddc_verify={:?} gamma_ok={} return_ok={}",
            value, mode, self.ddc_supported, use_ddc, try_ddc, use_gamma, ddc_attempted, ddc_ok, ddc_err, ddc_verify, gamma_ok, ok,
        );

        ok
    }

    fn set_contrast(&mut self, value: u16) -> bool {
        if !self.ddc_supported {
            log::info!("set_contrast[external]: skipped (ddc_supported=false) value={}", value);
            return false;
        }
        let res = self.ddc_monitor.set_vcp_feature(VCP_CONTRAST, value);
        let ok = res.is_ok();
        let err = res.err().map(|e| format!("{}", e));
        log::info!("set_contrast[external]: value={} return_ok={} err={:?}", value, ok, err);
        ok
    }

    fn reset_gamma(&self) {
        let _ = set_gamma_for_hmonitor(self.hmonitor, 100);
    }
}

/// Set software brightness by writing a gamma ramp to the GPU via GDI32.
/// brightness 0-100 scales the ramp linearly.
///
/// The gamma ramp is a 768-element u16 array: [256 red, 256 green, 256 blue].
/// Each entry maps an input intensity (0-255) to an output intensity (0-65535).
///
/// Returns `true` only when **every** step succeeded:
/// `GetMonitorInfoW` → `CreateDCW` → `SetDeviceGammaRamp`. On Intel Iris Xe
/// + external display setups, `SetDeviceGammaRamp` routinely returns `FALSE`
/// without effect — previously the return value was discarded with `let _`
/// and callers assumed success, producing the "slider moves but nothing dims"
/// symptom. Callers now surface a hard failure when both DDC and gamma fail.
fn set_gamma_for_hmonitor(hmonitor: HMONITOR, brightness: u32) -> bool {
    let factor = (brightness.min(100) as f64) / 100.0;
    unsafe {
        // Get the monitor's device name so we can create a DC (device context) for it
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(hmonitor, &mut info.monitorInfo as *mut _).as_bool() {
            return false;
        }
        let hdc = CreateDCW(
            windows::core::PCWSTR(info.szDevice.as_ptr()),
            None, None, None,
        );
        if hdc.is_invalid() {
            return false;
        }
        // Build the gamma ramp — linear from 0 to (factor * 65535) for each channel
        let mut ramp = [0u16; 768];
        for i in 0..256 {
            let val = ((i as f64 / 255.0 * factor) * 65535.0) as u16;
            ramp[i] = val;       // red
            ramp[256 + i] = val; // green
            ramp[512 + i] = val; // blue
        }
        // SetDeviceGammaRamp returns `BOOL` — non-zero on success. Intel iGPUs
        // commonly return 0 for external displays on multi-monitor configs;
        // capture the result so the caller can surface a hard failure.
        let ok = SetDeviceGammaRamp(
            hdc.0 as winapi::shared::windef::HDC,
            ramp.as_ptr() as *mut _,
        ) != 0;
        let _ = DeleteDC(hdc); // clean up the device context
        ok
    }
}

/// Enumerate all HMONITOR handles using the Win32 EnumDisplayMonitors callback.
/// HMONITOR is the GDI handle for gamma ramp access (separate from DDC handles).
fn enum_hmonitors() -> Vec<HMONITOR> {
    let mut hmonitors: Vec<HMONITOR> = Vec::new();
    unsafe {
        // Win32 callback — called once per monitor. We push each handle into the Vec.
        // LPARAM carries our Vec pointer through the callback (Win32's version of closure context).
        unsafe extern "system" fn enum_proc(
            hmonitor: HMONITOR, _hdc: HDC, _lprect: *mut RECT, lparam: LPARAM,
        ) -> BOOL {
            let monitors = &mut *(lparam.0 as *mut Vec<HMONITOR>);
            monitors.push(hmonitor);
            BOOL(1) // return TRUE to continue enumeration
        }
        let _ = EnumDisplayMonitors(
            None, None, Some(enum_proc),
            LPARAM(&mut hmonitors as *mut Vec<HMONITOR> as isize),
        );
    }
    hmonitors
}

/// Get the PnP device identifier and primary flag for an HMONITOR.
/// Returns (device_identifier, is_primary).
///
/// The identifier is extracted from the monitor's PnP device ID via EnumDisplayDevicesW.
/// The full device ID looks like `MONITOR\DEL40F4\{guid}\NNNN` — we extract the second
/// segment (e.g. "DEL40F4" for a Dell, "GSM5BBF" for an LG). This is used to:
/// 1. Disambiguate monitors with the same generic description ("Generic PnP Monitor")
/// 2. Create composite names like "Generic PnP Monitor (DEL40F4)"
///
/// Falls back to the display device name (e.g. "DISPLAY2") if EnumDisplayDevicesW fails.
///
/// The `is_primary` flag (from MONITORINFOF_PRIMARY) is critical for the builtin dedup
/// logic — on laptops, the primary HMONITOR is the built-in panel.
fn get_hmonitor_details(hmonitor: HMONITOR) -> (String, bool) {
    unsafe {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(hmonitor, &mut info.monitorInfo as *mut _).as_bool() {
            return (String::new(), false);
        }

        let is_primary = (info.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY

        // Call EnumDisplayDevicesW with the adapter device name to get the monitor's PnP ID.
        let mut dd: DISPLAY_DEVICEW = std::mem::zeroed();
        dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        if EnumDisplayDevicesW(
            windows::core::PCWSTR(info.szDevice.as_ptr()),
            0,
            &mut dd,
            0,
        ).as_bool() {
            let full_id = String::from_utf16_lossy(
                &dd.DeviceID[..dd.DeviceID.iter().position(|&c| c == 0).unwrap_or(dd.DeviceID.len())]
            );
            // PnP device ID looks like "MONITOR\DEL40F4\{guid}\NNNN" — extract "DEL40F4"
            let parts: Vec<&str> = full_id.split('\\').collect();
            if parts.len() >= 2 && !parts[1].is_empty() {
                return (parts[1].to_string(), is_primary);
            }
        }

        // Fallback: extract display number from device name ("\\.\DISPLAY2" -> "DISPLAY2")
        let device_name = String::from_utf16_lossy(
            &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len())]
        );
        let fallback = device_name.trim_start_matches("\\\\.\\").to_string();
        (fallback, is_primary)
    }
}

// =========================================================================
// Platform implementation — discovers all displays on Windows
// =========================================================================

/// Windows platform implementation.
/// Discovers all displays by combining WMI (built-in) with DDC/Dxva2 (external),
/// deduplicating the built-in panel when it appears in both APIs.
pub struct WinPlatform;

impl Platform for WinPlatform {
    /// Enumerate all displays on this Windows machine.
    /// Returns a unified list of (DisplayInfo, DisplayControl) pairs for both
    /// the built-in panel (via WMI) and external monitors (via DDC/CI).
    fn enumerate() -> Vec<(DisplayInfo, Box<dyn DisplayControl>)> {
        let mut result: Vec<(DisplayInfo, Box<dyn DisplayControl>)> = Vec::new();

        // --- Built-in display ---
        // WMI brightness is only available on laptops with an internal panel.
        // If wmi_get returns None, there's no built-in display (or it's a desktop).
        if let Some(brightness) = BuiltinControl::wmi_get() {
            let info = DisplayInfo {
                id: BUILTIN_ID.into(),
                name: "Built-in Display".into(),
                display_type: "builtin".into(),
                brightness: Some(brightness),
                contrast: None,
                ddc_supported: false,
            };
            // BuiltinControl is a unit struct — no fields to initialize
            result.push((info, Box::new(BuiltinControl)));
        }

        // --- External displays ---
        // We need both DDC handles (for brightness) and HMONITOR handles (for gamma).
        // Previously, we called `ddc_winapi::Monitor::enumerate()` and then `zip`'d
        // the resulting Vec with `enum_hmonitors()` by index — both internally call
        // `EnumDisplayMonitors`, and the assumption was "callback order is the same".
        // That assumption was fragile: `EnumDisplayMonitors` is not documented to
        // return a deterministic order, and a single mismatched index assigns the
        // wrong DDC handle to a monitor (e.g. SetVCPFeature targets the laptop panel
        // when the user pulls the slider on the external one, while the GDI gamma
        // call writes to the correct external HMONITOR — yielding a visible no-op
        // because the panel that DDC actually accepted the write for is a different
        // physical screen entirely).
        //
        // Fix: enumerate HMONITORs once via `enum_hmonitors()`, then for each
        // HMONITOR call `ddc_winapi::get_physical_monitors_from_hmonitor(hm)` to
        // get the physical monitor(s) attached to that specific HMONITOR. Wrap
        // each `PHYSICAL_MONITOR` in `Monitor::new(pm)`. The (DDC handle, HMONITOR)
        // pairing is now explicit and per-HMONITOR — order can't drift.
        //
        // DEDUP: On laptops, the built-in panel appears in both WMI and DDC
        // enumeration. We track has_builtin to skip the primary HMONITOR from DDC
        // when WMI already covered it. See "Windows display dedup" in CLAUDE.md.
        let has_builtin = !result.is_empty();
        let hmonitors = enum_hmonitors();
        let hmonitor_details: Vec<(String, bool)> = hmonitors.iter()
            .map(|&hm| get_hmonitor_details(hm))
            .collect();

        let mut ext_id = 1usize;
        for (idx, &hmonitor) in hmonitors.iter().enumerate() {
            let (device_id, is_primary) = hmonitor_details.get(idx)
                .cloned()
                .unwrap_or((String::new(), false));

            // DEDUP: Skip the primary (built-in) monitor if we already added it via WMI.
            // On laptops, the built-in panel appears in both WMI and DDC enumeration.
            // Without this check, you'd get a duplicate: the WMI "Built-in Display"
            // plus a DDC "Generic PnP Monitor" with null brightness (laptop panels
            // don't respond to DDC commands). The primary HMONITOR flag reliably
            // identifies the built-in panel across all Windows laptop configurations.
            if has_builtin && is_primary {
                continue;
            }

            // Resolve physical monitors for *this* HMONITOR — explicit pairing,
            // no implicit index alignment with a separate enumeration call.
            let physical_monitors = match ddc_winapi::get_physical_monitors_from_hmonitor(
                hmonitor_to_winapi(hmonitor),
            ) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!(
                        "GetPhysicalMonitorsFromHMONITOR failed for hmonitor[{}] device_id={:?}: {}",
                        idx, device_id, e,
                    );
                    continue;
                }
            };

            for pm in physical_monitors {
                // SAFETY: `pm` came from GetPhysicalMonitorsFromHMONITOR; ddc-winapi
                // takes ownership of the handle and calls DestroyPhysicalMonitor on
                // drop. The constructor is unsafe only because the crate cannot
                // verify the handle is valid; we obtained it from the OS API one
                // statement ago.
                let mut mon = unsafe { ddc_winapi::Monitor::new(pm) };

                let brightness = mon.get_vcp_feature(VCP_BRIGHTNESS).ok().map(|val| {
                    let max = val.maximum() as f64;
                    let cur = val.value() as f64;
                    if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 50 }
                });
                let contrast = mon.get_vcp_feature(VCP_CONTRAST).ok().map(|val| {
                    let max = val.maximum() as f64;
                    let cur = val.value() as f64;
                    if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 50 }
                });
                let ddc_supported = brightness.is_some();

                // Append device PnP identifier to distinguish monitors with the
                // same generic description (e.g. "Generic PnP Monitor (DEL40F4)")
                let base_name = mon.description();
                let name = if device_id.is_empty() {
                    base_name
                } else {
                    format!("{} ({})", base_name, device_id)
                };

                let info = DisplayInfo {
                    id: ext_id.to_string(),
                    name,
                    display_type: "external".into(),
                    brightness,
                    contrast,
                    ddc_supported,
                };
                result.push((info, Box::new(ExternalControl { ddc_monitor: mon, hmonitor, ddc_supported })));
                ext_id += 1;
            }
        }

        result
    }

    /// Reset gamma ramps on all monitors to their default (identity) values.
    /// Called by the `reset` CLI command. Iterates every HMONITOR and writes
    /// a 100% linear ramp, undoing any software dimming.
    fn reset_all_gamma() {
        for hmonitor in enum_hmonitors() {
            let _ = set_gamma_for_hmonitor(hmonitor, 100);
        }
    }

    /// Dump raw platform diagnostics for the `debug` command.
    /// Returns WMI brightness, all HMONITOR details (device names, rects, primary flag,
    /// PnP IDs), and raw DDC VCP reads for each monitor. This data is essential for
    /// diagnosing dedup issues and DDC/CI communication problems.
    fn debug_info() -> serde_json::Value {
        // --- WMI brightness (built-in panel) ---
        let wmi_brightness = BuiltinControl::wmi_get();

        // --- HMONITOR enumeration with full details ---
        let hmonitors = enum_hmonitors();
        let mut hmon_list = Vec::new();
        for (idx, &hm) in hmonitors.iter().enumerate() {
            hmon_list.push(debug_hmonitor(hm, idx));
        }

        // --- DDC monitors with raw VCP data ---
        // Enumerate per-HMONITOR (same pairing strategy as `enumerate()`) so the
        // `hmonitor_index` field anchors each DDC entry to a specific physical
        // display in the `hmonitors` array above. Previous code called
        // `Monitor::enumerate()` and reported a flat index, which made it
        // impossible to tell which HMONITOR a DDC entry belonged to when
        // ordering between the two calls drifted.
        let mut ddc_list = Vec::new();
        let mut ddc_errors: Vec<String> = Vec::new();
        let mut ddc_seq = 0usize;
        for (hm_idx, &hmonitor) in hmonitors.iter().enumerate() {
            match ddc_winapi::get_physical_monitors_from_hmonitor(hmonitor_to_winapi(hmonitor)) {
                Ok(physicals) => {
                    for pm in physicals {
                        // SAFETY: handle obtained from the OS one line above; ddc-winapi
                        // takes ownership and calls DestroyPhysicalMonitor on drop.
                        let mut mon = unsafe { ddc_winapi::Monitor::new(pm) };
                        let desc = mon.description();
                        let brightness_raw = mon.get_vcp_feature(VCP_BRIGHTNESS).ok().map(|v| {
                            serde_json::json!({"current": v.value(), "max": v.maximum()})
                        });
                        let contrast_raw = mon.get_vcp_feature(VCP_CONTRAST).ok().map(|v| {
                            serde_json::json!({"current": v.value(), "max": v.maximum()})
                        });
                        ddc_list.push(serde_json::json!({
                            "index": ddc_seq,
                            "hmonitor_index": hm_idx,
                            "description": desc,
                            "vcp_brightness": brightness_raw,
                            "vcp_contrast": contrast_raw,
                        }));
                        ddc_seq += 1;
                    }
                }
                Err(e) => {
                    ddc_errors.push(format!("hmonitor[{}]: {}", hm_idx, e));
                }
            }
        }
        let ddc_error: Option<String> = if ddc_errors.is_empty() {
            None
        } else {
            Some(ddc_errors.join("; "))
        };

        serde_json::json!({
            "wmi_brightness": wmi_brightness,
            "hmonitor_count": hmonitors.len(),
            "hmonitors": hmon_list,
            "ddc_monitor_count": ddc_list.len(),
            "ddc_monitors": ddc_list,
            "ddc_enumerate_error": ddc_error,
        })
    }
}

/// Dump full details for a single HMONITOR — used by debug_info().
/// Includes: device name (e.g. `\\.\DISPLAY1`), primary flag, monitor rect,
/// adapter name (GPU driving this output), and the monitor's PnP device ID.
/// This data helps diagnose which HMONITOR corresponds to which physical display.
fn debug_hmonitor(hmonitor: HMONITOR, idx: usize) -> serde_json::Value {
    unsafe {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(hmonitor, &mut info.monitorInfo as *mut _).as_bool() {
            return serde_json::json!({"index": idx, "error": "GetMonitorInfoW failed"});
        }

        let is_primary = (info.monitorInfo.dwFlags & 1) != 0;
        let device_name = String::from_utf16_lossy(
            &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len())]
        );
        let rc = info.monitorInfo.rcMonitor;

        // Get adapter info via EnumDisplayDevicesW (first call with device name)
        let mut adapter_name = String::new();
        let mut adapter_dd: DISPLAY_DEVICEW = std::mem::zeroed();
        adapter_dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

        // Enumerate adapters to find the one matching this device name
        let mut adapter_idx = 0u32;
        loop {
            let mut dd: DISPLAY_DEVICEW = std::mem::zeroed();
            dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
            if !EnumDisplayDevicesW(
                windows::core::PCWSTR::null(),
                adapter_idx,
                &mut dd,
                0,
            ).as_bool() {
                break;
            }
            let name = String::from_utf16_lossy(
                &dd.DeviceName[..dd.DeviceName.iter().position(|&c| c == 0).unwrap_or(dd.DeviceName.len())]
            );
            if name == device_name {
                adapter_name = String::from_utf16_lossy(
                    &dd.DeviceString[..dd.DeviceString.iter().position(|&c| c == 0).unwrap_or(dd.DeviceString.len())]
                );
                adapter_dd = dd;
                break;
            }
            adapter_idx += 1;
        }

        // Get monitor info (second call: enumerate monitors attached to this adapter)
        let mut monitor_device_string = String::new();
        let mut monitor_device_id = String::new();
        let mut mon_dd: DISPLAY_DEVICEW = std::mem::zeroed();
        mon_dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        if EnumDisplayDevicesW(
            windows::core::PCWSTR(adapter_dd.DeviceName.as_ptr()),
            0,
            &mut mon_dd,
            0,
        ).as_bool() {
            monitor_device_string = String::from_utf16_lossy(
                &mon_dd.DeviceString[..mon_dd.DeviceString.iter().position(|&c| c == 0).unwrap_or(mon_dd.DeviceString.len())]
            );
            monitor_device_id = String::from_utf16_lossy(
                &mon_dd.DeviceID[..mon_dd.DeviceID.iter().position(|&c| c == 0).unwrap_or(mon_dd.DeviceID.len())]
            );
        }

        serde_json::json!({
            "index": idx,
            "device_name": device_name,
            "is_primary": is_primary,
            "monitor_rect": {
                "left": rc.left,
                "top": rc.top,
                "right": rc.right,
                "bottom": rc.bottom,
                "width": rc.right - rc.left,
                "height": rc.bottom - rc.top,
            },
            "adapter_name": adapter_name,
            "monitor_device_string": monitor_device_string,
            "monitor_device_id": monitor_device_id,
        })
    }
}
