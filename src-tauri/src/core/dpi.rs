//! Per-display UI scaling ("DPI") read/write — **beta**.
//!
//! Display-dj-local (not vendored from display-dj-cli). One cross-platform
//! surface over three very different OS mechanisms:
//!
//! - **macOS**: scaling is a display *mode* choice. Each HiDPI mode "looks like"
//!   a logical resolution; scale % = native pixel width / logical width. Applied
//!   with `CGConfigureDisplayWithDisplayMode` (permanent).
//! - **Windows**: the undocumented-but-stable `DisplayConfigGetDeviceInfo` /
//!   `DisplayConfigSetDeviceInfo` packet types `-3` / `-4` (what Settings →
//!   Display → Scale uses). Values are indices relative to the recommended scale
//!   within [`WINDOWS_DPI_STEPS`].
//! - **Linux/X11**: `xrandr --output <name> --scale` (software scaling; may look
//!   soft). Wayland sessions are reported as unsupported.
//!
//! Every value is a UI scale percent: 100 = native, 200 = everything twice as
//! large. The app-wide allowed band is `[DPI_ABSOLUTE_MIN, DPI_ABSOLUTE_MAX]`.

use serde::Serialize;

/// Hard lower bound for any user-selectable scale percent.
pub const DPI_ABSOLUTE_MIN: u32 = 50;
/// Hard upper bound for any user-selectable scale percent.
pub const DPI_ABSOLUTE_MAX: u32 = 500;
/// Default user lower bound (Linux only; preferences.json `dpiMinPercent`).
pub const DPI_DEFAULT_MIN: u32 = 60;
/// Default user upper bound (Linux only; preferences.json `dpiMaxPercent`).
pub const DPI_DEFAULT_MAX: u32 = 250;

/// Scale steps Windows exposes, in order. Relative DPI indices address this table.
pub const WINDOWS_DPI_STEPS: [u32; 12] =
    [100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500];

/// Smallest step interval (percent) for continuous-scale backends.
pub const DPI_STEP_MIN: u32 = 1;
/// Largest step interval (percent) for continuous-scale backends.
pub const DPI_STEP_MAX: u32 = 100;
/// Default step interval (percent) for continuous-scale backends.
pub const DPI_DEFAULT_STEP: u32 = 5;

/// Clamps a user step interval into `[DPI_STEP_MIN, DPI_STEP_MAX]`.
pub fn clamp_dpi_step(step: u32) -> u32 {
    step.clamp(DPI_STEP_MIN, DPI_STEP_MAX)
}

/// One display as seen by the scaling backend.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DpiDisplay {
    /// Backend-specific stable-for-this-session identifier passed back to [`set_scale`].
    pub id: String,
    /// Human-readable name (OS-provided monitor name, or a fallback).
    pub name: String,
    /// Current UI scale percent, if it could be determined.
    pub current: Option<u32>,
    /// Every discrete scale percent this display supports, ascending (unfiltered).
    /// Empty for continuous backends.
    pub options: Vec<u32>,
    /// True when any percent in `[DPI_ABSOLUTE_MIN, DPI_ABSOLUTE_MAX]` works
    /// (Linux/xrandr); the UI then builds options from the user's min/max/step.
    /// macOS (display modes) and Windows (fixed OS steps) are discrete.
    pub continuous: bool,
}

/// Clamps a user min/max pair into `[DPI_ABSOLUTE_MIN, DPI_ABSOLUTE_MAX]` and
/// guarantees `min <= max` (an inverted pair collapses max down to min).
///
/// # Returns
/// `(min, max)` safe to feed to `u32::clamp`.
pub fn clamp_dpi_range(min: u32, max: u32) -> (u32, u32) {
    let min = min.clamp(DPI_ABSOLUTE_MIN, DPI_ABSOLUTE_MAX);
    let max = max.clamp(DPI_ABSOLUTE_MIN, DPI_ABSOLUTE_MAX).max(min);
    (min, max)
}

/// Returns true when `percent` is inside the user band after clamping.
pub fn is_within_range(percent: u32, min: u32, max: u32) -> bool {
    let (min, max) = clamp_dpi_range(min, max);
    (min..=max).contains(&percent)
}

