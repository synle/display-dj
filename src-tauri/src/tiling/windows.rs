//! Windows window tiling via Win32 API.
//!
//! Provides window tiling (halves, thirds, quarters, maximize, restore, exposé)
//! using `GetForegroundWindow`, `SetWindowPos`, `EnumDisplayMonitors`, and
//! `EnumWindows`. Normal windows need no special permissions; controlling an
//! elevated target requires launching Display DJ as administrator.

use super::snap_overlay::TileSnapOverlay;
use super::{
    build_snap_zones, build_sorted_window_list, calculate_target_rect,
    detect_snap_zone_with_toggles, find_display_for_window, layout_across_displays,
    layout_grid_on_display, plan_expose, plan_expose_app, plan_layout_preset, Rect,
    SnapZoneToggles, TilingLayout, WindowInfo, WindowState,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

/// Write a message to the debug log file (visible in production builds).
/// `log::info!` only goes to stdout which is invisible in Windows GUI apps.
fn dbg_log(app: &AppHandle, msg: &str) {
    if let Some(state) = app.try_state::<crate::AppState>() {
        crate::config::write_debug_log(&state, msg);
    }
}

/// Return whether the current Windows process runs with an elevated token.
pub(super) fn is_process_elevated() -> Result<bool, String> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = HANDLE::default();
    unsafe {
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|error| format!("OpenProcessToken failed: {error}"))?;
    }

    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned_length = 0;
    let query_result = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned_length,
        )
    };
    unsafe {
        let _ = CloseHandle(token);
    }
    query_result.map_err(|error| format!("GetTokenInformation(TokenElevation) failed: {error}"))?;

    let expected_length = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
    if returned_length < expected_length {
        return Err(format!(
            "GetTokenInformation(TokenElevation) returned {returned_length} bytes; expected {expected_length}"
        ));
    }

    Ok(elevation.TokenIsElevated != 0)
}

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, DispatchMessageW, EnumWindows, GetCursorPos, GetForegroundWindow,
    GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
    IsZoomed, PeekMessageW, SendMessageTimeoutW, SetForegroundWindow, SetWindowPos, ShowWindow,
    TranslateMessage, EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZESTART, HTBOTTOM, HTBOTTOMLEFT,
    HTBOTTOMRIGHT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, HWND_BOTTOM, HWND_TOP, MSG,
    OBJID_WINDOW, PM_REMOVE, SMTO_ABORTIFHUNG, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SW_RESTORE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_NCHITTEST,
};

// ---------------------------------------------------------------------------
// Display enumeration
// ---------------------------------------------------------------------------

/// Per-monitor info collected during enumeration (work area + full bounds + DPI).
struct MonitorDebugInfo {
    work: Rect,
    full: Rect,
    dpi_scale: f64,
}

