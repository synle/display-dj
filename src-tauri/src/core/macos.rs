use super::{DisplayControl, DisplayInfo, Platform, BUILTIN_ID, VCP_BRIGHTNESS, VCP_CONTRAST};
use ddc::Ddc; // trait that provides get_vcp_feature / set_vcp_feature methods
use std::thread;
use std::time::Duration;

// =========================================================================
// CoreGraphics FFI — declare C functions from macOS frameworks.
// extern "C" tells Rust to use the C calling convention.
// #[link] tells the linker which framework to link against.
// These functions are only available at runtime on macOS.
// =========================================================================

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    // Sets the gamma ramp for a display using a formula (min, max, gamma per channel).
    // We use this for software dimming — set max to <1.0 to reduce brightness.
    fn CGSetDisplayTransferByFormula(
        display: u32,
        red_min: f32, red_max: f32, red_gamma: f32,
        green_min: f32, green_max: f32, green_gamma: f32,
        blue_min: f32, blue_max: f32, blue_gamma: f32,
    ) -> i32;
    // Resets gamma to the system default (ColorSync profile).
    fn CGDisplayRestoreColorSyncSettings();
    // Lists active displays — fills a buffer with display IDs.
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
    // Returns non-zero if the display is a built-in (laptop/iMac) panel.
    fn CGDisplayIsBuiltin(display: u32) -> i32;
}

// =========================================================================
// DisplayServices — Apple's private framework for built-in brightness.
// Not in any public SDK, so we load it at runtime via dlopen/dlsym
// (like require() for a .dylib that might not exist).
// =========================================================================

// Type aliases for the function pointer signatures we'll load.
type GetBrightnessFn = unsafe extern "C" fn(u32, *mut f32) -> i32;
type SetBrightnessFn = unsafe extern "C" fn(u32, f32) -> i32;

/// Holds resolved function pointers from the DisplayServices private framework.
/// Loaded once at startup via dlopen/dlsym. If loading fails, there's no built-in
/// brightness control (e.g., on a Mac desktop with no built-in display).
struct DisplayServicesFns {
    get: GetBrightnessFn, // DisplayServicesGetBrightness(displayID, &mut brightness) -> status
    set: SetBrightnessFn, // DisplayServicesSetBrightness(displayID, brightness) -> status
}

/// Try to load DisplayServices.framework and resolve the brightness functions.
/// Returns None if the framework doesn't exist or the symbols are missing.
fn load_display_services() -> Option<DisplayServicesFns> {
    // unsafe = "I'm doing something the compiler can't verify" — here, calling C functions
    // and transmuting raw pointers into typed function pointers.
    unsafe {
        let path = std::ffi::CString::new(
            "/System/Library/PrivateFrameworks/DisplayServices.framework/DisplayServices"
        ).ok()?; // .ok()? converts Result to Option and returns None on error
        let handle = libc::dlopen(path.as_ptr(), libc::RTLD_NOW);
        if handle.is_null() { return None; }
        let get_sym = libc::dlsym(handle, b"DisplayServicesGetBrightness\0".as_ptr() as *const _);
        let set_sym = libc::dlsym(handle, b"DisplayServicesSetBrightness\0".as_ptr() as *const _);
        if get_sym.is_null() || set_sym.is_null() { return None; }
        // transmute = reinterpret the raw void* pointer as a typed function pointer.
        // Dangerous but necessary for FFI with private frameworks.
        Some(DisplayServicesFns {
            get: std::mem::transmute(get_sym),
            set: std::mem::transmute(set_sym),
        })
    }
}

