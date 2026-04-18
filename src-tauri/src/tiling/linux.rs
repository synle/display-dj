//! Linux window tiling via X11/EWMH.
//!
//! Uses `x11rb` (pure Rust X11 client) with EWMH window manager hints to
//! move/resize windows. Requires an X11 session (`$DISPLAY` set).
//! Not supported on Wayland-only sessions — `is_x11_available()` returns false.

use super::{
    build_sorted_window_list, calculate_target_rect, find_display_for_window,
    layout_across_displays, layout_grid_on_display, Rect, TilingLayout, WindowInfo, WindowState,
};
use tauri::{AppHandle, Manager};
use x11rb::connection::Connection;
use x11rb::protocol::randr;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

// ---------------------------------------------------------------------------
// X11 availability check
// ---------------------------------------------------------------------------

/// Check if an X11 session is available (`$DISPLAY` env var set).
pub fn is_x11_available() -> bool {
    std::env::var("DISPLAY").is_ok()
}

/// Connect to the X11 server. Returns the connection and screen number.
fn connect() -> Option<(RustConnection, usize)> {
    x11rb::connect(None).ok()
}

// ---------------------------------------------------------------------------
// Atom / property helpers
// ---------------------------------------------------------------------------

/// Intern an X11 atom by name.
fn intern_atom(conn: &RustConnection, name: &str) -> Option<Atom> {
    conn.intern_atom(false, name.as_bytes())
        .ok()?
        .reply()
        .ok()
        .map(|r| r.atom)
}

