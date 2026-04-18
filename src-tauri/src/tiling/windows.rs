//! Windows window tiling via Win32 API.
//!
//! Provides window tiling (halves, thirds, quarters, maximize, restore, exposé)
//! using `GetForegroundWindow`, `SetWindowPos`, `EnumDisplayMonitors`, and
//! `EnumWindows`. No special permissions are required on Windows.

use super::{
    build_sorted_window_list, calculate_target_rect, find_display_for_window,
    layout_across_displays, layout_grid_on_display, Rect, TilingLayout, WindowInfo, WindowState,
};
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetWindowPos, ShowWindow, HWND_TOP,
    SWP_NOZORDER, SW_RESTORE,
};

// ---------------------------------------------------------------------------
// Display enumeration
// ---------------------------------------------------------------------------

/// Get work areas (visible frames) for all monitors, sorted left-to-right.
/// Uses `EnumDisplayMonitors` + `GetMonitorInfoW` to get the work area
/// (excludes taskbar and other app bars).
fn get_display_work_areas() -> Vec<Rect> {
    let mut areas: Vec<Rect> = Vec::new();

    unsafe {
        // Callback collects MONITORINFO.rcWork for each monitor
        unsafe extern "system" fn monitor_callback(
            hmonitor: HMONITOR,
            _hdc: HDC,
            _rect: *mut RECT,
            lparam: LPARAM,
        ) -> BOOL {
            let areas = &mut *(lparam.0 as *mut Vec<Rect>);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
                let rc = info.rcWork;
                areas.push(Rect {
                    x: rc.left as f64,
                    y: rc.top as f64,
                    width: (rc.right - rc.left) as f64,
                    height: (rc.bottom - rc.top) as f64,
                });
            }
            TRUE
        }

        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_callback),
            LPARAM(&mut areas as *mut Vec<Rect> as isize),
        );
    }

    // Sort left-to-right, then top-to-bottom
    areas.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });

    areas
}

// ---------------------------------------------------------------------------
// Window helpers
// ---------------------------------------------------------------------------

/// Get the HWND of the currently focused (foreground) window.
fn get_foreground_hwnd() -> Option<HWND> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        None
    } else {
        Some(hwnd)
    }
}

/// Get the visible frame position and size of a window.
/// Prefers DWM extended frame bounds (which exclude invisible DWM borders)
/// over `GetWindowRect` (which includes them). This ensures that
/// `find_display_for_window` and restore use the actual visible frame.
fn get_hwnd_rect(hwnd: HWND) -> Option<Rect> {
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    let mut rc = RECT::default();
    unsafe {
        // Try DWM extended frame bounds first (visible frame without invisible borders)
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rc as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_ok()
        {
            return Some(Rect {
                x: rc.left as f64,
                y: rc.top as f64,
                width: (rc.right - rc.left) as f64,
                height: (rc.bottom - rc.top) as f64,
            });
        }
        // Fall back to GetWindowRect
        if GetWindowRect(hwnd, &mut rc).is_ok() {
            Some(Rect {
                x: rc.left as f64,
                y: rc.top as f64,
                width: (rc.right - rc.left) as f64,
                height: (rc.bottom - rc.top) as f64,
            })
        } else {
            None
        }
    }
}

/// Get the invisible DWM border offsets (left, top, right, bottom).
/// On Windows 10/11, every window has ~7px invisible borders on each side
/// (DWM drop shadows). `GetWindowRect` includes these invisible borders,
/// and `SetWindowPos` positions including them. This function computes the
/// difference between the full window rect and the visible (extended) frame
/// so callers can compensate. Returns (0, 0, 0, 0) if DWM info is unavailable.
fn get_dwm_border(hwnd: HWND) -> (i32, i32, i32, i32) {
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    let mut window_rect = RECT::default();
    let mut frame_rect = RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return (0, 0, 0, 0);
        }
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut frame_rect as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_err()
        {
            return (0, 0, 0, 0);
        }
    }
    (
        frame_rect.left - window_rect.left,    // left border
        frame_rect.top - window_rect.top,      // top border
        window_rect.right - frame_rect.right,  // right border
        window_rect.bottom - frame_rect.bottom, // bottom border
    )
}

/// Move and resize a window to the given rect, compensating for invisible
/// DWM borders. The rect specifies the desired *visible* frame position and
/// size. Internally, `SetWindowPos` operates on the full window rect (which
/// includes invisible borders), so we expand the position/size by the border
/// offsets so the visible frame lands exactly where requested.
fn set_hwnd_rect(hwnd: HWND, rect: &Rect) {
    let (bl, bt, br, bb) = get_dwm_border(hwnd);
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            rect.x as i32 - bl,
            rect.y as i32 - bt,
            rect.width as i32 + bl + br,
            rect.height as i32 + bt + bb,
            SWP_NOZORDER,
        );
    }
}