/// Find the CoreGraphics display ID for the built-in panel (if one exists).
fn find_builtin_display_id() -> Option<u32> {
    unsafe {
        let mut displays = [0u32; 10]; // stack-allocated fixed-size array
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

// =========================================================================
// Built-in display — uses DisplayServices for direct hardware brightness
// =========================================================================

/// Built-in (laptop/iMac) display controller.
/// Uses the private DisplayServices framework for direct hardware backlight control.
/// The display_id is a CoreGraphics display ID (from CGGetActiveDisplayList).
struct BuiltinControl {
    display_id: u32,          // CoreGraphics display ID for the built-in panel
    ds: DisplayServicesFns,   // resolved function pointers from DisplayServices.framework
}

/// DisplayControl implementation for built-in displays.
/// Mode parameter is ignored — built-in always uses DisplayServices (no DDC, no gamma).
/// Brightness is a 0.0–1.0 float internally, exposed as 0–100 percentage.
impl DisplayControl for BuiltinControl {
    fn get_brightness(&mut self) -> Option<u32> {
        let mut val: f32 = 0.0;
        // Call the function pointer stored in self.ds — extra parens needed: (self.ds.get)(...)
        let res = unsafe { (self.ds.get)(self.display_id, &mut val) };
        // DisplayServices returns brightness as 0.0-1.0 float, we convert to 0-100 percentage
        if res == 0 { Some((val * 100.0).round() as u32) } else { None }
    }

    fn get_contrast(&mut self) -> Option<u32> {
        None // built-in displays don't expose contrast via any macOS API
    }

    fn set_brightness(&mut self, value: u16, _mode: &str) -> bool {
        // _mode = unused param (underscore prefix silences the warning).
        // Built-in always uses DisplayServices regardless of mode.
        let res = unsafe { (self.ds.set)(self.display_id, value as f32 / 100.0) };
        res == 0
    }

    fn set_contrast(&mut self, _value: u16) -> bool {
        false // not supported on built-in
    }

    fn reset_gamma(&self) {}
}

// =========================================================================
// External monitor — DDC/CI for hardware control, CoreGraphics for gamma
// =========================================================================

/// External monitor controller for macOS.
/// Combines two APIs: ddc-macos crate (hardware DDC/CI over IOKit I2C) and
/// CoreGraphics gamma formula (software brightness). DDC uses IOAVServiceWriteI2C
/// on Apple Silicon or IOI2CSendRequest on Intel Macs.
struct ExternalControl {
    ddc_monitor: ddc_macos::Monitor, // from the ddc-macos crate — wraps IOKit I2C
    cg_display_id: u32,              // CoreGraphics display ID for gamma control
    ddc_supported: bool,             // true if initial VCP brightness read succeeded
}

/// DDC retry settings for stubborn monitors.
/// Some monitors (e.g., Acer XZ322QU V3) have flaky I2C buses that return
/// checksum errors intermittently. Retrying with delays between attempts
/// significantly improves reliability.
const DDC_WRITE_RETRIES: u32 = 5;   // writes need more retries than reads
const DDC_READ_RETRIES: u32 = 3;
const DDC_RETRY_DELAY_MS: u64 = 50; // ms between attempts — gives I2C bus time to settle

/// Try reading a VCP feature with retries and delays between attempts.
fn ddc_read_with_retry(mon: &mut ddc_macos::Monitor, vcp: u8) -> Option<(u16, u16)> {
    for attempt in 0..DDC_READ_RETRIES {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(DDC_RETRY_DELAY_MS));
        }
        if let Ok(val) = mon.get_vcp_feature(vcp) {
            return Some((val.value(), val.maximum()));
        }
    }
    None
}

/// Try writing a VCP feature with multiple attempts and delays.
/// Some monitors (e.g., Acer XZ322QU V3) need repeated I2C writes
/// before the hardware processes the command.
fn ddc_write_with_retry(mon: &mut ddc_macos::Monitor, vcp: u8, value: u16) -> bool {
    for attempt in 0..DDC_WRITE_RETRIES {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(DDC_RETRY_DELAY_MS));
        }
        if mon.set_vcp_feature(vcp, value).is_ok() {
            // Extra delay after successful write to let monitor process it
            thread::sleep(Duration::from_millis(DDC_RETRY_DELAY_MS));
            return true;
        }
    }
    false
}

/// DisplayControl implementation for external monitors on macOS.
/// Supports DDC/CI (hardware), CoreGraphics gamma (software), or both stacked.
/// DDC reads/writes use retry logic for monitors with flaky I2C communication.
impl DisplayControl for ExternalControl {
    fn get_brightness(&mut self) -> Option<u32> {
        ddc_read_with_retry(&mut self.ddc_monitor, VCP_BRIGHTNESS).map(|(cur, max)| {
            if max > 0 { ((cur as f64 / max as f64) * 100.0).round() as u32 } else { 50 }
        })
    }