/// Read a 32-bit cardinal property as a `Vec<u32>`.
fn get_cardinal_list(conn: &RustConnection, window: Window, property: Atom) -> Vec<u32> {
    conn.get_property(false, window, property, AtomEnum::CARDINAL, 0, 1024)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| {
            if r.format == 32 {
                r.value32().map(|iter| iter.collect())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Read a window-list property as `Vec<Window>`.
fn get_window_list(conn: &RustConnection, window: Window, property: Atom) -> Vec<Window> {
    conn.get_property(false, window, property, AtomEnum::WINDOW, 0, 4096)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| {
            if r.format == 32 {
                r.value32().map(|iter| iter.collect())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Read an atom-list property (e.g. `_NET_WM_STATE`).
fn get_atom_list(conn: &RustConnection, window: Window, property: Atom) -> Vec<Atom> {
    conn.get_property(false, window, property, AtomEnum::ATOM, 0, 64)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| {
            if r.format == 32 {
                r.value32().map(|iter| iter.collect())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Read a UTF-8 string property (e.g. `_NET_WM_NAME`).
fn get_utf8_property(
    conn: &RustConnection,
    window: Window,
    property: Atom,
    type_atom: Atom,
) -> Option<String> {
    let reply = conn
        .get_property(false, window, property, type_atom, 0, 1024)
        .ok()?
        .reply()
        .ok()?;
    if reply.format == 8 && !reply.value.is_empty() {
        Some(String::from_utf8_lossy(&reply.value).to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Strut (panel/dock reservation) handling
// ---------------------------------------------------------------------------

/// Panel/dock strut reservation from `_NET_WM_STRUT_PARTIAL`.
/// Values are in pixels relative to the root window edges.
struct StrutPartial {
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
    left_start_y: u32,
    left_end_y: u32,
    right_start_y: u32,
    right_end_y: u32,
    top_start_x: u32,
    top_end_x: u32,
    bottom_start_x: u32,
    bottom_end_x: u32,
}

/// Read strut data from a window. Tries `_NET_WM_STRUT_PARTIAL` first,
/// falls back to `_NET_WM_STRUT` (4-value, full-edge variant).
fn get_strut(
    conn: &RustConnection,
    window: Window,
    partial_atom: Atom,
    strut_atom: Atom,
    root_w: u32,
    root_h: u32,
) -> Option<StrutPartial> {
    // Try _NET_WM_STRUT_PARTIAL (12 values with edge ranges)
    let vals = get_cardinal_list(conn, window, partial_atom);
    if vals.len() >= 12 {
        return Some(StrutPartial {
            left: vals[0],
            right: vals[1],
            top: vals[2],
            bottom: vals[3],
            left_start_y: vals[4],
            left_end_y: vals[5],
            right_start_y: vals[6],
            right_end_y: vals[7],
            top_start_x: vals[8],
            top_end_x: vals[9],
            bottom_start_x: vals[10],
            bottom_end_x: vals[11],
        });
    }

    // Fallback: _NET_WM_STRUT (4 values, applies to full edge)
    let vals = get_cardinal_list(conn, window, strut_atom);
    if vals.len() >= 4 && (vals[0] > 0 || vals[1] > 0 || vals[2] > 0 || vals[3] > 0) {
        Some(StrutPartial {
            left: vals[0],
            right: vals[1],
            top: vals[2],
            bottom: vals[3],
            left_start_y: 0,
            left_end_y: root_h.saturating_sub(1),
            right_start_y: 0,
            right_end_y: root_h.saturating_sub(1),
            top_start_x: 0,
            top_end_x: root_w.saturating_sub(1),
            bottom_start_x: 0,
            bottom_end_x: root_w.saturating_sub(1),
        })
    } else {
        None
    }
}

/// Subtract strut reservations from a monitor's geometry to get the work area.
/// Strut edges are relative to the root window; `root_w`/`root_h` are needed
/// to convert right/bottom struts to absolute coordinates.
fn apply_struts_to_monitor(
    monitor: &Rect,
    struts: &[StrutPartial],
    root_w: u32,
    root_h: u32,
) -> Rect {
    let mut work = monitor.clone();

    for s in struts {
        // Top strut: reserved region y=[0, top), x=[top_start_x, top_end_x]
        if s.top > 0
            && (s.top_start_x as f64) < monitor.x + monitor.width
            && (s.top_end_x as f64) >= monitor.x
        {
            let strut_y = s.top as f64;
            if strut_y > work.y {
                let diff = strut_y - work.y;
                work.y = strut_y;
                work.height -= diff;
            }
        }
        // Bottom strut: reserved region y=[root_h - bottom, root_h)
        if s.bottom > 0
            && (s.bottom_start_x as f64) < monitor.x + monitor.width
            && (s.bottom_end_x as f64) >= monitor.x
        {
            let strut_y = (root_h - s.bottom) as f64;
            let work_bottom = work.y + work.height;
            if strut_y < work_bottom {
                work.height = strut_y - work.y;
            }
        }
        // Left strut: reserved region x=[0, left), y=[left_start_y, left_end_y]
        if s.left > 0
            && (s.left_start_y as f64) < monitor.y + monitor.height
            && (s.left_end_y as f64) >= monitor.y
        {
            let strut_x = s.left as f64;
            if strut_x > work.x {
                let diff = strut_x - work.x;
                work.x = strut_x;
                work.width -= diff;
            }
        }
        // Right strut: reserved region x=[root_w - right, root_w)
        if s.right > 0
            && (s.right_start_y as f64) < monitor.y + monitor.height
            && (s.right_end_y as f64) >= monitor.y
        {
            let strut_x = (root_w - s.right) as f64;
            let work_right = work.x + work.width;
            if strut_x < work_right {
                work.width = strut_x - work.x;
            }
        }
    }

    // Ensure positive dimensions
    work.width = work.width.max(1.0);
    work.height = work.height.max(1.0);
    work
}

// ---------------------------------------------------------------------------
// Display enumeration
// ---------------------------------------------------------------------------

/// Get per-monitor geometry from XRandr.
fn get_xrandr_monitors(conn: &RustConnection, root: Window) -> Vec<Rect> {
    randr::get_monitors(conn, root, true)
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| {
            r.monitors
                .iter()
                .map(|m| Rect {
                    x: m.x as f64,
                    y: m.y as f64,
                    width: m.width as f64,
                    height: m.height as f64,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Get work areas for all monitors, sorted left-to-right.
/// Uses XRandr for geometry and `_NET_WM_STRUT_PARTIAL` / `_NET_WM_STRUT`
/// for panel reservations. Falls back to `_NET_WORKAREA` for single-monitor setups.
fn get_display_work_areas() -> Vec<Rect> {
    let (conn, screen_num) = match connect() {
        Some(c) => c,
        None => return Vec::new(),
    };
    let setup = conn.setup();
    let screen = &setup.roots[screen_num];
    let root = screen.root;
    let root_w = screen.width_in_pixels as u32;
    let root_h = screen.height_in_pixels as u32;

    let mut monitors = get_xrandr_monitors(&conn, root);
    if monitors.is_empty() {
        monitors.push(Rect {
            x: 0.0,
            y: 0.0,
            width: root_w as f64,
            height: root_h as f64,
        });
    }

    // Sort left-to-right, then top-to-bottom
    monitors.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });

    // Read struts from panel/dock windows
    let sp_atom = intern_atom(&conn, "_NET_WM_STRUT_PARTIAL");
    let s_atom = intern_atom(&conn, "_NET_WM_STRUT");
    let cl_atom = intern_atom(&conn, "_NET_CLIENT_LIST");

    if let (Some(sp), Some(s), Some(cl)) = (sp_atom, s_atom, cl_atom) {
        let clients = get_window_list(&conn, root, cl);
        let struts: Vec<StrutPartial> = clients
            .iter()
            .filter_map(|&w| get_strut(&conn, w, sp, s, root_w, root_h))
            .collect();

        if !struts.is_empty() {
            return monitors
                .iter()
                .map(|m| apply_struts_to_monitor(m, &struts, root_w, root_h))
                .collect();
        }
    }

    // No struts found — try _NET_WORKAREA as fallback for single monitor
    if monitors.len() == 1 {
        if let Some(wa_atom) = intern_atom(&conn, "_NET_WORKAREA") {
            let wa = get_cardinal_list(&conn, root, wa_atom);
            if wa.len() >= 4 {
                return vec![Rect {
                    x: wa[0] as f64,
                    y: wa[1] as f64,
                    width: wa[2] as f64,
                    height: wa[3] as f64,
                }];
            }
        }
    }

    monitors
}

// ---------------------------------------------------------------------------
// Window helpers
// ---------------------------------------------------------------------------

/// Get the currently focused (active) window via `_NET_ACTIVE_WINDOW`.
fn get_focused_window(conn: &RustConnection, root: Window) -> Option<Window> {
    let atom = intern_atom(conn, "_NET_ACTIVE_WINDOW")?;
    let windows = get_window_list(conn, root, atom);
    windows.first().copied().filter(|&w| w != 0)
}

/// Get `_NET_FRAME_EXTENTS` (left, right, top, bottom) for a window.
/// Returns `(0, 0, 0, 0)` if not available.
fn get_frame_extents(conn: &RustConnection, window: Window) -> (i32, i32, i32, i32) {
    let atom = match intern_atom(conn, "_NET_FRAME_EXTENTS") {
        Some(a) => a,
        None => return (0, 0, 0, 0),
    };
    let vals = get_cardinal_list(conn, window, atom);
    if vals.len() >= 4 {
        (vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32)
    } else {
        (0, 0, 0, 0)
    }
}

/// Get the visible frame position and size of a window, including decorations.
/// Uses `GetGeometry` + `TranslateCoordinates` for the client area, then
/// expands by `_NET_FRAME_EXTENTS` to get the full visible bounding box.
fn get_window_rect(conn: &RustConnection, window: Window, root: Window) -> Option<Rect> {
    let geo = conn.get_geometry(window).ok()?.reply().ok()?;
    let trans = conn
        .translate_coordinates(window, root, 0, 0)
        .ok()?
        .reply()
        .ok()?;
    let (fl, fr, ft, fb) = get_frame_extents(conn, window);

    Some(Rect {
        x: trans.dst_x as f64 - fl as f64,
        y: trans.dst_y as f64 - ft as f64,
        width: geo.width as f64 + fl as f64 + fr as f64,
        height: geo.height as f64 + ft as f64 + fb as f64,
    })
}

/// Remove maximized and fullscreen states from a window before tiling.
fn unmaximize_window(conn: &RustConnection, root: Window, window: Window) {
    let net_wm_state = match intern_atom(conn, "_NET_WM_STATE") {
        Some(a) => a,
        None => return,
    };

    let state_names = [
        "_NET_WM_STATE_MAXIMIZED_HORZ",
        "_NET_WM_STATE_MAXIMIZED_VERT",
        "_NET_WM_STATE_FULLSCREEN",
    ];
    let atoms: Vec<Atom> = state_names
        .iter()
        .filter_map(|name| intern_atom(conn, name))
        .collect();

    let current = get_atom_list(conn, window, net_wm_state);
    let needs_change = atoms.iter().any(|a| current.contains(a));
    if !needs_change {
        return;
    }

    // Send _NET_WM_STATE client messages to remove states (two atoms per message)
    for chunk in atoms.chunks(2) {
        let a1 = chunk[0];
        let a2 = chunk.get(1).copied().unwrap_or(0);
        let event = ClientMessageEvent::new(
            32,
            window,
            net_wm_state,
            [0u32, a1, a2, 2, 0], // action=REMOVE(0), source=pager(2)
        );
        let _ = conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
    }
    let _ = conn.flush();
    std::thread::sleep(std::time::Duration::from_millis(50));
}

/// Move and resize a window to the given rect, compensating for frame extents.
/// The rect specifies the desired *visible* frame (including decorations).
/// Uses `_NET_MOVERESIZE_WINDOW` with NorthWestGravity so x,y is the frame's
/// top-left; width/height are adjusted to client-area dimensions.
fn set_window_rect(conn: &RustConnection, screen_num: usize, window: Window, rect: &Rect) {
    let root = conn.setup().roots[screen_num].root;

    unmaximize_window(conn, root, window);

    let (fl, fr, ft, fb) = get_frame_extents(conn, window);
    let client_w = ((rect.width as i32) - fl - fr).max(1) as u32;
    let client_h = ((rect.height as i32) - ft - fb).max(1) as u32;

    // gravity=NorthWest(1), flags: x|y|w|h present (bits 8-11), source=pager (bits 12-13)
    let gravity_and_flags: u32 = 1 | (0xf << 8) | (2 << 12);

    if let Some(move_atom) = intern_atom(conn, "_NET_MOVERESIZE_WINDOW") {
        let event = ClientMessageEvent::new(
            32,
            window,
            move_atom,
            [
                gravity_and_flags,
                rect.x as u32,
                rect.y as u32,
                client_w,
                client_h,
            ],
        );
        let _ = conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
        let _ = conn.flush();
    }
}

/// Get the window title. Tries `_NET_WM_NAME` (UTF-8) first, falls back to `WM_NAME`.
fn get_window_title(conn: &RustConnection, window: Window) -> String {
    if let (Some(name_atom), Some(utf8_atom)) = (
        intern_atom(conn, "_NET_WM_NAME"),
        intern_atom(conn, "UTF8_STRING"),
    ) {
        if let Some(name) = get_utf8_property(conn, window, name_atom, utf8_atom) {
            return name;
        }
    }

    // Fallback to WM_NAME (Latin-1)
    conn.get_property(false, window, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| {
            if r.format == 8 && !r.value.is_empty() {
                Some(String::from_utf8_lossy(&r.value).to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Get `_NET_WM_PID` for a window.
fn get_window_pid(conn: &RustConnection, window: Window) -> Option<u32> {
    let atom = intern_atom(conn, "_NET_WM_PID")?;
    let vals = get_cardinal_list(conn, window, atom);
    vals.first().copied()
}

/// Get process name from PID by reading `/proc/{pid}/comm`.
fn get_process_name_from_pid(pid: u32) -> String {
    std::fs::read_to_string(format!("/proc/{}/comm", pid))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Query minimum window size from `WM_NORMAL_HINTS` (PMinSize flag).
fn get_window_min_size(conn: &RustConnection, window: Window) -> Option<(f64, f64)> {
    let reply = conn
        .get_property(
            false,
            window,
            AtomEnum::WM_NORMAL_HINTS,
            AtomEnum::WM_SIZE_HINTS,
            0,
            18,
        )
        .ok()?
        .reply()
        .ok()?;
    if reply.format != 32 {
        return None;
    }
    let vals: Vec<u32> = reply.value32()?.collect();
    if vals.len() < 7 {
        return None;
    }
    // PMinSize flag = bit 4 (0x10); min_width at index 5, min_height at index 6
    if vals[0] & 0x10 != 0 {
        let w = vals[5] as f64;
        let h = vals[6] as f64;
        if w > 0.0 && h > 0.0 {
            return Some((w, h));
        }
    }
    None
}

/// Check if a window is a normal application window (not a dock/desktop/splash).
fn is_normal_window(conn: &RustConnection, window: Window) -> bool {
    let type_atom = match intern_atom(conn, "_NET_WM_WINDOW_TYPE") {
        Some(a) => a,
        None => return true,
    };
    let types = get_atom_list(conn, window, type_atom);
    if types.is_empty() {
        return true; // No type hint — assume normal
    }
    let normal = intern_atom(conn, "_NET_WM_WINDOW_TYPE_NORMAL");
    let dialog = intern_atom(conn, "_NET_WM_WINDOW_TYPE_DIALOG");
    types
        .iter()
        .any(|t| Some(*t) == normal || Some(*t) == dialog)
}

/// Check if a window's `_NET_WM_STATE` includes `_NET_WM_STATE_HIDDEN` (minimized).
fn is_window_hidden(conn: &RustConnection, window: Window) -> bool {
    let state_atom = match intern_atom(conn, "_NET_WM_STATE") {
        Some(a) => a,
        None => return false,
    };
    let hidden_atom = match intern_atom(conn, "_NET_WM_STATE_HIDDEN") {
        Some(a) => a,
        None => return false,
    };
    get_atom_list(conn, window, state_atom).contains(&hidden_atom)
}

/// Enumerate all visible, non-minimized, normal top-level windows.
fn get_all_windows() -> Vec<WindowInfo> {
    let (conn, screen_num) = match connect() {
        Some(c) => c,
        None => return Vec::new(),
    };
    let root = conn.setup().roots[screen_num].root;
    let net_client_list = match intern_atom(&conn, "_NET_CLIENT_LIST") {
        Some(a) => a,
        None => return Vec::new(),
    };

    let clients = get_window_list(&conn, root, net_client_list);
    let mut windows = Vec::new();

    for wid in clients {
        if is_window_hidden(&conn, wid) {
            continue;
        }
        if !is_normal_window(&conn, wid) {
            continue;
        }

        let rect = match get_window_rect(&conn, wid, root) {
            Some(r) => r,
            None => continue,
        };
        if rect.width < 50.0 || rect.height < 50.0 {
            continue;
        }

        let title = get_window_title(&conn, wid);
        if title.is_empty() {
            continue;
        }

        let pid = get_window_pid(&conn, wid).unwrap_or(0);
        let owner_name = if pid > 0 {
            let name = get_process_name_from_pid(pid);
            if name.is_empty() {
                title.clone()
            } else {
                name
            }
        } else {
            title.clone()
        };

        windows.push(WindowInfo {
            window_id: wid as i64,
            owner_pid: pid as i32,
            owner_name,
            bounds: rect,
            min_size: None,
        });
    }

    windows
}

/// Raise a window to the foreground using `_NET_ACTIVE_WINDOW`.
fn raise_window(conn: &RustConnection, root: Window, window: Window) {
    if let Some(active_atom) = intern_atom(conn, "_NET_ACTIVE_WINDOW") {
        let event = ClientMessageEvent::new(
            32,
            window,
            active_atom,
            [2u32, 0, 0, 0, 0], // source=pager(2)
        );
        let _ = conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
        let _ = conn.flush();
    }
}

/// Restore all minimized (hidden) normal windows before exposé layout.
fn restore_minimized_windows(conn: &RustConnection, root: Window) {
    let net_client_list = match intern_atom(conn, "_NET_CLIENT_LIST") {
        Some(a) => a,
        None => return,
    };
    let net_wm_state = match intern_atom(conn, "_NET_WM_STATE") {
        Some(a) => a,
        None => return,
    };
    let hidden_atom = match intern_atom(conn, "_NET_WM_STATE_HIDDEN") {
        Some(a) => a,
        None => return,
    };

    let clients = get_window_list(conn, root, net_client_list);
    let mut restored_any = false;

    for wid in clients {
        if !is_normal_window(conn, wid) {
            continue;
        }
        let states = get_atom_list(conn, wid, net_wm_state);
        if states.contains(&hidden_atom) {
            // Remove _NET_WM_STATE_HIDDEN
            let event = ClientMessageEvent::new(
                32,
                wid,
                net_wm_state,
                [0u32, hidden_atom, 0, 2, 0], // REMOVE, source=pager
            );
            let _ = conn.send_event(
                false,
                root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            );
            let _ = conn.map_window(wid);
            restored_any = true;
        }
    }

    if restored_any {
        let _ = conn.flush();
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Execute a tiling command on the focused window.
/// `layout_str` is a camelCase layout name (e.g. "leftHalf") or "restore".
pub fn execute_tile(app: &AppHandle, layout_str: &str) {
    if !is_x11_available() {
        log::warn!("tiling: X11 not available");
        return;
    }

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

    if layout_str == "restore" {
        execute_restore(app);
        return;
    }

    let layout = match TilingLayout::parse(layout_str) {
        Some(l) => l,
        None => {
            log::warn!("tiling: unknown layout '{}'", layout_str);
            return;
        }
    };

    let (conn, screen_num) = match connect() {
        Some(c) => c,
        None => {
            log::warn!("tiling: failed to connect to X11");
            return;
        }
    };
    let root = conn.setup().roots[screen_num].root;

    let window = match get_focused_window(&conn, root) {
        Some(w) => w,
        None => {
            log::info!("tiling: no focused window");
            return;
        }
    };

    let win_rect = match get_window_rect(&conn, window, root) {
        Some(r) => r,
        None => {
            log::info!("tiling: could not get window rect");
            return;
        }
    };

    let displays = get_display_work_areas();
    if displays.is_empty() {
        log::warn!("tiling: no displays found");
        return;
    }

    let target_display = find_display_for_window(&win_rect, &displays);
    let window_key = window as i64;

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

    let target = calculate_target_rect(
        layout,
        &displays[target_display],
        half_ratio,
        third_ratio,
        gap,
    );
    log::info!(
        "tiling: {} on display {} -> ({}, {}, {}x{})",
        layout_str,
        target_display,
        target.x,
        target.y,
        target.width,
        target.height,
    );
    set_window_rect(&conn, screen_num, window, &target);
}

/// Restore the focused window to its pre-tiled position and size.
fn execute_restore(app: &AppHandle) {
    let (conn, screen_num) = match connect() {
        Some(c) => c,
        None => return,
    };
    let root = conn.setup().roots[screen_num].root;

    let window = match get_focused_window(&conn, root) {
        Some(w) => w,
        None => return,
    };
    let window_key = window as i64;

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
        set_window_rect(&conn, screen_num, window, &rect);
    } else {
        log::info!("tiling: no saved state to restore");
    }
}

/// Execute the exposé command. Lays out all on-screen windows in a grid.
pub fn execute_expose(app: &AppHandle) {
    if !is_x11_available() {
        log::warn!("expose: X11 not available");
        return;
    }

    let (max_per_display, gap) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        ((prefs.tiling.expose_columns * prefs.tiling.expose_rows) as usize, prefs.tiling.gap)
    };

    let (conn, screen_num) = match connect() {
        Some(c) => c,
        None => return,
    };
    let root = conn.setup().roots[screen_num].root;

    restore_minimized_windows(&conn, root);

    let mut all_windows = get_all_windows();
    if all_windows.is_empty() {
        log::info!("expose: no windows found");
        return;
    }

    // Populate min sizes from WM_NORMAL_HINTS
    for w in &mut all_windows {
        w.min_size = get_window_min_size(&conn, w.window_id as u32);
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
    let placed = layout_across_displays(&ordered, &displays, max_per_display, g, &|win_info, rect| {
        let wid = win_info.window_id as u32;
        set_window_rect(&conn, screen_num, wid, rect);
        raise_window(&conn, root, wid);
    });

    log::info!(
        "expose: spread {} windows across {} displays",
        placed,
        displays.len()
    );
}

/// Execute the app exposé command. Lays out the frontmost app's windows in a grid.
pub fn execute_expose_app(app: &AppHandle) {
    if !is_x11_available() {
        log::warn!("app_expose: X11 not available");
        return;
    }

    let (max_per_display, gap) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        ((prefs.tiling.expose_columns * prefs.tiling.expose_rows) as usize, prefs.tiling.gap)
    };

    let (conn, screen_num) = match connect() {
        Some(c) => c,
        None => return,
    };
    let root = conn.setup().roots[screen_num].root;

    let focused = match get_focused_window(&conn, root) {
        Some(w) => w,
        None => {
            log::info!("app_expose: no focused window");
            return;
        }
    };

    let target_pid = get_window_pid(&conn, focused).unwrap_or(0);
    let target_app = if target_pid > 0 {
        get_process_name_from_pid(target_pid)
    } else {
        String::new()
    };

    restore_minimized_windows(&conn, root);

    let mut all_windows = get_all_windows();
    if all_windows.is_empty() {
        return;
    }

    // Populate min sizes from WM_NORMAL_HINTS
    for w in &mut all_windows {
        w.min_size = get_window_min_size(&conn, w.window_id as u32);
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
    let placed = layout_across_displays(&app_windows, &displays, max_per_display, g, &|win_info, rect| {
        let wid = win_info.window_id as u32;
        set_window_rect(&conn, screen_num, wid, rect);
        raise_window(&conn, root, wid);
    });

    log::info!(
        "app_expose: spread {} windows of '{}' across {} displays",
        placed,
        target_app,
        displays.len()
    );
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

    let (conn, screen_num) = match connect() {
        Some(c) => c,
        None => return,
    };

    let matches = super::match_windows_to_rules(&windows, &preset.rules);
    log::info!("layout_preset: '{}' matched {} windows", preset.name, matches.len());

    for (win_idx, layout, disp_idx) in matches {
        let w = &windows[win_idx];
        let display_index = disp_idx
            .unwrap_or_else(|| super::find_display_for_window(&w.bounds, &displays))
            .min(displays.len() - 1);
        let target = super::calculate_target_rect(layout, &displays[display_index], half_ratio, third_ratio, gap);
        set_window_rect(&conn, screen_num, w.window_id as u32, &target);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_x11_available_returns_bool() {
        // Verify it runs without panicking (may be true or false depending on env)
        let _ = is_x11_available();
    }

    #[test]
    fn test_apply_struts_no_struts() {
        let monitor = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let result = apply_struts_to_monitor(&monitor, &[], 1920, 1080);
        assert!((result.x).abs() < 0.01);
        assert!((result.y).abs() < 0.01);
        assert!((result.width - 1920.0).abs() < 0.01);
        assert!((result.height - 1080.0).abs() < 0.01);
    }

    #[test]
    fn test_apply_struts_top_panel() {
        let monitor = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![StrutPartial {
            left: 0,
            right: 0,
            top: 28,
            bottom: 0,
            left_start_y: 0,
            left_end_y: 0,
            right_start_y: 0,
            right_end_y: 0,
            top_start_x: 0,
            top_end_x: 1920,
            bottom_start_x: 0,
            bottom_end_x: 0,
        }];
        let result = apply_struts_to_monitor(&monitor, &struts, 1920, 1080);
        assert!((result.y - 28.0).abs() < 0.01);
        assert!((result.height - 1052.0).abs() < 0.01);
    }

    #[test]
    fn test_apply_struts_bottom_dock() {
        let monitor = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![StrutPartial {
            left: 0,
            right: 0,
            top: 0,
            bottom: 48,
            left_start_y: 0,
            left_end_y: 0,
            right_start_y: 0,
            right_end_y: 0,
            top_start_x: 0,
            top_end_x: 0,
            bottom_start_x: 0,
            bottom_end_x: 1920,
        }];
        let result = apply_struts_to_monitor(&monitor, &struts, 1920, 1080);
        assert!((result.y).abs() < 0.01);
        assert!((result.height - 1032.0).abs() < 0.01);
    }

    #[test]
    fn test_apply_struts_left_dock() {
        let monitor = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![StrutPartial {
            left: 64,
            right: 0,
            top: 0,
            bottom: 0,
            left_start_y: 0,
            left_end_y: 1080,
            right_start_y: 0,
            right_end_y: 0,
            top_start_x: 0,
            top_end_x: 0,
            bottom_start_x: 0,
            bottom_end_x: 0,
        }];
        let result = apply_struts_to_monitor(&monitor, &struts, 1920, 1080);
        assert!((result.x - 64.0).abs() < 0.01);
        assert!((result.width - 1856.0).abs() < 0.01);
    }

    #[test]
    fn test_apply_struts_dual_monitor_panel_on_first() {
        // Panel on monitor 1 only (top, x=0..1919 inclusive). Monitor 2 unaffected.
        let mon1 = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let mon2 = Rect {
            x: 1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![StrutPartial {
            left: 0,
            right: 0,
            top: 28,
            bottom: 0,
            left_start_y: 0,
            left_end_y: 0,
            right_start_y: 0,
            right_end_y: 0,
            top_start_x: 0,
            top_end_x: 1919, // inclusive end — real panels use last pixel of their monitor
            bottom_start_x: 0,
            bottom_end_x: 0,
        }];
        let r1 = apply_struts_to_monitor(&mon1, &struts, 3840, 1080);
        let r2 = apply_struts_to_monitor(&mon2, &struts, 3840, 1080);
        assert!((r1.y - 28.0).abs() < 0.01);
        assert!((r1.height - 1052.0).abs() < 0.01);
        assert!((r2.y).abs() < 0.01);
        assert!((r2.height - 1080.0).abs() < 0.01);
    }

    #[test]
    fn test_apply_struts_top_and_bottom() {
        let monitor = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![
            StrutPartial {
                left: 0,
                right: 0,
                top: 28,
                bottom: 0,
                left_start_y: 0,
                left_end_y: 0,
                right_start_y: 0,
                right_end_y: 0,
                top_start_x: 0,
                top_end_x: 1920,
                bottom_start_x: 0,
                bottom_end_x: 0,
            },
            StrutPartial {
                left: 0,
                right: 0,
                top: 0,
                bottom: 48,
                left_start_y: 0,
                left_end_y: 0,
                right_start_y: 0,
                right_end_y: 0,
                top_start_x: 0,
                top_end_x: 0,
                bottom_start_x: 0,
                bottom_end_x: 1920,
            },
        ];
        let result = apply_struts_to_monitor(&monitor, &struts, 1920, 1080);
        assert!((result.y - 28.0).abs() < 0.01);
        assert!((result.height - 1004.0).abs() < 0.01); // 1080 - 28 - 48
    }

    #[test]
    fn test_process_name_from_pid_nonexistent() {
        assert_eq!(get_process_name_from_pid(0), "");
        assert_eq!(get_process_name_from_pid(999_999_999), "");
    }

    #[test]
    fn test_apply_struts_right_dock() {
        let monitor = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![StrutPartial {
            left: 0,
            right: 64,
            top: 0,
            bottom: 0,
            left_start_y: 0,
            left_end_y: 0,
            right_start_y: 0,
            right_end_y: 1079,
            top_start_x: 0,
            top_end_x: 0,
            bottom_start_x: 0,
            bottom_end_x: 0,
        }];
        let result = apply_struts_to_monitor(&monitor, &struts, 1920, 1080);
        assert!((result.x).abs() < 0.01);
        assert!((result.width - 1856.0).abs() < 0.01); // 1920 - 64
    }

    #[test]
    fn test_apply_struts_covers_full_monitor_height() {
        // Strut reserves entire monitor height — work area should clamp to min 1.0
        let monitor = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![StrutPartial {
            left: 0,
            right: 0,
            top: 1080,
            bottom: 0,
            left_start_y: 0,
            left_end_y: 0,
            right_start_y: 0,
            right_end_y: 0,
            top_start_x: 0,
            top_end_x: 1919,
            bottom_start_x: 0,
            bottom_end_x: 0,
        }];
        let result = apply_struts_to_monitor(&monitor, &struts, 1920, 1080);
        assert!((result.y - 1080.0).abs() < 0.01);
        assert!((result.height - 1.0).abs() < 0.01); // clamped to min 1.0
    }

    #[test]
    fn test_apply_struts_overlapping_same_edge() {
        // Two top panels — only the larger one should win (struts applied sequentially)
        let monitor = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![
            StrutPartial {
                left: 0,
                right: 0,
                top: 28,
                bottom: 0,
                left_start_y: 0,
                left_end_y: 0,
                right_start_y: 0,
                right_end_y: 0,
                top_start_x: 0,
                top_end_x: 1919,
                bottom_start_x: 0,
                bottom_end_x: 0,
            },
            StrutPartial {
                left: 0,
                right: 0,
                top: 50,
                bottom: 0,
                left_start_y: 0,
                left_end_y: 0,
                right_start_y: 0,
                right_end_y: 0,
                top_start_x: 0,
                top_end_x: 1919,
                bottom_start_x: 0,
                bottom_end_x: 0,
            },
        ];
        let result = apply_struts_to_monitor(&monitor, &struts, 1920, 1080);
        // Second strut (50) is larger, applied after first (28)
        assert!((result.y - 50.0).abs() < 0.01);
        assert!((result.height - 1030.0).abs() < 0.01);
    }

    #[test]
    fn test_apply_struts_all_four_edges() {
        let monitor = Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![StrutPartial {
            left: 64,
            right: 64,
            top: 28,
            bottom: 48,
            left_start_y: 0,
            left_end_y: 1079,
            right_start_y: 0,
            right_end_y: 1079,
            top_start_x: 0,
            top_end_x: 1919,
            bottom_start_x: 0,
            bottom_end_x: 1919,
        }];
        let result = apply_struts_to_monitor(&monitor, &struts, 1920, 1080);
        assert!((result.x - 64.0).abs() < 0.01);
        assert!((result.y - 28.0).abs() < 0.01);
        assert!((result.width - 1792.0).abs() < 0.01); // 1920 - 64 - 64
        assert!((result.height - 1004.0).abs() < 0.01); // 1080 - 28 - 48
    }

    #[test]
    fn test_apply_struts_monitor_with_offset() {
        // Monitor 2 at x=1920, with a right strut on the root's right edge
        let monitor = Rect {
            x: 1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![StrutPartial {
            left: 0,
            right: 48,
            top: 0,
            bottom: 0,
            left_start_y: 0,
            left_end_y: 0,
            right_start_y: 0,
            right_end_y: 1079,
            top_start_x: 0,
            top_end_x: 0,
            bottom_start_x: 0,
            bottom_end_x: 0,
        }];
        // root is 3840 wide, right strut = 48, so reserved x = [3792, 3840)
        let result = apply_struts_to_monitor(&monitor, &struts, 3840, 1080);
        assert!((result.width - 1872.0).abs() < 0.01); // 1920 - 48
    }

    #[test]
    fn test_apply_struts_zero_strut_values_unchanged() {
        // All zero struts should leave monitor unchanged
        let monitor = Rect {
            x: 100.0,
            y: 50.0,
            width: 1280.0,
            height: 720.0,
        };
        let struts = vec![StrutPartial {
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
            left_start_y: 0,
            left_end_y: 0,
            right_start_y: 0,
            right_end_y: 0,
            top_start_x: 0,
            top_end_x: 0,
            bottom_start_x: 0,
            bottom_end_x: 0,
        }];
        let result = apply_struts_to_monitor(&monitor, &struts, 1920, 1080);
        assert!((result.x - 100.0).abs() < 0.01);
        assert!((result.y - 50.0).abs() < 0.01);
        assert!((result.width - 1280.0).abs() < 0.01);
        assert!((result.height - 720.0).abs() < 0.01);
    }

    #[test]
    fn test_apply_struts_non_overlapping_range_ignored() {
        // Top strut at x=[0, 500] should NOT affect monitor at x=1920
        let monitor = Rect {
            x: 1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let struts = vec![StrutPartial {
            left: 0,
            right: 0,
            top: 28,
            bottom: 0,
            left_start_y: 0,
            left_end_y: 0,
            right_start_y: 0,
            right_end_y: 0,
            top_start_x: 0,
            top_end_x: 500,
            bottom_start_x: 0,
            bottom_end_x: 0,
        }];
        let result = apply_struts_to_monitor(&monitor, &struts, 3840, 1080);
        assert!((result.y).abs() < 0.01); // unaffected
        assert!((result.height - 1080.0).abs() < 0.01);
    }
}