/// Rounds a native/logical width ratio to a whole scale percent.
///
/// # Returns
/// `None` when `logical_width` is 0.
pub fn scale_percent(native_width: u64, logical_width: u64) -> Option<u32> {
    if logical_width == 0 {
        return None;
    }
    Some(((native_width as f64 / logical_width as f64) * 100.0).round() as u32)
}

/// Decodes a Windows `DISPLAYCONFIG_SOURCE_DPI_SCALE_GET` packet.
///
/// `min_rel` is always `<= 0` and its magnitude is the recommended step index.
///
/// # Returns
/// `(current_percent, supported_percents)`; `None` when indices fall outside the table.
pub fn windows_scale_options(min_rel: i32, cur_rel: i32, max_rel: i32) -> Option<(u32, Vec<u32>)> {
    let recommended = min_rel.unsigned_abs() as i64;
    let at = |rel: i32| -> Option<u32> {
        let idx = recommended + rel as i64;
        usize::try_from(idx)
            .ok()
            .and_then(|i| WINDOWS_DPI_STEPS.get(i).copied())
    };
    let current = at(cur_rel)?;
    let options = (min_rel..=max_rel).filter_map(at).collect();
    Some((current, options))
}

/// Converts a target percent into the relative index Windows expects on set.
///
/// # Returns
/// `None` when the percent is not one of [`WINDOWS_DPI_STEPS`].
pub fn windows_relative_for(target: u32, min_rel: i32) -> Option<i32> {
    let idx = WINDOWS_DPI_STEPS.iter().position(|v| *v == target)? as i32;
    Some(idx - min_rel.abs())
}

/// One connected, active output parsed from `xrandr --query`.
#[derive(Debug, Clone, PartialEq)]
pub struct XrandrOutput {
    pub name: String,
    /// Framebuffer area width (after `--scale`).
    pub logical_width: u64,
    /// Active mode width (the `*`-marked mode).
    pub mode_width: u64,
}

/// Parses `xrandr --query` output into active outputs.
///
/// Connected line: `HDMI-1 connected primary 2560x1440+0+0 (...)`.
/// Active mode line: `   1920x1080     60.00*+`.
pub fn parse_xrandr_query(text: &str) -> Vec<XrandrOutput> {
    let mut out = Vec::new();
    let mut pending: Option<(String, u64)> = None;
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) {
            pending = None;
            let mut parts = line.split_whitespace();
            let Some(name) = parts.next() else { continue };
            if parts.next() != Some("connected") {
                continue;
            }
            let geometry = parts.find(|p| p.contains('x') && p.contains('+'));
            if let Some(w) = geometry
                .and_then(|g| g.split('x').next())
                .and_then(|w| w.parse().ok())
            {
                pending = Some((name.to_string(), w));
            }
            continue;
        }
        if !line.contains('*') {
            continue;
        }
        if let Some((name, logical_width)) = pending.take() {
            let mode = line.split_whitespace().next().unwrap_or("");
            if let Some(mode_width) = mode.split('x').next().and_then(|w| w.parse().ok()) {
                out.push(XrandrOutput {
                    name,
                    logical_width,
                    mode_width,
                });
            }
        }
    }
    out
}

/// Lists every display the scaling backend can see.
///
/// # Errors
/// Platform unsupported (e.g. Wayland) or OS query failure.
pub fn list_displays() -> Result<Vec<DpiDisplay>, String> {
    platform::list_displays()
}