/// Get work areas (visible frames) and DPI scale for all monitors, sorted
/// left-to-right. Uses `EnumDisplayMonitors` + `GetMonitorInfoW` + `GetDpiForMonitor`.
fn get_display_work_areas() -> Vec<(Rect, f64)> {
    let mut infos: Vec<MonitorDebugInfo> = Vec::new();

    unsafe {
        // Callback collects MONITORINFO + DPI for each monitor
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

                // Query effective DPI for this monitor (96 = 1x, 192 = 2x, 240 = 2.5x)
                use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
                let mut dpi_x: u32 = 96;
                let mut dpi_y: u32 = 96;
                let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
                let dpi_scale = dpi_x as f64 / 96.0;

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
                    dpi_scale,
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
        a.work
            .x
            .partial_cmp(&b.work.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.work
                    .y
                    .partial_cmp(&b.work.y)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    // Log full vs work area + DPI for each display (helps debug gap issues)
    for (i, info) in infos.iter().enumerate() {
        log::info!(
            "tiling_win: display[{}] — full=({},{} {}x{}), work_area=({},{} {}x{}), \
             dpi_scale={:.2}, taskbar_insets=(left={}, top={}, right={}, bottom={})",
            i,
            info.full.x as i32,
            info.full.y as i32,
            info.full.width as i32,
            info.full.height as i32,
            info.work.x as i32,
            info.work.y as i32,
            info.work.width as i32,
            info.work.height as i32,
            info.dpi_scale,
            (info.work.x - info.full.x) as i32,
            (info.work.y - info.full.y) as i32,
            ((info.full.x + info.full.width) - (info.work.x + info.work.width)) as i32,
            ((info.full.y + info.full.height) - (info.work.y + info.work.height)) as i32,
        );
    }

    infos.into_iter().map(|i| (i.work, i.dpi_scale)).collect()
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

/// Return whether ordinary tiling should restore this window before resizing it.
fn should_restore_before_tiling(is_minimized: bool, is_maximized: bool) -> bool {
    !is_minimized && is_maximized
}

/// Restore a maximized window to its normal placement before move/resize.
/// Minimized windows stay minimized; Exposé owns the separate unminimize flow.
fn restore_maximized_window(hwnd: HWND) -> bool {
    let should_restore =
        unsafe { should_restore_before_tiling(IsIconic(hwnd).as_bool(), IsZoomed(hwnd).as_bool()) };
    if !should_restore {
        return false;
    }

    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
    }
    true
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
        frame_rect.left - window_rect.left,     // left border
        frame_rect.top - window_rect.top,       // top border
        window_rect.right - frame_rect.right,   // right border
        window_rect.bottom - frame_rect.bottom, // bottom border
    )
}

/// Move and resize a window to the given rect.
/// Over-expands by DWM border amount so visible frames fill cells edge-to-edge.
/// Uses post-move correction for mixed-DPI setups: after the first SetWindowPos,
/// the window may land on a different-DPI display where DWM borders differ.
/// A second corrective SetWindowPos aligns the visible frame to the target rect.
fn set_hwnd_rect(hwnd: HWND, rect: &Rect) {
    restore_maximized_window(hwnd);

    let (bl, _bt, br, bb) = get_dwm_border(hwnd);
    // Expand outward by the border amount on each side.
    // Top border (bt) is typically 0 on Windows 10/11 (title bar has no invisible border).
    let mut swp_x = rect.x as i32 - bl;
    let mut swp_y = rect.y as i32; // no top expansion (bt is 0)
    let mut swp_w = rect.width as i32 + bl + br;
    let mut swp_h = rect.height as i32 + bb; // expand bottom only
    unsafe {
        let _ = SetWindowPos(hwnd, HWND_TOP, swp_x, swp_y, swp_w, swp_h, SWP_NOZORDER);
    }

    // Post-move correction for mixed-DPI setups.
    // After the first SetWindowPos, the window is on the target display and DWM
    // renders it with that display's DPI-scaled borders. Read back the actual
    // visible frame and apply a second corrective SetWindowPos if it doesn't
    // match the target rect. This fixes gaps on high-DPI monitors where borders
    // are larger (~18px at 2.5x) than the pre-move borders (~7px at 1x).
    if let Some(actual) = get_hwnd_rect(hwnd) {
        let dx = actual.x as i32 - rect.x as i32;
        let dy = actual.y as i32 - rect.y as i32;
        let dw = actual.width as i32 - rect.width as i32;
        let dh = actual.height as i32 - rect.height as i32;

        if dx != 0 || dy != 0 || dw != 0 || dh != 0 {
            log::info!(
                "tiling_win: post-move correction — delta=(dx={}, dy={}, dw={}, dh={})",
                dx,
                dy,
                dw,
                dh,
            );
            swp_x -= dx;
            swp_y -= dy;
            swp_w -= dw;
            swp_h -= dh;
            unsafe {
                let _ = SetWindowPos(hwnd, HWND_TOP, swp_x, swp_y, swp_w, swp_h, SWP_NOZORDER);
            }
        }
    }

    // Log final result for debugging
    if let Some(final_rect) = get_hwnd_rect(hwnd) {
        log::info!(
            "tiling_win: after SetWindowPos — actual_visible=({},{} {}x{}), \
             delta=(dx={}, dy={}, dw={}, dh={})",
            final_rect.x as i32,
            final_rect.y as i32,
            final_rect.width as i32,
            final_rect.height as i32,
            final_rect.x as i32 - rect.x as i32,
            final_rect.y as i32 - rect.y as i32,
            final_rect.width as i32 - rect.width as i32,
            final_rect.height as i32 - rect.height as i32,
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
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
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
                    ::windows::core::PWSTR(buf.as_mut_ptr()),
                    &mut size,
                )
                .is_ok()
                {
                    let path = String::from_utf16_lossy(&buf[..size as usize]);
                    let _ = ::windows::Win32::Foundation::CloseHandle(h);
                    // Return just the exe name without extension
                    path.rsplit('\\')
                        .next()
                        .unwrap_or(&path)
                        .strip_suffix(".exe")
                        .unwrap_or_else(|| path.rsplit('\\').next().unwrap_or(&path))
                        .to_string()
                } else {
                    let _ = ::windows::Win32::Foundation::CloseHandle(h);
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
            ::windows::Win32::Foundation::WPARAM(0),
            ::windows::Win32::Foundation::LPARAM(&mut info as *mut MINMAXINFO as isize),
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
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
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
                    ::windows::core::PWSTR(buf.as_mut_ptr()),
                    &mut size,
                )
                .is_ok()
                {
                    let path = String::from_utf16_lossy(&buf[..size as usize]);
                    let _ = ::windows::Win32::Foundation::CloseHandle(h);
                    path.rsplit('\\')
                        .next()
                        .unwrap_or(&path)
                        .strip_suffix(".exe")
                        .unwrap_or_else(|| path.rsplit('\\').next().unwrap_or(&path))
                        .to_string()
                } else {
                    let _ = ::windows::Win32::Foundation::CloseHandle(h);
                    String::new()
                }
            }
            Err(_) => String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Z-order helpers
// ---------------------------------------------------------------------------

/// Collect all top-level visible HWNDs that belong to a given PID, in the
/// order returned by `EnumWindows` (front-to-back z-order).
fn collect_hwnds_for_pid(target_pid: u32) -> Vec<HWND> {
    struct Ctx {
        pid: u32,
        out: Vec<HWND>,
    }
    let mut ctx = Ctx {
        pid: target_pid,
        out: Vec::new(),
    };
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == ctx.pid {
            ctx.out.push(hwnd);
        }
        TRUE
    }
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut Ctx as isize));
    }
    ctx.out
}

