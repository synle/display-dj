// =========================================================================
// High-level display ops — convenience wrappers over the platform Platform impl.
// Vendored/distilled from cmd_get / cmd_set_all / cmd_set_one in display-dj-cli.
// =========================================================================

use super::*;

/// Enumerate all displays and re-read live brightness/contrast from hardware.
/// Equivalent to `cmd_get` with no filter — returns the same shape used by the UI.
pub fn list_all() -> Vec<DisplayInfo> {
    let displays = <PlatformImpl as Platform>::enumerate();
    let mut results: Vec<DisplayInfo> = Vec::with_capacity(displays.len());
    for (info, mut ctrl) in displays {
        let mut info = info;
        info.brightness = ctrl.get_brightness(); // re-read live values from hardware
        info.contrast = ctrl.get_contrast();
        results.push(info);
    }
    results
}

/// Set brightness on a single display, matched by id/name/builtin alias.
/// Returns true if the display was found and set_brightness succeeded.
pub fn set_one_brightness(id: &str, level: u16, mode: &str) -> bool {
    let displays = <PlatformImpl as Platform>::enumerate();
    for (info, mut ctrl) in displays {
        if matches_display(&info, id) {
            return ctrl.set_brightness(level, mode);
        }
    }
    false
}

/// Set brightness on all displays. Returns (id, success) for each.
pub fn set_all_brightness(level: u16, mode: &str) -> Vec<(String, bool)> {
    let displays = <PlatformImpl as Platform>::enumerate();
    let mut results = Vec::with_capacity(displays.len());
    for (info, mut ctrl) in displays {
        let ok = ctrl.set_brightness(level, mode);
        results.push((info.id.clone(), ok));
    }
    results
}

/// Set contrast on a single display by id/name. Returns true on success.
pub fn set_one_contrast(id: &str, level: u16) -> bool {
    let displays = <PlatformImpl as Platform>::enumerate();
    for (info, mut ctrl) in displays {
        if matches_display(&info, id) {
            return ctrl.set_contrast(level);
        }
    }
    false
}

/// Set contrast on all displays. Returns (id, success) for each.
pub fn set_all_contrast(level: u16) -> Vec<(String, bool)> {
    let displays = <PlatformImpl as Platform>::enumerate();
    let mut results = Vec::with_capacity(displays.len());
    for (info, mut ctrl) in displays {
        let ok = ctrl.set_contrast(level);
        results.push((info.id.clone(), ok));
    }
    results
}
