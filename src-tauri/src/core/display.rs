// =========================================================================
// High-level display ops — convenience wrappers over the platform Platform impl.
// Vendored/distilled from cmd_get / cmd_set_all / cmd_set_one in display-dj-cli.
// =========================================================================

use super::*;

/// Moves display controls across threads (cache + parallel writes).
///
/// SAFETY: controls wrap OS handles (IOKit/IOAVService refs on macOS, physical
/// monitor HANDLEs on Windows, device paths on Linux) that are valid on any
/// thread. Access is always exclusive: through the `CONTROL_CACHE` mutex, or
/// one `&mut` per scoped worker thread — never shared concurrently.
struct SendControl(Box<dyn DisplayControl>);
unsafe impl Send for SendControl {}

/// Enumerated display controls reused across writes, so a slider change is a
/// single DDC write instead of enumerate + EDID + 2 DDC reads + write.
/// Refreshed by `list_all()` (the hot-plug / refresh path) and dropped +
/// re-enumerated once whenever a write fails or its target is missing.
static CONTROL_CACHE: std::sync::Mutex<Option<Vec<(DisplayInfo, SendControl)>>> =
    std::sync::Mutex::new(None);

/// Enumerates displays fresh, wrapping controls for the cache.
fn enumerate_controls() -> Vec<(DisplayInfo, SendControl)> {
    <PlatformImpl as Platform>::enumerate()
        .into_iter()
        .map(|(info, ctrl)| (info, SendControl(ctrl)))
        .collect()
}

/// Runs `op` against cached controls (enumerating on a cold cache). When `op`
/// reports failure, drops the cache, re-enumerates once, and retries — which
/// covers unplugged/replugged panels and handles invalidated by sleep/wake.
fn with_controls<T>(
    op: impl Fn(&mut Vec<(DisplayInfo, SendControl)>) -> T,
    succeeded: impl Fn(&T) -> bool,
) -> T {
    let mut cache = CONTROL_CACHE.lock().unwrap_or_else(|p| p.into_inner());
    let was_cached = cache.is_some();
    let controls = cache.get_or_insert_with(enumerate_controls);
    let result = op(controls);
    if succeeded(&result) || !was_cached {
        return result;
    }
    log::info!("core::display: cached write failed, re-enumerating and retrying");
    let controls = cache.insert(enumerate_controls());
    op(controls)
}

/// Enumerate all displays and re-read live brightness/contrast from hardware.
/// Equivalent to `cmd_get` with no filter — returns the same shape used by the UI.
/// Also refreshes the write-control cache with the freshly enumerated handles.
pub fn list_all() -> Vec<DisplayInfo> {
    let mut displays = enumerate_controls();
    let mut results: Vec<DisplayInfo> = Vec::with_capacity(displays.len());
    for (info, ctrl) in displays.iter_mut() {
        let mut info = info.clone();
        info.brightness = ctrl.0.get_brightness(); // re-read live values from hardware
        info.contrast = ctrl.0.get_contrast();
        results.push(info);
    }
    *CONTROL_CACHE.lock().unwrap_or_else(|p| p.into_inner()) = Some(displays);
    results
}

/// Set brightness on a single display, matched by id/name/builtin alias.
/// Returns true if the display was found and set_brightness succeeded.
pub fn set_one_brightness(id: &str, level: u16, mode: &str) -> bool {
    with_controls(
        |controls| {
            controls
                .iter_mut()
                .find(|(info, _)| matches_display(info, id))
                .is_some_and(|(_, ctrl)| ctrl.0.set_brightness(level, mode))
        },
        |ok| *ok,
    )
}

/// Set brightness on all displays in parallel (one scoped thread per display,
/// since each sits on its own DDC bus). Returns (id, success) per display in
/// enumeration order.
pub fn set_all_brightness(level: u16, mode: &str) -> Vec<(String, bool)> {
    with_controls(
        |controls| {
            std::thread::scope(|scope| {
                let handles: Vec<_> = controls
                    .iter_mut()
                    .map(|(info, ctrl)| {
                        let handle = scope.spawn(move || ctrl.0.set_brightness(level, mode));
                        (info.id.clone(), handle)
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|(id, handle)| (id, handle.join().unwrap_or(false)))
                    .collect()
            })
        },
        |results: &Vec<(String, bool)>| results.iter().all(|(_, ok)| *ok),
    )
}

/// Set contrast on a single display by id/name. Returns true on success.
pub fn set_one_contrast(id: &str, level: u16) -> bool {
    with_controls(
        |controls| {
            controls
                .iter_mut()
                .find(|(info, _)| matches_display(info, id))
                .is_some_and(|(_, ctrl)| ctrl.0.set_contrast(level))
        },
        |ok| *ok,
    )
}

/// Set contrast on all displays. Returns (id, success) for each.
pub fn set_all_contrast(level: u16) -> Vec<(String, bool)> {
    with_controls(
        |controls| {
            controls
                .iter_mut()
                .map(|(info, ctrl)| (info.id.clone(), ctrl.0.set_contrast(level)))
                .collect()
        },
        |results: &Vec<(String, bool)>| results.iter().all(|(_, ok)| *ok),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: list_all must not panic. Returns Vec<DisplayInfo>; may be
    /// empty in CI (no display server) or non-empty on dev machines.
    #[test]
    fn test_list_all_smoke() {
        let displays = list_all();
        // Either empty (CI) or each entry has a non-empty id.
        for d in &displays {
            assert!(!d.id.is_empty(), "display has empty id");
        }
    }

    /// Smoke test: set_one_brightness with a non-existent id returns false.
    #[test]
    fn test_set_one_brightness_unknown_id() {
        let result = set_one_brightness("nonexistent_id_xyz_123", 50, "force");
        assert!(!result);
    }

    /// Smoke test: set_one_contrast with a non-existent id returns false.
    #[test]
    fn test_set_one_contrast_unknown_id() {
        let result = set_one_contrast("nonexistent_id_xyz_123", 50);
        assert!(!result);
    }

    /// Smoke test: set_all_brightness returns one entry per display.
    #[test]
    fn test_set_all_brightness_returns_per_display() {
        let results = set_all_brightness(50, "force");
        // Length matches number of displays; each entry has non-empty id.
        for (id, _ok) in &results {
            assert!(!id.is_empty());
        }
    }

    /// Smoke test: set_all_contrast returns one entry per display.
    #[test]
    fn test_set_all_contrast_returns_per_display() {
        let results = set_all_contrast(50);
        for (id, _ok) in &results {
            assert!(!id.is_empty());
        }
    }
}