/// Bring the focused (foreground) window to the top of the z-order
/// and give it focus.
pub fn move_window_to_front(_app: &AppHandle) {
    let hwnd = match get_foreground_hwnd() {
        Some(h) => h,
        None => {
            log::info!("move_window_to_front: no foreground window");
            return;
        }
    };
    unsafe {
        // Raise in z-order without activating (avoids double-activation flicker),
        // then ensure focus via SetForegroundWindow + BringWindowToTop.
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
    }
}

/// Bring all top-level windows of the focused app's PID to the top of
/// the z-order. The originally focused window stays focused / topmost.
pub fn move_app_to_front(_app: &AppHandle) {
    let hwnd = match get_foreground_hwnd() {
        Some(h) => h,
        None => {
            log::info!("move_app_to_front: no foreground window");
            return;
        }
    };
    let mut target_pid: u32 = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut target_pid));
    }
    if target_pid == 0 {
        log::info!("move_app_to_front: could not resolve PID for foreground window");
        return;
    }
    let hwnds = collect_hwnds_for_pid(target_pid);
    unsafe {
        // Raise each app window to HWND_TOP without activating. EnumWindows
        // returns front-to-back z-order, so iterating in reverse means the
        // already-topmost window ends up topmost again.
        for h in hwnds.iter().rev() {
            let _ = SetWindowPos(
                *h,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        // Make sure the originally focused window remains the foreground
        // window after the raise loop.
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
    }
}

/// Send the focused window to the bottom of the z-order via
/// `SetWindowPos(HWND_BOTTOM)`. Doesn't change focus explicitly — Windows
/// transfers focus to the next-frontmost window automatically.
pub fn move_window_to_back(_app: &AppHandle) {
    let hwnd = match get_foreground_hwnd() {
        Some(h) => h,
        None => {
            log::info!("move_window_to_back: no foreground window");
            return;
        }
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Send all top-level windows of the focused app's PID to the bottom of
/// the z-order.
///
/// `EnumWindows` returns windows in front-to-back z-order. Iterating
/// forward and calling `SetWindowPos(HWND_BOTTOM)` on each one in turn
/// preserves the relative within-app order: each call drops one window
/// to the absolute bottom, so the originally frontmost-of-app ends up
/// topmost-among-the-lowered-set.
pub fn move_app_to_back(_app: &AppHandle) {
    let hwnd = match get_foreground_hwnd() {
        Some(h) => h,
        None => {
            log::info!("move_app_to_back: no foreground window");
            return;
        }
    };
    let mut target_pid: u32 = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut target_pid));
    }
    if target_pid == 0 {
        log::info!("move_app_to_back: could not resolve PID for foreground window");
        return;
    }
    let hwnds = collect_hwnds_for_pid(target_pid);
    unsafe {
        for h in hwnds.iter() {
            let _ = SetWindowPos(
                *h,
                HWND_BOTTOM,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

/// Check if the focused window is the global topmost. On Windows the
/// foreground window is generally the topmost, so this almost always
/// returns true unless the user's app is covered by a topmost (always-on-top)
/// system overlay or the focus is on a non-foreground window. Used by the
/// toggle dispatch.
///
/// `pub(super)` so the shared z-order self-test in `tiling/mod.rs` can read
/// live front/back state when `DISPLAY_DJ_ZORDER_SELFTEST=1`.
pub(super) fn is_focused_window_at_front() -> bool {
    let foreground = match get_foreground_hwnd() {
        Some(h) => h,
        None => return false,
    };
    let foreground_id = foreground.0 as isize as i64;
    // Build a front-to-back list of visible top-level HWNDs (filtering
    // skip-list system windows like Program Manager / TextInputHost).
    let mut z_order: Vec<i64> = Vec::new();
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let z = &mut *(lparam.0 as *mut Vec<i64>);
        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }
        let process = get_process_name(hwnd);
        let title = get_window_title(hwnd);
        if super::should_skip_system_window(&process, &title) {
            return TRUE;
        }
        z.push(hwnd.0 as isize as i64);
        TRUE
    }
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut z_order as *mut Vec<i64> as isize));
    }
    super::is_window_at_front(foreground_id, &z_order)
}

/// Toggle the focused window's z-order: front if it isn't, back if it is.
pub fn toggle_window_front_back(app: &AppHandle) {
    if is_focused_window_at_front() {
        move_window_to_back(app);
    } else {
        move_window_to_front(app);
    }
}

/// Toggle the focused app's z-order: front if it isn't, back if it is.
pub fn toggle_app_front_back(app: &AppHandle) {
    if is_focused_window_at_front() {
        move_app_to_back(app);
    } else {
        move_app_to_front(app);
    }
}

// ---------------------------------------------------------------------------
// Tile Snap -- WinEvent move tracking with Tauri preview overlays
// ---------------------------------------------------------------------------

const TILE_SNAP_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);
const TILE_SNAP_IDLE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

static TILE_SNAP_STARTED: AtomicBool = AtomicBool::new(false);
static WIN_EVENT_SENDER: std::sync::OnceLock<std::sync::mpsc::Sender<WindowMoveEvent>> =
    std::sync::OnceLock::new();

