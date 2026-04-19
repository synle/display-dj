//! Windows window tiling via Win32 API.
//!
//! Provides window tiling (halves, thirds, quarters, maximize, restore, exposé)
//! using `GetForegroundWindow`, `SetWindowPos`, `EnumDisplayMonitors`, and
//! `EnumWindows`. No special permissions are required on Windows.

use super::{
    build_sorted_window_list, calculate_target_rect, find_display_for_window,
    layout_across_displays, layout_grid_on_display, plan_expose, plan_expose_app,
    plan_layout_preset, Rect, TilingLayout, WindowInfo, WindowState,
};
use tauri::{AppHandle, Manager};

/// Write a message to the debug log file (visible in production builds).
/// `log::info!` only goes to stdout which is invisible in Windows GUI apps.
fn dbg_log(app: &AppHandle, msg: &str) {
    if let Some(state) = app.try_state::<crate::AppState>() {
        crate::config::write_debug_log(&state, msg);
    }
}
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, SetWindowPos, ShowWindow, HWND_TOP,
    SWP_NOZORDER, SW_RESTORE,
};

// ---------------------------------------------------------------------------
// Display enumeration
// ---------------------------------------------------------------------------

/// Per-monitor info collected during enumeration (work area + full bounds).
struct MonitorDebugInfo {
    work: Rect,
    full: Rect,
}