/// Get the window title text.
fn get_window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        String::new()
    }
}

/// Get the process name (exe basename) for a window's owning process.
fn get_process_name(hwnd: HWND) -> String {
    let mut pid: u32 = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        return String::new();
    }

    // Open the process and query the image name
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        match handle {
            Ok(h) => {
                let mut buf = [0u16; 512];
                let mut size = buf.len() as u32;
                if QueryFullProcessImageNameW(
                    h,
                    PROCESS_NAME_FORMAT(0),
                    windows::core::PWSTR(buf.as_mut_ptr()),
                    &mut size,
                )
                .is_ok()
                {
                    let path = String::from_utf16_lossy(&buf[..size as usize]);
                    let _ = windows::Win32::Foundation::CloseHandle(h);
                    // Return just the exe name without extension
                    path.rsplit('\\')
                        .next()
                        .unwrap_or(&path)
                        .strip_suffix(".exe")
                        .unwrap_or_else(|| path.rsplit('\\').next().unwrap_or(&path))
                        .to_string()
                } else {
                    let _ = windows::Win32::Foundation::CloseHandle(h);
                    String::new()
                }
            }
            Err(_) => String::new(),
        }
    }
}

/// Query the minimum window size via WM_GETMINMAXINFO.
fn get_window_min_size(hwnd: HWND) -> Option<(f64, f64)> {
    use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, MINMAXINFO, WM_GETMINMAXINFO};
    let mut info = MINMAXINFO::default();
    unsafe {
        SendMessageW(
            hwnd,
            WM_GETMINMAXINFO,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(&mut info as *mut MINMAXINFO as isize),
        );
    }
    let w = info.ptMinTrackSize.x as f64;
    let h = info.ptMinTrackSize.y as f64;
    if w > 0.0 && h > 0.0 {
        Some((w, h))
    } else {
        None
    }
}

/// Enumerate all visible, non-minimized top-level windows.
/// Returns a list of WindowInfo with HWND stored as i64.
fn get_all_windows() -> Vec<WindowInfo> {
    let mut windows: Vec<WindowInfo> = Vec::new();

    unsafe {
        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

            // Skip invisible windows
            if !IsWindowVisible(hwnd).as_bool() {
                return TRUE;
            }

            // Skip minimized windows
            if IsIconic(hwnd).as_bool() {
                return TRUE;
            }

            // Get window rect and skip tiny windows
            let mut rc = RECT::default();
            if GetWindowRect(hwnd, &mut rc).is_err() {
                return TRUE;
            }
            let width = (rc.right - rc.left) as f64;
            let height = (rc.bottom - rc.top) as f64;
            if width < 50.0 || height < 50.0 {
                return TRUE;
            }

            // Skip windows with empty titles (background/system windows)
            let mut title_buf = [0u16; 512];
            let title_len = GetWindowTextW(hwnd, &mut title_buf);
            if title_len == 0 {
                return TRUE;
            }

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));

            // Get process name for grouping
            let owner_name = get_process_name_from_pid(pid);

            windows.push(WindowInfo {
                window_id: hwnd.0 as isize as i64,
                owner_pid: pid as i32,
                owner_name,
                bounds: Rect {
                    x: rc.left as f64,
                    y: rc.top as f64,
                    width,
                    height,
                },
                min_size: None,
            });

            TRUE
        }

        let _ = EnumWindows(
            Some(enum_callback),
            LPARAM(&mut windows as *mut Vec<WindowInfo> as isize),
        );
    }

    windows
}