/// Move/size lifecycle event copied out of the Win32 callback.
#[derive(Clone, Copy, Debug)]
enum WindowMoveEvent {
    Start(isize),
    End(isize),
}

/// State captured for one system window-move gesture.
struct TileSnapDrag {
    hwnd: HWND,
    original_rect: Rect,
    displays: Vec<Rect>,
    half_ratio: u32,
    third_ratio: u32,
    gap: u32,
    side_edge_trigger: f64,
    top_edge_trigger: f64,
    corner_trigger: f64,
    toggles: SnapZoneToggles,
    current_layout: Option<TilingLayout>,
    current_display: usize,
    current_target: Option<Rect>,
}

/// WinEvent callback for system move/size start and end events.
///
/// The callback crosses an OS FFI boundary, so it catches panics and performs
/// only a channel send. All Win32 queries and Tauri work happen on the monitor
/// thread.
unsafe extern "system" fn tile_snap_win_event_callback(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object_id: i32,
    child_id: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    let _ = std::panic::catch_unwind(|| {
        if object_id != OBJID_WINDOW.0 || child_id != 0 || hwnd.is_invalid() {
            return;
        }
        let window_id = hwnd.0 as isize;
        let move_event = match event {
            EVENT_SYSTEM_MOVESIZESTART => WindowMoveEvent::Start(window_id),
            EVENT_SYSTEM_MOVESIZEEND => WindowMoveEvent::End(window_id),
            _ => return,
        };
        if let Some(sender) = WIN_EVENT_SENDER.get() {
            let _ = sender.send(move_event);
        }
    });
}

/// Start the Windows Tile Snap monitor once for the process lifetime.
///
/// The monitor remains dormant while the preference is disabled, which lets a
/// user enable Tile Snap in Settings without restarting the application.
pub fn start_tile_snap(app: AppHandle) {
    if TILE_SNAP_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        log::info!("tile_snap_win: already started");
        return;
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    if WIN_EVENT_SENDER.set(sender).is_err() {
        log::warn!("tile_snap_win: event sender already initialized");
        return;
    }

    let thread_app = app.clone();
    match std::thread::Builder::new()
        .name("display-dj-tile-snap-win".into())
        .spawn(move || run_tile_snap_monitor(thread_app, receiver))
    {
        Ok(_) => dbg_log(&app, "tile_snap_win: WinEvent monitor thread started"),
        Err(error) => dbg_log(
            &app,
            &format!("tile_snap_win: failed to start monitor thread: {error}"),
        ),
    }
}

/// Run the WinEvent message pump and poll active drags for cursor movement.
fn run_tile_snap_monitor(app: AppHandle, receiver: std::sync::mpsc::Receiver<WindowMoveEvent>) {
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_MOVESIZESTART,
            EVENT_SYSTEM_MOVESIZEEND,
            None,
            Some(tile_snap_win_event_callback),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    if hook.0.is_null() {
        dbg_log(&app, "tile_snap_win: SetWinEventHook failed");
        return;
    }

    let mut overlay = TileSnapOverlay::new(app.clone());
    let mut drag: Option<TileSnapDrag> = None;
    dbg_log(&app, "tile_snap_win: SetWinEventHook registered");

    loop {
        pump_win_event_messages();

        while let Ok(event) = receiver.try_recv() {
            match event {
                WindowMoveEvent::Start(raw_hwnd) => {
                    overlay.hide();
                    drag = begin_tile_snap_drag(&app, &mut overlay, HWND(raw_hwnd as *mut _));
                }
                WindowMoveEvent::End(raw_hwnd) => {
                    let should_finish = drag
                        .as_ref()
                        .map(|active| active.hwnd.0 as isize == raw_hwnd)
                        .unwrap_or(false);
                    if should_finish {
                        if let Some(active) = drag.take() {
                            finish_tile_snap_drag(&app, &overlay, active);
                        }
                    }
                }
            }
        }

        if let Some(active) = drag.as_mut() {
            if !update_tile_snap_drag(&app, &overlay, active) {
                overlay.hide();
                drag = None;
            }
        }

        std::thread::sleep(if drag.is_some() {
            TILE_SNAP_POLL_INTERVAL
        } else {
            TILE_SNAP_IDLE_INTERVAL
        });
    }
}