    fn get_contrast(&mut self) -> Option<u32> {
        ddc_read_with_retry(&mut self.ddc_monitor, VCP_CONTRAST).map(|(cur, max)| {
            if max > 0 { ((cur as f64 / max as f64) * 100.0).round() as u32 } else { 50 }
        })
    }

    fn set_brightness(&mut self, value: u16, mode: &str) -> bool {
        let use_ddc = mode == "ddc" || mode == "force" || (mode == "auto" && self.ddc_supported);
        // In force mode, always try DDC (even if initial read failed — write might work with retries)
        let try_ddc = use_ddc || mode == "force";
        let use_gamma = mode == "gamma" || mode == "force" || (mode == "auto" && !self.ddc_supported);
        let mut ok = true;

        if try_ddc {
            let ddc_val = if value == 0 { 1 } else { value };
            if !ddc_write_with_retry(&mut self.ddc_monitor, VCP_BRIGHTNESS, ddc_val) {
                ok = false;
            }
        }

        if use_gamma {
            set_cg_gamma(self.cg_display_id, value as u32);
        }

        ok
    }

    fn set_contrast(&mut self, value: u16) -> bool {
        ddc_write_with_retry(&mut self.ddc_monitor, VCP_CONTRAST, value)
    }

    fn reset_gamma(&self) {
        set_cg_gamma(self.cg_display_id, 100); // 100% = no dimming
    }
}

/// Set software brightness via CoreGraphics gamma formula.
/// brightness 0-100 maps to gamma max 0.0-1.0 (linear dimming across RGB channels).
fn set_cg_gamma(display_id: u32, brightness: u32) {
    let val = (brightness.min(100) as f32) / 100.0;
    unsafe {
        // min=0, max=val, gamma=1 for each channel — linear ramp from black to val
        CGSetDisplayTransferByFormula(
            display_id,
            0.0, val, 1.0,
            0.0, val, 1.0,
            0.0, val, 1.0,
        );
    }
}

// =========================================================================
// Platform implementation — discovers all displays on this Mac
// =========================================================================

/// macOS platform implementation.
/// Discovers displays by combining DisplayServices (built-in) with ddc-macos (external).
/// Unlike Windows, macOS doesn't have a dedup problem — DisplayServices and DDC enumerate
/// from different sources (private framework vs IOKit), and the built-in panel only appears
/// in DisplayServices while DDC only lists external monitors with I2C interfaces.
pub struct MacPlatform;

impl Platform for MacPlatform {
    /// Enumerate all displays on this Mac.
    /// Built-in: DisplayServices private framework (if available).
    /// External: ddc-macos crate via IOKit I2C services.
    fn enumerate() -> Vec<(DisplayInfo, Box<dyn DisplayControl>)> {
        let mut result: Vec<(DisplayInfo, Box<dyn DisplayControl>)> = Vec::new();

        // --- Built-in display ---
        // Only add if we can load DisplayServices AND find the built-in panel
        if let Some(ds) = load_display_services() {
            if let Some(display_id) = find_builtin_display_id() {
                let mut ctrl = BuiltinControl { display_id, ds };
                let brightness = ctrl.get_brightness();
                let info = DisplayInfo {
                    id: BUILTIN_ID.into(),       // .into() converts &str to String
                    name: "Built-in Display".into(),
                    display_type: "builtin".into(),
                    brightness,                  // field shorthand (like JS { brightness })
                    contrast: None,
                    ddc_supported: false,
                    // Built-in Apple displays dim natively via DisplayServices,
                    // so the soft-overlay fallback never runs on them.
                    monitor_rect: None,
                };
                // Box::new() allocates on the heap — required because trait objects have
                // unknown size at compile time (different structs can impl DisplayControl).
                result.push((info, Box::new(ctrl)));
            }
        }

        // --- External displays ---
        // ddc_macos::Monitor::enumerate() finds all DDC-capable displays via IOKit
        if let Ok(monitors) = ddc_macos::Monitor::enumerate() {
            for (idx, mut mon) in monitors.into_iter().enumerate() {
                let cg_display_id = mon.handle().id;
                // .unwrap_or_else() provides a fallback name if EDID is unreadable
                let name = mon.product_name()
                    .unwrap_or_else(|| format!("External Display {}", idx + 1));

                // Try reading brightness/contrast to check if DDC actually works.
                // Some monitors report as DDC-capable but return errors on reads.
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
                let ddc_supported = brightness.is_some(); // if read worked, DDC is supported

                let info = DisplayInfo {
                    id: (idx + 1).to_string(), // external IDs start at "1"
                    name,
                    display_type: "external".into(),
                    brightness,
                    contrast,
                    ddc_supported,
                    // TODO(overlay-macos): populate the physical screen rect
                    // so the soft-overlay brightness fallback can run on
                    // macOS too. Read via `CGDisplayBounds(cg_display_id)`
                    // and convert to (left, top, width, height) in global
                    // physical pixels. Until this is filled in, `brightnessMode
                    // = "overlay"` is a no-op on macOS even though the
                    // dropdown is selectable.
                    monitor_rect: None,
                };

                let ctrl = ExternalControl { ddc_monitor: mon, cg_display_id, ddc_supported };
                result.push((info, Box::new(ctrl)));
            }
        }

        result
    }