/// Get work areas (visible frames) for all monitors, sorted left-to-right.
/// Uses `EnumDisplayMonitors` + `GetMonitorInfoW` to get the work area
/// (excludes taskbar and other app bars).
fn get_display_work_areas() -> Vec<Rect> {
    let mut infos: Vec<MonitorDebugInfo> = Vec::new();

    unsafe {
        // Callback collects MONITORINFO for each monitor (work area + full bounds)
        unsafe extern "system" fn monitor_callback(
            hmonitor: HMONITOR,
            _hdc: HDC,
            _rect: *mut RECT,
            lparam: LPARAM,
        ) -> BOOL {
            let infos = &mut *(lparam.0 as *mut Vec<MonitorDebugInfo>);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
                let rc = info.rcWork;
                let fm = info.rcMonitor;
                infos.push(MonitorDebugInfo {
                    work: Rect {
                        x: rc.left as f64,
                        y: rc.top as f64,
                        width: (rc.right - rc.left) as f64,
                        height: (rc.bottom - rc.top) as f64,
                    },
                    full: Rect {
                        x: fm.left as f64,
                        y: fm.top as f64,
                        width: (fm.right - fm.left) as f64,
                        height: (fm.bottom - fm.top) as f64,
                    },
                });
            }
            TRUE
        }

        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_callback),
            LPARAM(&mut infos as *mut Vec<MonitorDebugInfo> as isize),
        );
    }

    // Sort left-to-right, then top-to-bottom
    infos.sort_by(|a, b| {
        a.work.x.partial_cmp(&b.work.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.work.y.partial_cmp(&b.work.y).unwrap_or(std::cmp::Ordering::Equal))
    });

    // Log full vs work area for each display (helps debug taskbar/gap issues)
    for (i, info) in infos.iter().enumerate() {
        log::info!(
            "tiling_win: display[{}] — full=({},{} {}x{}), work_area=({},{} {}x{}), \
             taskbar_insets=(left={}, top={}, right={}, bottom={})",
            i,
            info.full.x as i32, info.full.y as i32, info.full.width as i32, info.full.height as i32,
            info.work.x as i32, info.work.y as i32, info.work.width as i32, info.work.height as i32,
            (info.work.x - info.full.x) as i32,
            (info.work.y - info.full.y) as i32,
            ((info.full.x + info.full.width) - (info.work.x + info.work.width)) as i32,
            ((info.full.y + info.full.height) - (info.work.y + info.work.height)) as i32,
        );
    }

    infos.into_iter().map(|i| i.work).collect()
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
    let swp_x = rect.x as i32 - bl;
    let swp_y = rect.y as i32 - bt;
    let swp_w = rect.width as i32 + bl + br;
    let swp_h = rect.height as i32 + bt + bb;
    log::info!(
        "tiling_win: set_hwnd_rect — target=({},{} {}x{}), dwm_border=({},{},{},{}), \
         swp_args=({},{} {}x{})",
        rect.x as i32, rect.y as i32, rect.width as i32, rect.height as i32,
        bl, bt, br, bb,
        swp_x, swp_y, swp_w, swp_h,
    );
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            swp_x,
            swp_y,
            swp_w,
            swp_h,
            SWP_NOZORDER,
        );
    }
    // Log the actual result after SetWindowPos
    if let Some(actual) = get_hwnd_rect(hwnd) {
        log::info!(
            "tiling_win: after SetWindowPos — actual_visible=({},{} {}x{}), \
             delta=(dx={}, dy={}, dw={}, dh={})",
            actual.x as i32, actual.y as i32, actual.width as i32, actual.height as i32,
            actual.x as i32 - rect.x as i32,
            actual.y as i32 - rect.y as i32,
            actual.width as i32 - rect.width as i32,
            actual.height as i32 - rect.height as i32,
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

use super::should_skip_system_window;

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

            // Get window title as String for filtering
            let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

            // Skip system/desktop windows that should not be tiled or exposed
            if should_skip_system_window(&owner_name, &title) {
                return TRUE;
            }

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

    // Log display info and window details for debugging (e.g. gap issues)
    let title = get_window_title(hwnd);
    let process = get_process_name(hwnd);
    let (bl, bt, br, bb) = get_dwm_border(hwnd);
    let display_info: Vec<String> = displays.iter().enumerate().map(|(i, d)| {
        format!("D{}({},{} {}x{})", i, d.x as i32, d.y as i32, d.width as i32, d.height as i32)
    }).collect();
    dbg_log(app, &format!(
        "tiling_win: tile '{}' — layout={}, displays=[{}], window='{}' ({}), \
         visible_rect=({},{} {}x{}), dwm_border=({},{},{},{}), prefs=(half={}, third={}, gap={})",
        layout_str, layout_str, display_info.join(", "), title, process,
        win_rect.x as i32, win_rect.y as i32, win_rect.width as i32, win_rect.height as i32,
        bl, bt, br, bb, half_ratio, third_ratio, gap,
    ));

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
    dbg_log(app, &format!(
        "tiling_win: target_rect — display={}, layout={}, rect=({},{} {}x{})",
        target_display, layout_str,
        target.x as i32, target.y as i32, target.width as i32, target.height as i32,
    ));
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

/// Guard to prevent concurrent expose runs. If expose is already in progress,
/// subsequent calls are ignored until the first one finishes.
static EXPOSE_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Execute the exposé command. Lays out all on-screen windows in a grid.
/// Exposé: spread all windows across displays using shared plan_expose logic.
/// Debounced: ignores calls if expose is already running.
pub fn execute_expose(app: &AppHandle) {
    if EXPOSE_RUNNING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        dbg_log(app, "tiling_win: expose — skipped (already running)");
        return;
    }
    // Ensure the flag is cleared when we exit, even on early returns
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            EXPOSE_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _guard = Guard;

    let (max_per_display, gap, spread) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        ((prefs.tiling.expose_columns * prefs.tiling.expose_rows) as usize, prefs.tiling.gap, prefs.tiling.expose_layout_strategy == "spread")
    };

    restore_minimized_windows();

    let all_windows = get_all_windows();
    if all_windows.is_empty() {
        log::info!("expose: no windows found");
        return;
    }

    // NOTE: We intentionally do NOT query min_size here. On mixed-DPI setups,
    // min_size is DPI-dependent and changes when windows move between monitors.
    // This caused "oversized" misclassification (e.g., Brave at 516x89 on 1x
    // became 1282x219 on 2.5x). Windows will enforce their own min_size when
    // SetWindowPos is called — they just won't shrink below it.

    let displays = get_display_work_areas();
    if displays.is_empty() {
        return;
    }

    // Log display work areas for debugging gap issues
    let display_info: Vec<String> = displays.iter().enumerate().map(|(i, d)| {
        format!("D{}({},{} {}x{})", i, d.x as i32, d.y as i32, d.width as i32, d.height as i32)
    }).collect();
    let total_cap = max_per_display * displays.len();
    let cols = (max_per_display as f64).sqrt().ceil() as usize;
    let rows = if cols > 0 { (max_per_display + cols - 1) / cols } else { 0 };
    dbg_log(app, &format!(
        "tiling_win: expose — {} windows (cap={}), {} displays=[{}], grid={}x{} (max_per_display={}), gap={}, spread={}",
        all_windows.len(), total_cap, displays.len(), display_info.join(", "),
        cols, rows, max_per_display, gap, spread,
    ));
    for (i, w) in all_windows.iter().enumerate() {
        let min_str = w.min_size.map_or("none".to_string(), |(mw, mh)| format!("{}x{}", mw as i32, mh as i32));
        dbg_log(app, &format!(
            "tiling_win: expose_window[{}] — '{}' (pid={}), bounds=({},{} {}x{}), min_size={}",
            i, w.owner_name, w.owner_pid,
            w.bounds.x as i32, w.bounds.y as i32, w.bounds.width as i32, w.bounds.height as i32,
            min_str,
        ));
    }

    let placements = plan_expose(&all_windows, &displays, max_per_display, gap as f64, spread);
    for (i, p) in placements.iter().enumerate() {
        let hwnd = HWND(p.window_id as isize as *mut _);
        let title = get_window_title(hwnd);
        let process = get_process_name(hwnd);
        // Determine which display and grid position
        let display_idx = displays.iter().position(|d| {
            p.target.x >= d.x && p.target.x < d.x + d.width
        }).unwrap_or(0);
        dbg_log(app, &format!(
            "tiling_win: expose_place[{}] — '{}' ({}), display={}, target=({},{} {}x{})",
            i, title, process, display_idx,
            p.target.x as i32, p.target.y as i32, p.target.width as i32, p.target.height as i32,
        ));
        set_hwnd_rect(hwnd, &p.target);
        unsafe { let _ = BringWindowToTop(hwnd); }
    }
    let placed = placements.len();
    let capped = all_windows.len().saturating_sub(placed);
    dbg_log(app, &format!(
        "tiling_win: expose_done — placed={}, capped={} (total_windows={}), displays={}",
        placed, capped, all_windows.len(), displays.len(),
    ));
}