/// Dispatch pending WinEvent callback messages on the hook-owning thread.
fn pump_win_event_messages() {
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Initialize one move gesture after verifying it is not a resize.
fn begin_tile_snap_drag(
    app: &AppHandle,
    overlay: &mut TileSnapOverlay,
    hwnd: HWND,
) -> Option<TileSnapDrag> {
    if !unsafe { IsWindow(hwnd).as_bool() } || !is_window_move_gesture(hwnd) {
        return None;
    }

    let prefs = {
        let state = app.try_state::<crate::AppState>()?;
        let prefs = state.preferences.lock().ok()?;
        if !prefs.tiling.enabled || !prefs.tiling.tile_snap_enabled {
            return None;
        }
        prefs.tiling.clone()
    };
    if restore_maximized_window(hwnd) {
        dbg_log(
            app,
            "tile_snap_win: restored maximized window before drag tracking",
        );
    }
    let original_rect = get_hwnd_rect(hwnd)?;
    let display_infos = get_display_work_areas();
    let displays: Vec<Rect> = display_infos.iter().map(|(rect, _)| rect.clone()).collect();
    let scale_factors: Vec<f64> = display_infos.iter().map(|(_, scale)| *scale).collect();
    if displays.is_empty() {
        dbg_log(app, "tile_snap_win: no displays found");
        return None;
    }

    let toggles = SnapZoneToggles::from_prefs(&prefs);
    let zones = build_snap_zones(
        &displays,
        prefs.side_edge_trigger as f64,
        prefs.top_edge_trigger as f64,
        prefs.corner_trigger as f64,
        &toggles,
    );
    if let Err(error) = overlay.show_zones(&displays, &zones, &scale_factors) {
        dbg_log(app, &error);
        overlay.hide();
        return None;
    }

    dbg_log(
        app,
        &format!(
            "tile_snap_win: move started hwnd={:?}, displays={}, zones={}",
            hwnd,
            displays.len(),
            zones.len(),
        ),
    );
    Some(TileSnapDrag {
        hwnd,
        original_rect,
        displays,
        half_ratio: prefs.half_ratio,
        third_ratio: prefs.third_ratio,
        gap: prefs.gap,
        side_edge_trigger: prefs.side_edge_trigger as f64,
        top_edge_trigger: prefs.top_edge_trigger as f64,
        corner_trigger: prefs.corner_trigger as f64,
        toggles,
        current_layout: None,
        current_display: 0,
        current_target: None,
    })
}

/// Update preview state from the current cursor position.
///
/// Returns `false` when the drag should be cancelled.
fn update_tile_snap_drag(
    app: &AppHandle,
    overlay: &TileSnapOverlay,
    drag: &mut TileSnapDrag,
) -> bool {
    if !tile_snap_enabled(app) || !unsafe { IsWindow(drag.hwnd).as_bool() } {
        return false;
    }

    let cursor = match get_cursor_position() {
        Some(cursor) => cursor,
        None => return true,
    };
    let zone = detect_snap_zone_with_toggles(
        cursor.x as f64,
        cursor.y as f64,
        &drag.displays,
        drag.side_edge_trigger,
        drag.top_edge_trigger,
        drag.corner_trigger,
        &drag.toggles,
    );

    match zone {
        Some((layout, display_index))
            if drag.current_layout != Some(layout) || drag.current_display != display_index =>
        {
            let display = match drag.displays.get(display_index) {
                Some(display) => display,
                None => return false,
            };
            let target =
                calculate_target_rect(layout, display, drag.half_ratio, drag.third_ratio, drag.gap);
            if let Err(error) = overlay.show_preview(display_index, &target) {
                dbg_log(app, &error);
                return false;
            }
            drag.current_layout = Some(layout);
            drag.current_display = display_index;
            drag.current_target = Some(target);
        }
        Some(_) => {}
        None if drag.current_layout.is_some() => {
            if let Err(error) = overlay.clear_preview() {
                dbg_log(app, &error);
                return false;
            }
            drag.current_layout = None;
            drag.current_target = None;
        }
        None => {}
    }
    true
}

/// Hide overlays and apply the exact rectangle shown in the preview.
fn finish_tile_snap_drag(app: &AppHandle, overlay: &TileSnapOverlay, drag: TileSnapDrag) {
    overlay.hide();
    let (layout, target) = match (drag.current_layout, drag.current_target) {
        (Some(layout), Some(target)) => (layout, target),
        _ => return,
    };
    if !unsafe { IsWindow(drag.hwnd).as_bool() } {
        return;
    }

    let window_key = drag.hwnd.0 as isize as i64;
    if let Some(state) = app.try_state::<crate::AppState>() {
        if let Ok(mut tiling_state) = state.tiling_state.lock() {
            let entry = tiling_state
                .windows
                .entry(window_key)
                .or_insert(WindowState {
                    original: drag.original_rect,
                    layout,
                    display_index: drag.current_display,
                });
            entry.layout = layout;
            entry.display_index = drag.current_display;
        }
    }

    dbg_log(
        app,
        &format!(
            "tile_snap_win: drop hwnd={:?}, layout={:?}, display={}, target=({:.0},{:.0} {:.0}x{:.0})",
            drag.hwnd,
            layout,
            drag.current_display,
            target.x,
            target.y,
            target.width,
            target.height,
        ),
    );
    set_hwnd_rect(drag.hwnd, &target);
}

/// Return whether Tile Snap remains enabled while a drag is active.
fn tile_snap_enabled(app: &AppHandle) -> bool {
    app.try_state::<crate::AppState>()
        .and_then(|state| {
            state
                .preferences
                .try_lock()
                .ok()
                .map(|prefs| prefs.tiling.enabled && prefs.tiling.tile_snap_enabled)
        })
        .unwrap_or(false)
}

/// Read the global cursor position in physical screen pixels.
fn get_cursor_position() -> Option<POINT> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok().map(|_| point) }
}