/// Get process name from PID (used inside enum callback where we can't call
/// the method that takes HWND since we're already in the callback).
fn get_process_name_from_pid(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        match handle {
            Ok(h) => {
                let mut buf = [0u16; 512];
                let mut size = buf.len() as u32;
                if QueryFullProcessImageNameW(
                    h,
                    PROCESS_NAME_FORMAT(0),
                    windows::core::PWSTR(buf.as_mut_ptr()),
                    &mut size,
                )
                .is_ok()
                {
                    let path = String::from_utf16_lossy(&buf[..size as usize]);
                    let _ = windows::Win32::Foundation::CloseHandle(h);
                    path.rsplit('\\')
                        .next()
                        .unwrap_or(&path)
                        .strip_suffix(".exe")
                        .unwrap_or_else(|| path.rsplit('\\').next().unwrap_or(&path))
                        .to_string()
                } else {
                    let _ = windows::Win32::Foundation::CloseHandle(h);
                    String::new()
                }
            }
            Err(_) => String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Execute a tiling command on the focused window.
/// `layout_str` is a camelCase layout name (e.g. "leftHalf") or "restore".
pub fn execute_tile(app: &AppHandle, layout_str: &str) {
    // Read tiling preferences
    let (enabled, half_ratio, third_ratio, gap) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        (
            prefs.tiling.enabled,
            prefs.tiling.half_ratio,
            prefs.tiling.third_ratio,
            prefs.tiling.gap,
        )
    };

    if !enabled {
        log::info!("tiling: disabled in preferences");
        return;
    }

    // Handle restore
    if layout_str == "restore" {
        execute_restore(app);
        return;
    }

    // Parse layout
    let layout = match TilingLayout::parse(layout_str) {
        Some(l) => l,
        None => {
            log::warn!("tiling: unknown layout '{}'", layout_str);
            return;
        }
    };

    // Get focused window
    let hwnd = match get_foreground_hwnd() {
        Some(h) => h,
        None => {
            log::info!("tiling: no focused window");
            return;
        }
    };

    let win_rect = match get_hwnd_rect(hwnd) {
        Some(r) => r,
        None => {
            log::info!("tiling: could not get window rect");
            return;
        }
    };

    // Get displays
    let displays = get_display_work_areas();
    if displays.is_empty() {
        log::warn!("tiling: no displays found");
        return;
    }

    // Tile on the display the window is currently on
    let target_display = find_display_for_window(&win_rect, &displays);
    let window_key = hwnd.0 as isize as i64;

    // Save original position (only on first tile) and update state
    {
        let state = app.state::<crate::AppState>();
        let mut ts = state.tiling_state.lock().unwrap();
        let entry = ts.windows.entry(window_key).or_insert(WindowState {
            original: win_rect,
            layout,
            display_index: target_display,
        });
        entry.layout = layout;
        entry.display_index = target_display;
    }

    // Calculate and apply target rect
    let target = calculate_target_rect(layout, &displays[target_display], half_ratio, third_ratio, gap);
    log::info!(
        "tiling: {} on display {} -> ({}, {}, {}x{})",
        layout_str,
        target_display,
        target.x,
        target.y,
        target.width,
        target.height,
    );
    set_hwnd_rect(hwnd, &target);
}

/// Restore the focused window to its pre-tiled position and size.
fn execute_restore(app: &AppHandle) {
    let hwnd = match get_foreground_hwnd() {
        Some(h) => h,
        None => return,
    };
    let window_key = hwnd.0 as isize as i64;

    // Remove state and get original rect
    let original = {
        let state = app.state::<crate::AppState>();
        let mut ts = state.tiling_state.lock().unwrap();
        ts.windows.remove(&window_key).map(|ws| ws.original)
    };

    if let Some(rect) = original {
        log::info!(
            "tiling: restore -> ({}, {}, {}x{})",
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        );
        set_hwnd_rect(hwnd, &rect);
    } else {
        log::info!("tiling: no saved state to restore");
    }
}

/// Execute the exposé command. Lays out all on-screen windows in a grid.
pub fn execute_expose(app: &AppHandle) {
    let (max_per_display, gap, spread) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        ((prefs.tiling.expose_columns * prefs.tiling.expose_rows) as usize, prefs.tiling.gap, prefs.tiling.expose_layout_strategy == "spread")
    };

    // Restore minimized windows first
    restore_minimized_windows();

    let mut all_windows = get_all_windows();
    if all_windows.is_empty() {
        log::info!("expose: no windows found");
        return;
    }

    // Populate min sizes via WM_GETMINMAXINFO for adaptive grid layout
    for w in &mut all_windows {
        let hwnd = HWND(w.window_id as isize as *mut _);
        w.min_size = get_window_min_size(hwnd);
    }

    let displays = get_display_work_areas();
    if displays.is_empty() {
        return;
    }

    let total_cap = max_per_display * displays.len();
    let ordered = build_sorted_window_list(&all_windows, total_cap);
    if ordered.is_empty() {
        return;
    }

    let g = gap as f64;
    let placed = layout_across_displays(&ordered, &displays, max_per_display, g, spread, &|win_info, rect| {
        let hwnd = HWND(win_info.window_id as isize as *mut _);
        set_hwnd_rect(hwnd, rect);
        unsafe { let _ = BringWindowToTop(hwnd); }
    });

    log::info!(
        "expose: spread {} windows across {} displays",
        placed,
        displays.len()
    );
}