/// Applies `percent` to display `id`.
///
/// # Errors
/// Out of `[DPI_ABSOLUTE_MIN, DPI_ABSOLUTE_MAX]`, unknown display, unsupported
/// percent for that display, or the OS rejected the change.
pub fn set_scale(id: &str, percent: u32) -> Result<(), String> {
    if !(DPI_ABSOLUTE_MIN..=DPI_ABSOLUTE_MAX).contains(&percent) {
        return Err(format!(
            "scale {}% outside allowed range {}-{}%",
            percent, DPI_ABSOLUTE_MIN, DPI_ABSOLUTE_MAX
        ));
    }
    platform::set_scale(id, percent)
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
mod platform {
    use super::{scale_percent, DpiDisplay};
    use std::collections::BTreeMap;
    use std::ffi::c_void;

    type CFTypeRef = *const c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
        fn CGDisplayIsBuiltin(display: u32) -> i32;
        fn CGDisplayCopyAllDisplayModes(display: u32, options: CFTypeRef) -> CFTypeRef;
        fn CGDisplayCopyDisplayMode(display: u32) -> CFTypeRef;
        fn CGDisplayModeGetWidth(mode: CFTypeRef) -> usize;
        fn CGDisplayModeGetPixelWidth(mode: CFTypeRef) -> usize;
        fn CGDisplayModeGetHeight(mode: CFTypeRef) -> usize;
        fn CGDisplayModeGetRefreshRate(mode: CFTypeRef) -> f64;
        fn CGDisplayModeIsUsableForDesktopGUI(mode: CFTypeRef) -> bool;
        fn CGDisplayModeRelease(mode: CFTypeRef);
        fn CGDisplayModeGetIOFlags(mode: CFTypeRef) -> u32;
        fn CGBeginDisplayConfiguration(config: *mut *mut c_void) -> i32;
        fn CGConfigureDisplayWithDisplayMode(
            config: *mut c_void,
            display: u32,
            mode: CFTypeRef,
            options: CFTypeRef,
        ) -> i32;
        fn CGCompleteDisplayConfiguration(config: *mut c_void, option: u32) -> i32;
        fn CGCancelDisplayConfiguration(config: *mut c_void) -> i32;
        static kCGDisplayShowDuplicateLowResolutionModes: CFTypeRef;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(array: CFTypeRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFTypeRef, idx: isize) -> CFTypeRef;
        fn CFRelease(cf: CFTypeRef);
    }

    /// `kCGConfigurePermanently` — survive logout/reboot like System Settings does.
    const CG_CONFIGURE_PERMANENTLY: u32 = 2;
    const MAX_DISPLAYS: usize = 16;
    /// IOKit `kDisplayModeNativeFlag` — marks the panel's native resolution.
    const DISPLAY_MODE_NATIVE_FLAG: u32 = 0x0200_0000;

    /// Mode summary used to pick one mode per scale percent.
    struct ModeInfo {
        index: isize,
        width: usize,
        height: usize,
        pixel_width: usize,
        refresh: f64,
    }

    /// `{kCGDisplayShowDuplicateLowResolutionModes: YES}` — without it macOS
    /// hides the HiDPI ("looks like") modes. Autoreleased NSDictionary, toll-free bridged.
    unsafe fn hidpi_options() -> CFTypeRef {
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};
        let (Some(dict_cls), Some(num_cls)) = (Class::get("NSDictionary"), Class::get("NSNumber")) else {
            return std::ptr::null();
        };
        let yes: *mut Object = msg_send![num_cls, numberWithBool: true];
        let key = kCGDisplayShowDuplicateLowResolutionModes as *mut Object;
        let dict: *mut Object = msg_send![dict_cls, dictionaryWithObject: yes forKey: key];
        dict as CFTypeRef
    }

    /// Localized screen names keyed by CGDirectDisplayID (`NSScreen.localizedName`, 10.15+).
    unsafe fn screen_names() -> BTreeMap<u32, String> {
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};
        let mut names = BTreeMap::new();
        let (Some(screen_cls), Some(str_cls)) = (Class::get("NSScreen"), Class::get("NSString"))
        else {
            return names;
        };
        let screens: *mut Object = msg_send![screen_cls, screens];
        if screens.is_null() {
            return names;
        }
        let key_c = b"NSScreenNumber\0";
        let key: *mut Object = msg_send![str_cls, stringWithUTF8String: key_c.as_ptr()];
        let count: usize = msg_send![screens, count];
        for i in 0..count {
            let screen: *mut Object = msg_send![screens, objectAtIndex: i];
            let desc: *mut Object = msg_send![screen, deviceDescription];
            let num: *mut Object = msg_send![desc, objectForKey: key];
            if num.is_null() {
                continue;
            }
            let id: u32 = msg_send![num, unsignedIntValue];
            let responds: bool = msg_send![screen, respondsToSelector: sel!(localizedName)];
            if !responds {
                continue;
            }
            let name: *mut Object = msg_send![screen, localizedName];
            if name.is_null() {
                continue;
            }
            let utf8: *const std::os::raw::c_char = msg_send![name, UTF8String];
            if !utf8.is_null() {
                names.insert(
                    id,
                    std::ffi::CStr::from_ptr(utf8)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        names
    }

    fn active_displays() -> Vec<u32> {
        let mut ids = [0u32; MAX_DISPLAYS];
        let mut count = 0u32;
        let err =
            unsafe { CGGetActiveDisplayList(MAX_DISPLAYS as u32, ids.as_mut_ptr(), &mut count) };
        if err != 0 {
            return Vec::new();
        }
        ids[..count as usize].to_vec()
    }

    /// Summaries of every desktop-usable mode plus the native pixel width.
    ///
    /// Native = the mode IOKit flags native; fallback = widest 1x mode
    /// (oversampled HiDPI modes can exceed the panel's real pixel width).
    unsafe fn modes(all: CFTypeRef) -> (Vec<ModeInfo>, usize) {
        let n = CFArrayGetCount(all);
        let mut out = Vec::new();
        let (mut flagged, mut widest_1x) = (0usize, 0usize);
        for index in 0..n {
            let m = CFArrayGetValueAtIndex(all, index);
            if !CGDisplayModeIsUsableForDesktopGUI(m) {
                continue;
            }
            let info = ModeInfo {
                index,
                width: CGDisplayModeGetWidth(m),
                height: CGDisplayModeGetHeight(m),
                pixel_width: CGDisplayModeGetPixelWidth(m),
                refresh: CGDisplayModeGetRefreshRate(m),
            };
            if CGDisplayModeGetIOFlags(m) & DISPLAY_MODE_NATIVE_FLAG != 0 {
                flagged = flagged.max(info.pixel_width);
            }
            if info.pixel_width == info.width {
                widest_1x = widest_1x.max(info.pixel_width);
            }
            out.push(info);
        }
        (out, if flagged > 0 { flagged } else { widest_1x })
    }

    /// Picks the best mode index per scale percent: prefer HiDPI (pixel = 2×
    /// point width), then matching refresh rate, then larger height.
    fn best_by_percent(
        modes: &[ModeInfo],
        native: usize,
        current_refresh: f64,
    ) -> BTreeMap<u32, isize> {
        let mut best: BTreeMap<u32, &ModeInfo> = BTreeMap::new();
        let score = |m: &ModeInfo| {
            (
                (m.pixel_width == m.width * 2) as u8,
                ((m.refresh - current_refresh).abs() < 0.5) as u8,
                m.height,
            )
        };
        for m in modes {
            let Some(p) = scale_percent(native as u64, m.width as u64) else {
                continue;
            };
            match best.get(&p) {
                Some(prev) if score(prev) >= score(m) => {}
                _ => {
                    best.insert(p, m);
                }
            }
        }
        best.into_iter().map(|(p, m)| (p, m.index)).collect()
    }

    pub fn list_displays() -> Result<Vec<DpiDisplay>, String> {
        let names = unsafe { screen_names() };
        let mut out = Vec::new();
        for (n, id) in active_displays().into_iter().enumerate() {
            unsafe {
                let all = CGDisplayCopyAllDisplayModes(id, hidpi_options());
                if all.is_null() {
                    continue;
                }
                let (modes, native) = modes(all);
                let cur = CGDisplayCopyDisplayMode(id);
                let (current, refresh) = if cur.is_null() {
                    (None, 0.0)
                } else {
                    let r = (
                        scale_percent(native as u64, CGDisplayModeGetWidth(cur) as u64),
                        CGDisplayModeGetRefreshRate(cur),
                    );
                    CGDisplayModeRelease(cur);
                    r
                };
                let options: Vec<u32> = best_by_percent(&modes, native, refresh)
                    .into_keys()
                    .collect();
                CFRelease(all);
                let fallback = if CGDisplayIsBuiltin(id) != 0 {
                    "Built-in Display".to_string()
                } else {
                    format!("Display {}", n + 1)
                };
                out.push(DpiDisplay {
                    id: id.to_string(),
                    name: names.get(&id).cloned().unwrap_or(fallback),
                    current,
                    options,
                    continuous: false,
                });
            }
        }
        Ok(out)
    }

    pub fn set_scale(id: &str, percent: u32) -> Result<(), String> {
        let display: u32 = id
            .parse()
            .map_err(|_| format!("invalid display id: {}", id))?;
        if !active_displays().contains(&display) {
            return Err(format!("display {} not found", id));
        }
        unsafe {
            let all = CGDisplayCopyAllDisplayModes(display, hidpi_options());
            if all.is_null() {
                return Err("CGDisplayCopyAllDisplayModes returned null".into());
            }
            let (modes, native) = modes(all);
            let cur = CGDisplayCopyDisplayMode(display);
            let refresh = if cur.is_null() {
                0.0
            } else {
                CGDisplayModeGetRefreshRate(cur)
            };
            if !cur.is_null() {
                CGDisplayModeRelease(cur);
            }
            let Some(&index) = best_by_percent(&modes, native, refresh).get(&percent) else {
                CFRelease(all);
                return Err(format!("{}% not supported on display {}", percent, id));
            };
            let mode = CFArrayGetValueAtIndex(all, index);
            let mut config: *mut c_void = std::ptr::null_mut();
            let mut err = CGBeginDisplayConfiguration(&mut config);
            if err == 0 {
                err = CGConfigureDisplayWithDisplayMode(config, display, mode, std::ptr::null());
                err = if err == 0 {
                    CGCompleteDisplayConfiguration(config, CG_CONFIGURE_PERMANENTLY)
                } else {
                    CGCancelDisplayConfiguration(config);
                    err
                };
            }
            CFRelease(all);
            if err != 0 {
                return Err(format!(
                    "CoreGraphics display configuration failed ({})",
                    err
                ));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------
#[cfg(target_os = "windows")]
mod platform {
    use super::{windows_relative_for, windows_scale_options, DpiDisplay};
    use windows::Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, DisplayConfigSetDeviceInfo, GetDisplayConfigBufferSizes,
        QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
        DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_DEVICE_INFO_TYPE, DISPLAYCONFIG_MODE_INFO,
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL, DISPLAYCONFIG_PATH_INFO,
        DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    };
    use windows::Win32::Foundation::{ERROR_SUCCESS, LUID};

    /// Undocumented packet type: read source DPI scale (used by Settings app).
    const GET_DPI_SCALE: i32 = -3;
    /// Undocumented packet type: write source DPI scale.
    const SET_DPI_SCALE: i32 = -4;

    #[repr(C)]
    struct DpiGet {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        min_rel: i32,
        cur_rel: i32,
        max_rel: i32,
    }

    #[repr(C)]
    struct DpiSet {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        scale_rel: i32,
    }

    fn header(kind: i32, size: usize, adapter: LUID, id: u32) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
        DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_TYPE(kind),
            size: size as u32,
            adapterId: adapter,
            id,
        }
    }

    fn encode_id(adapter: LUID, source: u32) -> String {
        format!("{}:{}:{}", adapter.HighPart, adapter.LowPart, source)
    }

    fn active_paths() -> Result<Vec<DISPLAYCONFIG_PATH_INFO>, String> {
        unsafe {
            let (mut np, mut nm) = (0u32, 0u32);
            let rc = GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm);
            if rc != ERROR_SUCCESS {
                return Err(format!("GetDisplayConfigBufferSizes failed ({:?})", rc));
            }
            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
            let rc = QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut np,
                paths.as_mut_ptr(),
                &mut nm,
                modes.as_mut_ptr(),
                None,
            );
            if rc != ERROR_SUCCESS {
                return Err(format!("QueryDisplayConfig failed ({:?})", rc));
            }
            paths.truncate(np as usize);
            Ok(paths)
        }
    }

    fn read_dpi(adapter: LUID, source: u32) -> Option<DpiGet> {
        let mut pkt = DpiGet {
            header: header(
                GET_DPI_SCALE,
                std::mem::size_of::<DpiGet>(),
                adapter,
                source,
            ),
            min_rel: 0,
            cur_rel: 0,
            max_rel: 0,
        };
        let rc = unsafe { DisplayConfigGetDeviceInfo(&mut pkt.header) };
        (rc == 0).then_some(pkt)
    }

    fn target_name(path: &DISPLAYCONFIG_PATH_INFO) -> Option<String> {
        let mut pkt = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
        pkt.header = header(
            DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME.0,
            std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>(),
            path.targetInfo.adapterId,
            path.targetInfo.id,
        );
        if unsafe { DisplayConfigGetDeviceInfo(&mut pkt.header) } != 0 {
            return None;
        }
        let raw = &pkt.monitorFriendlyDeviceName;
        let len = raw.iter().position(|c| *c == 0).unwrap_or(raw.len());
        let name = String::from_utf16_lossy(&raw[..len]);
        (!name.trim().is_empty()).then_some(name)
    }

    pub fn list_displays() -> Result<Vec<DpiDisplay>, String> {
        let mut out: Vec<DpiDisplay> = Vec::new();
        for (n, path) in active_paths()?.iter().enumerate() {
            let adapter = path.sourceInfo.adapterId;
            let source = path.sourceInfo.id;
            let id = encode_id(adapter, source);
            if out.iter().any(|d| d.id == id) {
                continue; // clone/mirror topology: one source, many targets
            }
            let Some(pkt) = read_dpi(adapter, source) else {
                continue;
            };
            let Some((current, options)) =
                windows_scale_options(pkt.min_rel, pkt.cur_rel, pkt.max_rel)
            else {
                continue;
            };
            let fallback =
                if path.targetInfo.outputTechnology == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL {
                    "Built-in Display".to_string()
                } else {
                    format!("Display {}", n + 1)
                };
            out.push(DpiDisplay {
                id,
                name: target_name(path).unwrap_or(fallback),
                current: Some(current),
                options,
                continuous: false,
            });
        }
        Ok(out)
    }

    pub fn set_scale(id: &str, percent: u32) -> Result<(), String> {
        let path = active_paths()?
            .into_iter()
            .find(|p| encode_id(p.sourceInfo.adapterId, p.sourceInfo.id) == id)
            .ok_or_else(|| format!("display {} not found", id))?;
        let (adapter, source) = (path.sourceInfo.adapterId, path.sourceInfo.id);
        let pkt = read_dpi(adapter, source).ok_or("reading current DPI scale failed")?;
        let rel = windows_relative_for(percent, pkt.min_rel)
            .filter(|r| (pkt.min_rel..=pkt.max_rel).contains(r))
            .ok_or_else(|| format!("{}% not supported on display {}", percent, id))?;
        let set = DpiSet {
            header: header(
                SET_DPI_SCALE,
                std::mem::size_of::<DpiSet>(),
                adapter,
                source,
            ),
            scale_rel: rel,
        };
        let rc = unsafe { DisplayConfigSetDeviceInfo(&set.header) };
        if rc != 0 {
            return Err(format!("DisplayConfigSetDeviceInfo failed ({})", rc));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Linux (X11 via xrandr)
// ---------------------------------------------------------------------------
#[cfg(target_os = "linux")]
mod platform {
    use super::{parse_xrandr_query, scale_percent, DpiDisplay};
    use std::process::Command;

    fn ensure_x11() -> Result<(), String> {
        let wayland = std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false);
        if wayland || std::env::var_os("DISPLAY").is_none() {
            return Err(
                "DPI scaling requires an X11 session (Wayland is not supported yet)".into(),
            );
        }
        Ok(())
    }

    fn query() -> Result<String, String> {
        let out = Command::new("xrandr")
            .arg("--query")
            .output()
            .map_err(|e| format!("xrandr: {}", e))?;
        if !out.status.success() {
            return Err(format!(
                "xrandr --query failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn list_displays() -> Result<Vec<DpiDisplay>, String> {
        ensure_x11()?;
        Ok(parse_xrandr_query(&query()?)
            .into_iter()
            .map(|o| DpiDisplay {
                current: scale_percent(o.mode_width, o.logical_width),
                id: o.name.clone(),
                name: o.name,
                options: Vec::new(),
                continuous: true,
            })
            .collect())
    }

    pub fn set_scale(id: &str, percent: u32) -> Result<(), String> {
        ensure_x11()?;
        if !parse_xrandr_query(&query()?).iter().any(|o| o.name == id) {
            return Err(format!("output {} not found", id));
        }
        // UI scale 200% == framebuffer half the mode size == xrandr --scale 0.5.
        let factor = 100.0 / percent as f64;
        let arg = format!("{:.4}x{:.4}", factor, factor);
        let out = Command::new("xrandr")
            .args(["--output", id, "--scale", &arg])
            .output()
            .map_err(|e| format!("xrandr: {}", e))?;
        if !out.status.success() {
            return Err(format!(
                "xrandr --scale failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod platform {
    use super::DpiDisplay;
    pub fn list_displays() -> Result<Vec<DpiDisplay>, String> {
        Err("DPI scaling is not supported on this platform".into())
    }
    pub fn set_scale(_id: &str, _percent: u32) -> Result<(), String> {
        Err("DPI scaling is not supported on this platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Range clamps both ends to the absolute caps and repairs inversion.
    #[test]
    fn clamp_dpi_range_bounds_and_orders() {
        assert_eq!(clamp_dpi_range(10, 999), (50, 500));
        assert_eq!(clamp_dpi_range(150, 100), (150, 150));
        assert_eq!(clamp_dpi_range(75, 200), (75, 200));
    }

    /// Band check honors the clamped range, inclusive.
    #[test]
    fn is_within_range_inclusive() {
        assert!(is_within_range(60, 60, 200));
        assert!(is_within_range(200, 60, 200));
        assert!(!is_within_range(225, 60, 200));
        assert!(!is_within_range(40, 0, 200));
    }

    /// 4K panel "looks like 1920" is 200%; zero width is rejected.
    #[test]
    fn scale_percent_rounds() {
        assert_eq!(scale_percent(3840, 1920), Some(200));
        assert_eq!(scale_percent(3840, 2560), Some(150));
        assert_eq!(scale_percent(2560, 2048), Some(125));
        assert_eq!(scale_percent(100, 0), None);
    }

    /// Recommended 150% (idx 2), current 125%, max 225%.
    #[test]
    fn windows_scale_options_decodes_relative_indices() {
        assert_eq!(
            windows_scale_options(-2, -1, 3),
            Some((125, vec![100, 125, 150, 175, 200, 225]))
        );
    }

    /// Out-of-table current index yields None.
    #[test]
    fn windows_scale_options_rejects_out_of_table() {
        assert_eq!(windows_scale_options(0, 20, 0), None);
    }

    /// Target percent maps back to an index relative to recommended.
    #[test]
    fn windows_relative_for_maps_back() {
        assert_eq!(windows_relative_for(100, -2), Some(-2));
        assert_eq!(windows_relative_for(200, -2), Some(2));
        assert_eq!(windows_relative_for(130, -2), None);
    }

    /// Parses active outputs and skips disconnected ones.
    #[test]
    fn parse_xrandr_query_reads_active_outputs() {
        let text = concat!(
            "Screen 0: minimum 8 x 8, current 3200 x 1080, maximum 32767 x 32767\n",
            "eDP-1 connected primary 1280x720+0+0 (normal left inverted right x axis y axis) 344mm x 194mm\n",
            "   2560x1440     60.00*+\n",
            "   1920x1080     60.00\n",
            "HDMI-1 connected 1920x1080+1280+0 (normal) 527mm x 296mm\n",
            "   1920x1080     60.00*+  50.00\n",
            "DP-1 disconnected (normal left inverted right x axis y axis)\n",
        );
        assert_eq!(
            parse_xrandr_query(text),
            vec![
                XrandrOutput {
                    name: "eDP-1".into(),
                    logical_width: 1280,
                    mode_width: 2560
                },
                XrandrOutput {
                    name: "HDMI-1".into(),
                    logical_width: 1920,
                    mode_width: 1920
                },
            ]
        );
    }

    /// Absolute caps reject out-of-band percents before touching the OS.
    #[test]
    fn set_scale_rejects_out_of_band() {
        assert!(set_scale("x", 49).unwrap_err().contains("outside"));
        assert!(set_scale("x", 501).unwrap_err().contains("outside"));
    }
}