/// Distinguish title-bar moves from border resize gestures.
///
/// `EVENT_SYSTEM_MOVESIZESTART` covers both operations. `WM_NCHITTEST` gives
/// the non-client region that initiated the modal loop; explicit resize-border
/// hits are rejected, while custom title bars that report `HTCLIENT` remain
/// eligible.
fn is_window_move_gesture(hwnd: HWND) -> bool {
    if unsafe { IsZoomed(hwnd).as_bool() } {
        return true;
    }
    let cursor = match get_cursor_position() {
        Some(cursor) => cursor,
        None => return false,
    };
    let mut hit_test = 0usize;
    let status = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_NCHITTEST,
            WPARAM(0),
            point_to_lparam(cursor),
            SMTO_ABORTIFHUNG,
            50,
            Some(&mut hit_test),
        )
    };
    status.0 != 0 && !is_resize_hit_test(hit_test as u32)
}

/// Pack signed screen coordinates for `WM_NCHITTEST`.
fn point_to_lparam(point: POINT) -> LPARAM {
    let x = point.x as i16 as u16 as u32;
    let y = point.y as i16 as u16 as u32;
    LPARAM(((y << 16) | x) as isize)
}

/// Return whether a `WM_NCHITTEST` result represents a resize border.
fn is_resize_hit_test(hit_test: u32) -> bool {
    matches!(
        hit_test,
        HTLEFT | HTRIGHT | HTTOP | HTTOPLEFT | HTTOPRIGHT | HTBOTTOM | HTBOTTOMLEFT | HTBOTTOMRIGHT
    )
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

    if restore_maximized_window(hwnd) {
        dbg_log(
            app,
            "tiling_win: restored maximized focused window before tiling",
        );
    }

    let win_rect = match get_hwnd_rect(hwnd) {
        Some(r) => r,
        None => {
            log::info!("tiling: could not get window rect");
            return;
        }
    };

    // Get displays (unpack work areas, discard DPI for tiling)
    let displays: Vec<Rect> = get_display_work_areas()
        .into_iter()
        .map(|(r, _)| r)
        .collect();
    if displays.is_empty() {
        log::warn!("tiling: no displays found");
        return;
    }

    // Log display info and window details for debugging (e.g. gap issues)
    let title = get_window_title(hwnd);
    let process = get_process_name(hwnd);
    let (bl, bt, br, bb) = get_dwm_border(hwnd);
    let display_info: Vec<String> = displays
        .iter()
        .enumerate()
        .map(|(i, d)| {
            format!(
                "D{}({},{} {}x{})",
                i, d.x as i32, d.y as i32, d.width as i32, d.height as i32
            )
        })
        .collect();
    dbg_log(
        app,
        &format!(
            "tiling_win: tile '{}' — layout={}, displays=[{}], window='{}' ({}), \
         visible_rect=({},{} {}x{}), dwm_border=({},{},{},{}), prefs=(half={}, third={}, gap={})",
            layout_str,
            layout_str,
            display_info.join(", "),
            title,
            process,
            win_rect.x as i32,
            win_rect.y as i32,
            win_rect.width as i32,
            win_rect.height as i32,
            bl,
            bt,
            br,
            bb,
            half_ratio,
            third_ratio,
            gap,
        ),
    );

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
    let target = calculate_target_rect(
        layout,
        &displays[target_display],
        half_ratio,
        third_ratio,
        gap,
    );
    dbg_log(
        app,
        &format!(
            "tiling_win: target_rect — display={}, layout={}, rect=({},{} {}x{})",
            target_display,
            layout_str,
            target.x as i32,
            target.y as i32,
            target.width as i32,
            target.height as i32,
        ),
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

    let (max_per_display, gap, spread, expose_min_w, expose_min_h) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        (
            (prefs.tiling.expose_columns * prefs.tiling.expose_rows) as usize,
            prefs.tiling.gap,
            prefs.tiling.expose_layout_strategy == "spread",
            prefs.tiling.expose_min_width as f64,
            prefs.tiling.expose_min_height as f64,
        )
    };

    restore_windows_for_expose();

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

    let display_infos = get_display_work_areas();
    if display_infos.is_empty() {
        return;
    }

    // Unpack work areas and compute DPI-scaled min cell sizes
    let displays: Vec<Rect> = display_infos.iter().map(|(r, _)| r.clone()).collect();
    let min_cell_sizes: Vec<(f64, f64)> = display_infos
        .iter()
        .map(|(_, scale)| (expose_min_w * scale, expose_min_h * scale))
        .collect();

    // Log display work areas for debugging gap issues
    let display_info: Vec<String> = display_infos
        .iter()
        .enumerate()
        .map(|(i, (d, scale))| {
            format!(
                "D{}({},{} {}x{} @{:.1}x)",
                i, d.x as i32, d.y as i32, d.width as i32, d.height as i32, scale
            )
        })
        .collect();
    let total_cap = max_per_display * displays.len();
    let cols = (max_per_display as f64).sqrt().ceil() as usize;
    let rows = if cols > 0 {
        (max_per_display + cols - 1) / cols
    } else {
        0
    };
    dbg_log(app, &format!(
        "tiling_win: expose — {} windows (cap={}), {} displays=[{}], grid={}x{} (max_per_display={}), gap={}, spread={}, min_cell={}x{}",
        all_windows.len(), total_cap, displays.len(), display_info.join(", "),
        cols, rows, max_per_display, gap, spread, expose_min_w as i32, expose_min_h as i32,
    ));
    for (i, w) in all_windows.iter().enumerate() {
        let min_str = w.min_size.map_or("none".to_string(), |(mw, mh)| {
            format!("{}x{}", mw as i32, mh as i32)
        });
        dbg_log(
            app,
            &format!(
                "tiling_win: expose_window[{}] — '{}' (pid={}), bounds=({},{} {}x{}), min_size={}",
                i,
                w.owner_name,
                w.owner_pid,
                w.bounds.x as i32,
                w.bounds.y as i32,
                w.bounds.width as i32,
                w.bounds.height as i32,
                min_str,
            ),
        );
    }

    let placements = plan_expose(
        &all_windows,
        &displays,
        max_per_display,
        gap as f64,
        spread,
        &min_cell_sizes,
    );
    for (i, p) in placements.iter().enumerate() {
        let hwnd = HWND(p.window_id as isize as *mut _);
        let title = get_window_title(hwnd);
        let process = get_process_name(hwnd);
        // Determine which display and grid position
        let display_idx = displays
            .iter()
            .position(|d| p.target.x >= d.x && p.target.x < d.x + d.width)
            .unwrap_or(0);
        dbg_log(
            app,
            &format!(
                "tiling_win: expose_place[{}] — '{}' ({}), display={}, target=({},{} {}x{})",
                i,
                title,
                process,
                display_idx,
                p.target.x as i32,
                p.target.y as i32,
                p.target.width as i32,
                p.target.height as i32,
            ),
        );
        set_hwnd_rect(hwnd, &p.target);
        unsafe {
            let _ = BringWindowToTop(hwnd);
        }
    }
    let placed = placements.len();
    let capped = all_windows.len().saturating_sub(placed);
    dbg_log(
        app,
        &format!(
            "tiling_win: expose_done — placed={}, capped={} (total_windows={}), displays={}",
            placed,
            capped,
            all_windows.len(),
            displays.len(),
        ),
    );
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
    let (max_per_display, gap, spread, expose_min_w, expose_min_h) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        (
            (prefs.tiling.expose_columns * prefs.tiling.expose_rows) as usize,
            prefs.tiling.gap,
            prefs.tiling.expose_layout_strategy == "spread",
            prefs.tiling.expose_min_width as f64,
            prefs.tiling.expose_min_height as f64,
        )
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

    restore_windows_for_expose();

    let mut all_windows = get_all_windows();
    if all_windows.is_empty() {
        return;
    }

    // Skip min_size query — see comment in execute_expose for rationale.

    let display_infos = get_display_work_areas();
    if display_infos.is_empty() {
        return;
    }

    // Unpack work areas and compute DPI-scaled min cell sizes
    let displays: Vec<Rect> = display_infos.iter().map(|(r, _)| r.clone()).collect();
    let min_cell_sizes: Vec<(f64, f64)> = display_infos
        .iter()
        .map(|(_, scale)| (expose_min_w * scale, expose_min_h * scale))
        .collect();

    // Log display work areas for debugging gap issues
    let display_info: Vec<String> = display_infos
        .iter()
        .enumerate()
        .map(|(i, (d, scale))| {
            format!(
                "D{}({},{} {}x{} @{:.1}x)",
                i, d.x as i32, d.y as i32, d.width as i32, d.height as i32, scale
            )
        })
        .collect();
    let app_window_count = all_windows
        .iter()
        .filter(|w| w.owner_pid == target_pid as i32)
        .count();
    let other_window_count = all_windows.len() - app_window_count;
    let total_cap = max_per_display * displays.len();
    dbg_log(
        app,
        &format!(
        "tiling_win: app_expose — app='{}' (pid={}), app_windows={}, other_windows={}, cap={}, \
         {} displays=[{}], max_per_display={}, gap={}, spread={}, min_cell={}x{}",
        target_app, target_pid, app_window_count, other_window_count, total_cap,
        displays.len(), display_info.join(", "), max_per_display, gap, spread,
        expose_min_w as i32, expose_min_h as i32,
    ),
    );
    for (i, w) in all_windows.iter().enumerate() {
        let is_target = if w.owner_pid == target_pid as i32 {
            "TARGET"
        } else {
            "other"
        };
        dbg_log(
            app,
            &format!(
                "tiling_win: app_expose_window[{}] — [{}] '{}' (pid={}), bounds=({},{} {}x{})",
                i,
                is_target,
                w.owner_name,
                w.owner_pid,
                w.bounds.x as i32,
                w.bounds.y as i32,
                w.bounds.width as i32,
                w.bounds.height as i32,
            ),
        );
    }

    // How many displays the app's windows will consume
    let app_displays_needed = if max_per_display > 0 {
        (app_window_count + max_per_display - 1) / max_per_display
    } else {
        0
    };
    let app_displays_used = app_displays_needed.min(displays.len());
    let displays_for_others = displays.len() - app_displays_used;
    let app_slots_total = app_displays_used * max_per_display;
    let app_slots_unused = app_slots_total.saturating_sub(app_window_count);
    let other_slots_total = displays_for_others * max_per_display;
    dbg_log(
        app,
        &format!(
        "tiling_win: app_expose_plan — app_windows={} → uses {} display(s) ({} slots, {} unused), \
         other_windows={} → {} display(s) remaining ({} slots)",
        app_window_count, app_displays_used, app_slots_total, app_slots_unused,
        other_window_count, displays_for_others, other_slots_total,
    ),
    );

    let placements = plan_expose_app(
        &all_windows,
        target_pid as i32,
        &displays,
        max_per_display,
        gap as f64,
        spread,
        &min_cell_sizes,
    );
    let mut app_placed = 0;
    let mut other_placed = 0;
    for (i, p) in placements.iter().enumerate() {
        let hwnd = HWND(p.window_id as isize as *mut _);
        let title = get_window_title(hwnd);
        let process = get_process_name(hwnd);
        let is_app = p.owner_pid == target_pid as i32;
        let tag = if is_app { "APP" } else { "other" };
        if is_app {
            app_placed += 1;
        } else {
            other_placed += 1;
        }
        let display_idx = displays
            .iter()
            .position(|d| p.target.x >= d.x && p.target.x < d.x + d.width)
            .unwrap_or(0);
        dbg_log(
            app,
            &format!(
            "tiling_win: app_expose_place[{}] — [{}] '{}' ({}), display={}, target=({},{} {}x{})",
            i, tag, title, process, display_idx,
            p.target.x as i32, p.target.y as i32, p.target.width as i32, p.target.height as i32,
        ),
        );
        set_hwnd_rect(hwnd, &p.target);
        unsafe {
            let _ = BringWindowToTop(hwnd);
        }
    }
    dbg_log(
        app,
        &format!(
            "tiling_win: app_expose_done — placed={} (app={}, other={}), \
         displays={} (app_used={}, other_used={})",
            placements.len(),
            app_placed,
            other_placed,
            displays.len(),
            app_displays_used,
            displays_for_others,
        ),
    );
}

