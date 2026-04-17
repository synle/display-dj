//! Windows window tiling via Win32 API.
//!
//! Provides window tiling (halves, thirds, quarters, maximize, restore, exposé)
//! using `GetForegroundWindow`, `SetWindowPos`, `EnumDisplayMonitors`, and
//! `EnumWindows`. No special permissions are required on Windows.

use super::{
    build_sorted_window_list, calculate_target_rect, find_display_for_window,
    layout_grid_on_display, Rect, TilingLayout, WindowInfo, WindowState,
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

/// Get the current position and size of a window.
fn get_hwnd_rect(hwnd: HWND) -> Option<Rect> {
    let mut rc = RECT::default();
    unsafe {
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

/// Move and resize a window to the given rect.
fn set_hwnd_rect(hwnd: HWND, rect: &Rect) {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            rect.x as i32,
            rect.y as i32,
            rect.width as i32,
            rect.height as i32,
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
    let (max_per_display, gap) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        (prefs.tiling.expose_max_windows as usize, prefs.tiling.gap)
    };

    // Restore minimized windows first
    restore_minimized_windows();

    let all_windows = get_all_windows();
    if all_windows.is_empty() {
        log::info!("expose: no windows found");
        return;
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
    let mut offset = 0;
    let n = ordered.len();
    for (i, display) in displays.iter().enumerate() {
        if offset >= n {
            break;
        }
        let count = (n - offset).min(max_per_display);
        let slice = &ordered[offset..offset + count];
        layout_grid_on_display(slice, display, g, &|win_info, rect| {
            let hwnd = HWND(win_info.window_id as isize as *mut _);
            set_hwnd_rect(hwnd, rect);
            unsafe { let _ = BringWindowToTop(hwnd); }
        });
        log::info!("expose: placed {} windows on display {}", count, i);
        offset += count;
    }

    log::info!(
        "expose: spread {} windows across {} displays",
        n.min(offset),
        displays.len()
    );
}

/// Execute the app exposé command. Lays out the frontmost app's windows in a grid.
pub fn execute_expose_app(app: &AppHandle) {
    let (max_per_display, gap) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        (prefs.tiling.expose_max_windows as usize, prefs.tiling.gap)
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

    let all_windows = get_all_windows();
    if all_windows.is_empty() {
        return;
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
    let n = app_windows.len();
    let mut offset = 0;
    for (i, display) in displays.iter().enumerate() {
        if offset >= n {
            break;
        }
        let count = (n - offset).min(max_per_display);
        let slice = &app_windows[offset..offset + count];
        layout_grid_on_display(slice, display, g, &|win_info, rect| {
            let hwnd = HWND(win_info.window_id as isize as *mut _);
            set_hwnd_rect(hwnd, rect);
            unsafe { let _ = BringWindowToTop(hwnd); }
        });
        log::info!("app_expose: placed {} windows on display {}", count, i);
        offset += count;
    }

    log::info!(
        "app_expose: spread {} windows of '{}' across {} displays",
        n.min(offset),
        target_app,
        displays.len()
    );
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