/// Execute the app exposé command. Lays out the frontmost app's windows in a grid.
pub fn execute_expose_app(app: &AppHandle) {
    let (max_per_display, gap, spread) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        ((prefs.tiling.expose_columns * prefs.tiling.expose_rows) as usize, prefs.tiling.gap, prefs.tiling.expose_layout_strategy == "spread")
    };

    // Identify the target app (foreground window's process)
    let hwnd = match get_foreground_hwnd() {
        Some(h) => h,
        None => {
            log::info!("app_expose: no focused window");
            return;
        }
    };

    let mut target_pid: u32 = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut target_pid));
    }
    let target_app = get_process_name(hwnd);

    // Restore minimized windows
    restore_minimized_windows();

    let mut all_windows = get_all_windows();
    if all_windows.is_empty() {
        return;
    }

    // Populate min sizes via WM_GETMINMAXINFO for adaptive grid layout
    for w in &mut all_windows {
        let hwnd = HWND(w.window_id as isize as *mut _);
        w.min_size = get_window_min_size(hwnd);
    }

    let displays = get_display_work_areas();
    if displays.is_empty() {
        return;
    }

    let total_cap = max_per_display * displays.len();

    // Filter to target app's windows
    let mut app_windows: Vec<&WindowInfo> = all_windows
        .iter()
        .filter(|w| w.owner_pid == target_pid as i32)
        .take(total_cap)
        .collect();
    app_windows.sort_by_key(|w| w.window_id);

    if app_windows.is_empty() {
        return;
    }

    let g = gap as f64;
    let set_rect_fn = |win_info: &WindowInfo, rect: &Rect| {
        let hwnd = HWND(win_info.window_id as isize as *mut _);
        set_hwnd_rect(hwnd, rect);
        unsafe { let _ = BringWindowToTop(hwnd); }
    };
    let placed = layout_across_displays(&app_windows, &displays, max_per_display, g, spread, &set_rect_fn);

    log::info!(
        "app_expose: spread {} windows of '{}' across {} displays",
        placed,
        target_app,
        displays.len()
    );

    // Calculate how many displays the app's windows fully occupied.
    let displays_consumed = if max_per_display > 0 {
        (placed + max_per_display - 1) / max_per_display
    } else {
        0
    };
    let displays_consumed = displays_consumed.min(displays.len());

    // Only use displays NOT consumed by the target app for other windows.
    let remaining_displays = &displays[displays_consumed..];
    let remaining_cap = max_per_display * remaining_displays.len();

    if remaining_cap > 0 && !remaining_displays.is_empty() {
        let other_windows: Vec<&WindowInfo> = build_sorted_window_list(
            &all_windows,
            remaining_cap,
        )
        .into_iter()
        .filter(|w| w.owner_pid != target_pid as i32)
        .collect();

        if !other_windows.is_empty() {
            let placed_others = layout_across_displays(
                &other_windows, remaining_displays, max_per_display, g, spread, &set_rect_fn,
            );
            log::info!(
                "app_expose: filled remaining {} displays with {} other windows (skipped first {} used by '{}')",
                remaining_displays.len(), placed_others, displays_consumed, target_app,
            );
        }
    }
}

/// Restore all minimized (iconic) windows before exposé layout.
fn restore_minimized_windows() {
    unsafe {
        unsafe extern "system" fn restore_callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
            if IsIconic(hwnd).as_bool() && IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            TRUE
        }

        let _ = EnumWindows(Some(restore_callback), LPARAM(0));
    }
    // Brief pause to let Windows finish animations
    std::thread::sleep(std::time::Duration::from_millis(300));
}

/// Execute a layout preset by name or index. Enumerates windows, matches by
/// app name, and tiles each matched window according to the preset's rules.
pub fn execute_layout_preset(app: &AppHandle, name_or_index: &str) {
    let (preset, half_ratio, third_ratio, gap) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        let preset = match super::resolve_layout_preset(&prefs.layout_presets, name_or_index) {
            Some(p) => p,
            None => {
                log::warn!("layout_preset: preset '{}' not found", name_or_index);
                return;
            }
        };
        (preset, prefs.tiling.half_ratio, prefs.tiling.third_ratio, prefs.tiling.gap)
    };

    let windows = get_all_windows();
    if windows.is_empty() {
        log::info!("layout_preset: no windows found");
        return;
    }

    let displays = get_display_work_areas();
    if displays.is_empty() {
        log::warn!("layout_preset: no displays found");
        return;
    }

    let matches = super::match_windows_to_rules(&windows, &preset.rules);
    log::info!("layout_preset: '{}' matched {} windows", preset.name, matches.len());

    for (win_idx, layout, disp_idx) in matches {
        let w = &windows[win_idx];
        let display_index = disp_idx
            .unwrap_or_else(|| super::find_display_for_window(&w.bounds, &displays))
            .min(displays.len() - 1);
        let target = super::calculate_target_rect(layout, &displays[display_index], half_ratio, third_ratio, gap);
        unsafe {
            set_hwnd_rect(HWND(w.window_id as isize as *mut _), &target);
        }
    }
}