/// App Exposé: target app's windows on first displays, others on remaining.
/// Uses shared plan_expose_app logic. Debounced: shares the same guard as expose.
pub fn execute_expose_app(app: &AppHandle) {
    if EXPOSE_RUNNING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        dbg_log(app, "tiling_win: app_expose — skipped (already running)");
        return;
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            EXPOSE_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _guard = Guard;
    let (max_per_display, gap, spread) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        ((prefs.tiling.expose_columns * prefs.tiling.expose_rows) as usize, prefs.tiling.gap, prefs.tiling.expose_layout_strategy == "spread")
    };

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

    restore_minimized_windows();

    let mut all_windows = get_all_windows();
    if all_windows.is_empty() {
        return;
    }

    // Skip min_size query — see comment in execute_expose for rationale.

    let displays = get_display_work_areas();
    if displays.is_empty() {
        return;
    }

    // Log display work areas for debugging gap issues
    let display_info: Vec<String> = displays.iter().enumerate().map(|(i, d)| {
        format!("D{}({},{} {}x{})", i, d.x as i32, d.y as i32, d.width as i32, d.height as i32)
    }).collect();
    let app_window_count = all_windows.iter().filter(|w| w.owner_pid == target_pid as i32).count();
    let other_window_count = all_windows.len() - app_window_count;
    let total_cap = max_per_display * displays.len();
    dbg_log(app, &format!(
        "tiling_win: app_expose — app='{}' (pid={}), app_windows={}, other_windows={}, cap={}, \
         {} displays=[{}], max_per_display={}, gap={}, spread={}",
        target_app, target_pid, app_window_count, other_window_count, total_cap,
        displays.len(), display_info.join(", "), max_per_display, gap, spread,
    ));
    for (i, w) in all_windows.iter().enumerate() {
        let is_target = if w.owner_pid == target_pid as i32 { "TARGET" } else { "other" };
        dbg_log(app, &format!(
            "tiling_win: app_expose_window[{}] — [{}] '{}' (pid={}), bounds=({},{} {}x{})",
            i, is_target, w.owner_name, w.owner_pid,
            w.bounds.x as i32, w.bounds.y as i32, w.bounds.width as i32, w.bounds.height as i32,
        ));
    }

    // How many displays the app's windows will consume
    let app_displays_needed = if max_per_display > 0 {
        (app_window_count + max_per_display - 1) / max_per_display
    } else { 0 };
    let app_displays_used = app_displays_needed.min(displays.len());
    let displays_for_others = displays.len() - app_displays_used;
    let app_slots_total = app_displays_used * max_per_display;
    let app_slots_unused = app_slots_total.saturating_sub(app_window_count);
    let other_slots_total = displays_for_others * max_per_display;
    dbg_log(app, &format!(
        "tiling_win: app_expose_plan — app_windows={} → uses {} display(s) ({} slots, {} unused), \
         other_windows={} → {} display(s) remaining ({} slots)",
        app_window_count, app_displays_used, app_slots_total, app_slots_unused,
        other_window_count, displays_for_others, other_slots_total,
    ));

    let placements = plan_expose_app(&all_windows, target_pid as i32, &displays, max_per_display, gap as f64, spread);
    let mut app_placed = 0;
    let mut other_placed = 0;
    for (i, p) in placements.iter().enumerate() {
        let hwnd = HWND(p.window_id as isize as *mut _);
        let title = get_window_title(hwnd);
        let process = get_process_name(hwnd);
        let is_app = p.owner_pid == target_pid as i32;
        let tag = if is_app { "APP" } else { "other" };
        if is_app { app_placed += 1; } else { other_placed += 1; }
        let display_idx = displays.iter().position(|d| {
            p.target.x >= d.x && p.target.x < d.x + d.width
        }).unwrap_or(0);
        dbg_log(app, &format!(
            "tiling_win: app_expose_place[{}] — [{}] '{}' ({}), display={}, target=({},{} {}x{})",
            i, tag, title, process, display_idx,
            p.target.x as i32, p.target.y as i32, p.target.width as i32, p.target.height as i32,
        ));
        set_hwnd_rect(hwnd, &p.target);
        unsafe { let _ = BringWindowToTop(hwnd); }
    }
    dbg_log(app, &format!(
        "tiling_win: app_expose_done — placed={} (app={}, other={}), \
         displays={} (app_used={}, other_used={})",
        placements.len(), app_placed, other_placed,
        displays.len(), app_displays_used, displays_for_others,
    ));
}