    /// Reset gamma on all displays to their ColorSync profiles.
    /// A single CoreGraphics call handles every display at once.
    fn reset_all_gamma() {
        unsafe { CGDisplayRestoreColorSyncSettings(); }
    }

    /// Dump raw platform diagnostics for the `debug` command.
    /// Returns DisplayServices availability, CoreGraphics display list with IDs,
    /// and raw DDC VCP reads from each external monitor.
    fn debug_info() -> serde_json::Value {
        // --- DisplayServices framework ---
        let ds_loaded = load_display_services().is_some();
        let builtin_id = find_builtin_display_id();

        // DisplayServices raw brightness for built-in
        let mut ds_brightness_raw: Option<f32> = None;
        if let Some(ds) = load_display_services() {
            if let Some(did) = builtin_id {
                let mut val: f32 = 0.0;
                if unsafe { (ds.get)(did, &mut val) } == 0 {
                    ds_brightness_raw = Some(val);
                }
            }
        }

        // --- CoreGraphics active displays ---
        let mut cg_displays = Vec::new();
        unsafe {
            let mut displays = [0u32; 10];
            let mut count: u32 = 0;
            CGGetActiveDisplayList(10, displays.as_mut_ptr(), &mut count);
            for i in 0..count as usize {
                let did = displays[i];
                cg_displays.push(serde_json::json!({
                    "index": i,
                    "cg_display_id": did,
                    "is_builtin": CGDisplayIsBuiltin(did) != 0,
                }));
            }
        }

        // --- DDC monitors via ddc-macos ---
        let mut ddc_list = Vec::new();
        let ddc_error: Option<String>;
        match ddc_macos::Monitor::enumerate() {
            Ok(monitors) => {
                ddc_error = None;
                for (idx, mut mon) in monitors.into_iter().enumerate() {
                    let name = mon.product_name().unwrap_or_else(|| "unknown".into());
                    let handle_id = mon.handle().id;
                    let brightness_raw = mon.get_vcp_feature(VCP_BRIGHTNESS).ok().map(|v| {
                        serde_json::json!({"current": v.value(), "max": v.maximum()})
                    });
                    let contrast_raw = mon.get_vcp_feature(VCP_CONTRAST).ok().map(|v| {
                        serde_json::json!({"current": v.value(), "max": v.maximum()})
                    });
                    ddc_list.push(serde_json::json!({
                        "index": idx,
                        "product_name": name,
                        "cg_display_id": handle_id,
                        "vcp_brightness": brightness_raw,
                        "vcp_contrast": contrast_raw,
                    }));
                }
            }
            Err(e) => {
                ddc_error = Some(format!("{}", e));
            }
        }

        serde_json::json!({
            "display_services_loaded": ds_loaded,
            "builtin_cg_display_id": builtin_id,
            "builtin_brightness_raw": ds_brightness_raw,
            "cg_display_count": cg_displays.len(),
            "cg_displays": cg_displays,
            "ddc_monitor_count": ddc_list.len(),
            "ddc_monitors": ddc_list,
            "ddc_enumerate_error": ddc_error,
        })
    }
}