/// Restore all minimized and maximized windows before exposé layout.
/// Minimized windows need to be restored so they appear in the grid.
/// Maximized windows need to be restored because SetWindowPos behaves
/// differently for maximized windows and their bounds span the full work area.
fn restore_windows_for_expose() {
    unsafe {
        unsafe extern "system" fn restore_callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
            if (IsIconic(hwnd).as_bool() || IsZoomed(hwnd).as_bool())
                && IsWindowVisible(hwnd).as_bool()
            {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            TRUE
        }

        let _ = EnumWindows(Some(restore_callback), LPARAM(0));
    }
    // No sleep needed — SetWindowPos will snap windows to their target rects
    // regardless of animation state. Window bounds are not used for placement
    // (grid targets come from display size / grid dimensions).
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
        (
            preset,
            prefs.tiling.half_ratio,
            prefs.tiling.third_ratio,
            prefs.tiling.gap,
        )
    };

    let windows = get_all_windows();
    if windows.is_empty() {
        log::info!("layout_preset: no windows found");
        return;
    }

    let displays: Vec<Rect> = get_display_work_areas()
        .into_iter()
        .map(|(r, _)| r)
        .collect();
    if displays.is_empty() {
        log::warn!("layout_preset: no displays found");
        return;
    }

    let placements = plan_layout_preset(&windows, &preset, &displays, half_ratio, third_ratio, gap);
    log::info!(
        "layout_preset: '{}' placing {} windows",
        preset.name,
        placements.len()
    );
    for p in &placements {
        unsafe {
            set_hwnd_rect(HWND(p.window_id as isize as *mut _), &p.target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_resize_hit_test, point_to_lparam, should_restore_before_tiling};
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP,
        HTTOPLEFT, HTTOPRIGHT,
    };

    /// Focused tiling restores maximized windows but never unminimizes them.
    #[test]
    fn test_should_restore_before_tiling_only_for_maximized_windows() {
        assert!(!should_restore_before_tiling(false, false));
        assert!(!should_restore_before_tiling(true, false));
        assert!(!should_restore_before_tiling(true, true));
        assert!(should_restore_before_tiling(false, true));
    }

    /// Move detection rejects every Win32 resize-border hit-test value.
    #[test]
    fn resize_hit_tests_are_rejected() {
        assert!(is_resize_hit_test(HTLEFT));
        assert!(is_resize_hit_test(HTRIGHT));
        assert!(is_resize_hit_test(HTTOP));
        assert!(is_resize_hit_test(HTTOPLEFT));
        assert!(is_resize_hit_test(HTTOPRIGHT));
        assert!(is_resize_hit_test(HTBOTTOM));
        assert!(is_resize_hit_test(HTBOTTOMLEFT));
        assert!(is_resize_hit_test(HTBOTTOMRIGHT));
        assert!(!is_resize_hit_test(HTCAPTION));
        assert!(!is_resize_hit_test(HTCLIENT));
    }

    /// `WM_NCHITTEST` coordinate packing preserves signed virtual-screen values.
    #[test]
    fn point_lparam_preserves_negative_coordinates() {
        let packed = point_to_lparam(POINT { x: -1200, y: 640 }).0 as u32;
        assert_eq!(packed as u16 as i16, -1200);
        assert_eq!((packed >> 16) as u16 as i16, 640);
    }
}