/// Restore all minimized and maximized windows before exposé layout.
/// Minimized windows need to be restored so they appear in the grid.
/// Maximized windows need to be restored because SetWindowPos behaves
/// differently for maximized windows and their bounds span the full work area.
fn restore_minimized_windows() {
    unsafe {
        unsafe extern "system" fn restore_callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
            if (IsIconic(hwnd).as_bool() || IsZoomed(hwnd).as_bool()) && IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            TRUE
        }

        let _ = EnumWindows(Some(restore_callback), LPARAM(0));
    }
    // Pause to let Windows finish restore/un-maximize animations.
    // 1 second is needed because un-maximize animations take longer than
    // un-minimize. Without this, window bounds are read mid-animation
    // and the first expose run has incorrect layout.
    std::thread::sleep(std::time::Duration::from_millis(1000));
}

/// Execute a layout preset by name or index. Enumerates windows, matches by
/// app name, and tiles each matched window according to the preset's rules.
/// Apply a layout preset using shared plan_layout_preset logic.
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

    let placements = plan_layout_preset(&windows, &preset, &displays, half_ratio, third_ratio, gap);
    log::info!("layout_preset: '{}' placing {} windows", preset.name, placements.len());
    for p in &placements {
        unsafe {
            set_hwnd_rect(HWND(p.window_id as isize as *mut _), &p.target);
        }
    }
}

