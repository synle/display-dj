//! Cross-platform window tiling.
//!
//! Provides window tiling commands (halves, thirds, quarters, maximize, restore, exposé).
//! Platform-specific implementations live in submodules; this module contains shared
//! types, layout math, and the public API that delegates to the active platform.

use std::collections::HashMap;
use tauri::AppHandle;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

// ---------------------------------------------------------------------------
// Public types (shared across platforms)
// ---------------------------------------------------------------------------

/// A rectangle in screen coordinates (top-left origin, points/pixels).
#[derive(Clone, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    /// Center X coordinate.
    pub(crate) fn cx(&self) -> f64 {
        self.x + self.width / 2.0
    }
    /// Center Y coordinate.
    pub(crate) fn cy(&self) -> f64 {
        self.y + self.height / 2.0
    }
}

/// One of the 19 supported tiling layouts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TilingLayout {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    LeftThird,
    CenterThird,
    RightThird,
    TopThird,
    MiddleThird,
    BottomThird,
    LeftTwoThirds,
    RightTwoThirds,
    TopTwoThirds,
    BottomTwoThirds,
    TopLeftQuarter,
    TopRightQuarter,
    BottomLeftQuarter,
    BottomRightQuarter,
    Maximize,
}

impl TilingLayout {
    /// Parse a camelCase layout name into a TilingLayout.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "leftHalf" => Some(Self::LeftHalf),
            "rightHalf" => Some(Self::RightHalf),
            "topHalf" => Some(Self::TopHalf),
            "bottomHalf" => Some(Self::BottomHalf),
            "leftThird" => Some(Self::LeftThird),
            "centerThird" => Some(Self::CenterThird),
            "rightThird" => Some(Self::RightThird),
            "topThird" => Some(Self::TopThird),
            "middleThird" => Some(Self::MiddleThird),
            "bottomThird" => Some(Self::BottomThird),
            "leftTwoThirds" => Some(Self::LeftTwoThirds),
            "rightTwoThirds" => Some(Self::RightTwoThirds),
            "topTwoThirds" => Some(Self::TopTwoThirds),
            "bottomTwoThirds" => Some(Self::BottomTwoThirds),
            "topLeftQuarter" => Some(Self::TopLeftQuarter),
            "topRightQuarter" => Some(Self::TopRightQuarter),
            "bottomLeftQuarter" => Some(Self::BottomLeftQuarter),
            "bottomRightQuarter" => Some(Self::BottomRightQuarter),
            "maximize" => Some(Self::Maximize),
            _ => None,
        }
    }
}

/// Per-window tiling state tracked at runtime.
#[derive(Clone, Debug)]
pub(crate) struct WindowState {
    /// Original position/size before first tile (for restore).
    pub original: Rect,
    /// Current tiling layout applied to this window.
    pub layout: TilingLayout,
    /// Display index the window is currently tiled on.
    pub display_index: usize,
}

/// Runtime tiling state, keyed by platform window ID (CGWindowID on macOS, HWND as isize on Windows).
pub struct TilingState {
    pub(crate) windows: HashMap<i64, WindowState>,
}

impl TilingState {
    /// Create empty tiling state.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }
}

impl Default for TilingState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Layout calculation (pure geometry — platform-independent)
// ---------------------------------------------------------------------------

/// Calculate the target rectangle for a layout on a given display.
/// `half_ratio` and `third_ratio` are percentages (e.g. 50, 33).
/// `gap` is the outer padding in points around the usable area.
pub fn calculate_target_rect(
    layout: TilingLayout,
    display: &Rect,
    half_ratio: u32,
    third_ratio: u32,
    gap: u32,
) -> Rect {
    let g = gap as f64;
    let dx = display.x + g;
    let dy = display.y + g;
    let dw = display.width - 2.0 * g;
    let dh = display.height - 2.0 * g;
    let h = half_ratio as f64 / 100.0;
    let t = third_ratio as f64 / 100.0;

    match layout {
        // Halves
        TilingLayout::LeftHalf => Rect {
            x: dx,
            y: dy,
            width: dw * h,
            height: dh,
        },
        TilingLayout::RightHalf => Rect {
            x: dx + dw * h,
            y: dy,
            width: dw * (1.0 - h),
            height: dh,
        },
        TilingLayout::TopHalf => Rect {
            x: dx,
            y: dy,
            width: dw,
            height: dh * h,
        },
        TilingLayout::BottomHalf => Rect {
            x: dx,
            y: dy + dh * h,
            width: dw,
            height: dh * (1.0 - h),
        },

        // Thirds (horizontal)
        TilingLayout::LeftThird => Rect {
            x: dx,
            y: dy,
            width: dw * t,
            height: dh,
        },
        TilingLayout::CenterThird => Rect {
            x: dx + dw * t,
            y: dy,
            width: dw * (1.0 - 2.0 * t),
            height: dh,
        },
        TilingLayout::RightThird => Rect {
            x: dx + dw * (1.0 - t),
            y: dy,
            width: dw * t,
            height: dh,
        },

        // Thirds (vertical)
        TilingLayout::TopThird => Rect {
            x: dx,
            y: dy,
            width: dw,
            height: dh * t,
        },
        TilingLayout::MiddleThird => Rect {
            x: dx,
            y: dy + dh * t,
            width: dw,
            height: dh * (1.0 - 2.0 * t),
        },
        TilingLayout::BottomThird => Rect {
            x: dx,
            y: dy + dh * (1.0 - t),
            width: dw,
            height: dh * t,
        },

        // Two-thirds
        TilingLayout::LeftTwoThirds => Rect {
            x: dx,
            y: dy,
            width: dw * (1.0 - t),
            height: dh,
        },
        TilingLayout::RightTwoThirds => Rect {
            x: dx + dw * t,
            y: dy,
            width: dw * (1.0 - t),
            height: dh,
        },
        TilingLayout::TopTwoThirds => Rect {
            x: dx,
            y: dy,
            width: dw,
            height: dh * (1.0 - t),
        },
        TilingLayout::BottomTwoThirds => Rect {
            x: dx,
            y: dy + dh * t,
            width: dw,
            height: dh * (1.0 - t),
        },

        // Quarters
        TilingLayout::TopLeftQuarter => Rect {
            x: dx,
            y: dy,
            width: dw * h,
            height: dh * h,
        },
        TilingLayout::TopRightQuarter => Rect {
            x: dx + dw * h,
            y: dy,
            width: dw * (1.0 - h),
            height: dh * h,
        },
        TilingLayout::BottomLeftQuarter => Rect {
            x: dx,
            y: dy + dh * h,
            width: dw * h,
            height: dh * (1.0 - h),
        },
        TilingLayout::BottomRightQuarter => Rect {
            x: dx + dw * h,
            y: dy + dh * h,
            width: dw * (1.0 - h),
            height: dh * (1.0 - h),
        },

        // Maximize
        TilingLayout::Maximize => Rect {
            x: dx,
            y: dy,
            width: dw,
            height: dh,
        },
    }
}

/// Determine which display a window center point falls on.
pub(crate) fn find_display_for_window(rect: &Rect, displays: &[Rect]) -> usize {
    let cx = rect.cx();
    let cy = rect.cy();

    // Check which display contains the center point
    for (i, d) in displays.iter().enumerate() {
        if cx >= d.x && cx < d.x + d.width && cy >= d.y && cy < d.y + d.height {
            return i;
        }
    }

    // Fallback: closest display by center distance
    displays
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let da = (cx - a.cx()).powi(2) + (cy - a.cy()).powi(2);
            let db = (cx - b.cx()).powi(2) + (cy - b.cy()).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Info about an on-screen window (platform-independent subset).
pub(crate) struct WindowInfo {
    pub window_id: i64,
    pub owner_pid: i32,
    pub owner_name: String,
    pub bounds: Rect,
    /// Minimum size the window allows (if known). Used by expose to avoid undersized cells.
    pub min_size: Option<(f64, f64)>,
}

/// Build a deterministic, alphabetically-sorted window list from all windows.
/// Groups by app name (case-insensitive), sorts groups alphabetically,
/// sorts windows within each group by window_id for stability.
pub(crate) fn build_sorted_window_list(windows: &[WindowInfo], max: usize) -> Vec<&WindowInfo> {
    let mut app_groups: Vec<(String, Vec<&WindowInfo>)> = Vec::new();
    for w in windows {
        if let Some(group) = app_groups.iter_mut().find(|(name, _)| *name == w.owner_name) {
            group.1.push(w);
        } else {
            app_groups.push((w.owner_name.clone(), vec![w]));
        }
    }
    // Sort groups alphabetically by app name (case-insensitive)
    app_groups.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    // Sort windows within each group by window_id for deterministic order
    for (_, wins) in &mut app_groups {
        wins.sort_by_key(|w| w.window_id);
    }
    app_groups
        .iter()
        .flat_map(|(_, wins)| wins.iter().copied())
        .take(max)
        .collect()
}

/// Lay out windows in a grid on a single display. Returns the number of windows placed.
///
/// Uses an adaptive layout when some windows have a minimum size that exceeds
/// the default grid cell dimensions: normal-sized windows are placed first in a
/// standard grid, then oversized windows get rows with fewer columns so their
/// cells meet the minimum width requirement. The last window in an incomplete
/// oversized row is right-aligned to the grid's right edge.
pub(crate) fn layout_grid_on_display(
    ordered: &[&WindowInfo],
    display: &Rect,
    gap: f64,
    set_rect: &dyn Fn(&WindowInfo, &Rect),
) -> usize {
    let n = ordered.len();
    if n == 0 {
        return 0;
    }

    // Calculate the default grid dimensions
    let cols = (n as f64).sqrt().ceil() as usize;
    let rows = (n + cols - 1) / cols;
    let cell_w = (display.width - gap * (cols as f64 + 1.0)) / cols as f64;
    let cell_h = (display.height - gap * (rows as f64 + 1.0)) / rows as f64;

    // Partition: windows that fit in a normal cell vs those that don't
    let mut fits: Vec<&WindowInfo> = Vec::new();
    let mut oversized: Vec<&WindowInfo> = Vec::new();
    for &w in ordered {
        if let Some((min_w, min_h)) = w.min_size {
            if min_w > cell_w || min_h > cell_h {
                oversized.push(w);
                continue;
            }
        }
        fits.push(w);
    }

    // If nothing is oversized, use the simple grid for all
    if oversized.is_empty() {
        for (idx, win_info) in ordered.iter().enumerate() {
            let col = idx % cols;
            let row = idx / cols;
            let x = display.x + gap + col as f64 * (cell_w + gap);
            let y = display.y + gap + row as f64 * (cell_h + gap);
            set_rect(
                win_info,
                &Rect {
                    x,
                    y,
                    width: cell_w,
                    height: cell_h,
                },
            );
        }
        return n;
    }

    // Adaptive layout: normal windows first, then oversized in wider rows
    let total = fits.len() + oversized.len();
    // Recalculate: how many rows do normal windows need?
    let normal_cols = if fits.is_empty() {
        1
    } else {
        (fits.len() as f64).sqrt().ceil() as usize
    };
    let normal_rows = if fits.is_empty() {
        0
    } else {
        (fits.len() + normal_cols - 1) / normal_cols
    };

    // For oversized windows, figure out how many columns each row needs
    // by finding the max min_width among oversized windows
    let max_min_w = oversized
        .iter()
        .filter_map(|w| w.min_size.map(|(mw, _)| mw))
        .fold(0.0f64, f64::max);
    let oversized_cols = if max_min_w > 0.0 {
        ((display.width - gap) / (max_min_w + gap)).floor() as usize
    } else {
        normal_cols
    }
    .max(1);
    let oversized_rows = if oversized.is_empty() {
        0
    } else {
        (oversized.len() + oversized_cols - 1) / oversized_cols
    };

    let total_rows = normal_rows + oversized_rows;
    let row_h = (display.height - gap * (total_rows as f64 + 1.0)) / total_rows as f64;

    // Layout normal windows
    let normal_cell_w = if normal_cols > 0 {
        (display.width - gap * (normal_cols as f64 + 1.0)) / normal_cols as f64
    } else {
        0.0
    };

    for (idx, win_info) in fits.iter().enumerate() {
        let col = idx % normal_cols;
        let row = idx / normal_cols;
        let x = display.x + gap + col as f64 * (normal_cell_w + gap);
        let y = display.y + gap + row as f64 * (row_h + gap);
        set_rect(
            win_info,
            &Rect {
                x,
                y,
                width: normal_cell_w,
                height: row_h,
            },
        );
    }

    // Layout oversized windows
    let oversized_cell_w = if oversized_cols > 0 {
        (display.width - gap * (oversized_cols as f64 + 1.0)) / oversized_cols as f64
    } else {
        0.0
    };

    for (idx, win_info) in oversized.iter().enumerate() {
        let col = idx % oversized_cols;
        let row = normal_rows + idx / oversized_cols;
        let is_last_row = row == total_rows - 1;
        let items_in_this_row = if is_last_row {
            let remaining = oversized.len() - (row - normal_rows) * oversized_cols;
            remaining.min(oversized_cols)
        } else {
            oversized_cols
        };

        // Right-align the last window in an incomplete row
        let x = if is_last_row
            && col == items_in_this_row - 1
            && items_in_this_row < oversized_cols
        {
            // Right-align: position so right edge aligns with grid right edge
            display.x + display.width - gap - oversized_cell_w
        } else {
            display.x + gap + col as f64 * (oversized_cell_w + gap)
        };
        let y = display.y + gap + row as f64 * (row_h + gap);
        set_rect(
            win_info,
            &Rect {
                x,
                y,
                width: oversized_cell_w,
                height: row_h,
            },
        );
    }

    total
}

// ---------------------------------------------------------------------------
// Snap zone detection (shared geometry, used by macOS tile snap)
// ---------------------------------------------------------------------------

/// Detect which snap zone the cursor is in, if any.
/// Returns `(layout, display_index)` or `None`.
///
/// Two-pass approach: first checks displays whose bounds contain the cursor
/// (exact match), then checks displays where the cursor is slightly outside
/// (vertical overflow into menu bar/dock area). This prevents margin expansion
/// from stealing a cursor that belongs to an adjacent display.
pub(crate) fn detect_snap_zone(
    cx: f64,
    cy: f64,
    displays: &[Rect],
    side_trigger: f64,
    top_trigger: f64,
    corner_trigger: f64,
) -> Option<(TilingLayout, usize)> {
    let passes: &[bool] = &[false, true]; // false = exact only, true = with margin
    for &allow_overflow in passes {
        for (i, d) in displays.iter().enumerate() {
            let in_bounds =
                cx >= d.x && cx < d.x + d.width && cy >= d.y && cy < d.y + d.height;

            if !allow_overflow && !in_bounds {
                continue;
            }
            if allow_overflow && in_bounds {
                continue; // already checked in first pass
            }
            if allow_overflow {
                // Only allow vertical overflow (top/bottom — menu bar and dock).
                // Horizontal overflow would bleed into adjacent side-by-side displays.
                let v_margin = corner_trigger.max(top_trigger);
                if cx < d.x
                    || cx >= d.x + d.width
                    || cy < d.y - v_margin
                    || cy >= d.y + d.height + v_margin
                {
                    continue;
                }
            }

            // Clamp cursor vertically to display bounds — treats "above/below
            // the edge" the same as "at the edge" so snap zones extend through
            // the menu bar / dock. No horizontal clamping to avoid bleeding
            // into adjacent displays.
            let clamped_y = cy.clamp(d.y, d.y + d.height - 1.0);
            let left = cx - d.x;
            let right = d.x + d.width - cx;
            let top = clamped_y - d.y;
            let bottom = d.y + d.height - clamped_y;

            let at_left = left < side_trigger;
            let at_right = right < side_trigger;
            let at_top = top < top_trigger;
            let at_bottom = bottom < side_trigger;
            let in_corner_top = top < corner_trigger;
            let in_corner_bottom = bottom < corner_trigger;
            let in_corner_left = left < corner_trigger;
            let in_corner_right = right < corner_trigger;

            // Corners take priority (check first)
            if at_left && in_corner_top || at_top && in_corner_left {
                return Some((TilingLayout::TopLeftQuarter, i));
            }
            if at_right && in_corner_top || at_top && in_corner_right {
                return Some((TilingLayout::TopRightQuarter, i));
            }
            if at_left && in_corner_bottom || at_bottom && in_corner_left {
                return Some((TilingLayout::BottomLeftQuarter, i));
            }
            if at_right && in_corner_bottom || at_bottom && in_corner_right {
                return Some((TilingLayout::BottomRightQuarter, i));
            }

            // Edges
            if at_left {
                return Some((TilingLayout::LeftHalf, i));
            }
            if at_right {
                return Some((TilingLayout::RightHalf, i));
            }
            if at_top {
                return Some((TilingLayout::Maximize, i));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Platform-delegating public API
// ---------------------------------------------------------------------------

/// Tauri command: check if tiling is supported on this platform.
/// On Linux, checks at runtime whether X11 is available.
#[tauri::command]
pub fn get_tiling_supported() -> bool {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        true
    }
    #[cfg(target_os = "linux")]
    {
        linux::is_x11_available()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

/// Tauri command: check if accessibility/permissions are granted for tiling.
/// On Windows and Linux (X11), no special permission is needed — always returns true.
#[tauri::command]
pub fn get_accessibility_trusted() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::is_accessibility_trusted()
    }
    #[cfg(target_os = "windows")]
    {
        true // Win32 window management needs no special permissions
    }
    #[cfg(target_os = "linux")]
    {
        true // X11 allows any client to manipulate windows
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

/// Execute a tiling command on the focused window.
/// `layout_str` is a camelCase layout name (e.g. "leftHalf") or "restore".
pub fn execute_tile(app: &AppHandle, layout_str: &str) {
    #[cfg(target_os = "macos")]
    macos::execute_tile(app, layout_str);
    #[cfg(target_os = "windows")]
    windows::execute_tile(app, layout_str);
    #[cfg(target_os = "linux")]
    linux::execute_tile(app, layout_str);
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (app, layout_str);
        log::warn!("tiling: not supported on this platform");
    }
}

/// Execute the exposé command. Lays out all on-screen windows in a grid.
pub fn execute_expose(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    macos::execute_expose(app);
    #[cfg(target_os = "windows")]
    windows::execute_expose(app);
    #[cfg(target_os = "linux")]
    linux::execute_expose(app);
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = app;
        log::warn!("expose: not supported on this platform");
    }
}

/// Execute the app exposé command. Lays out the frontmost app's windows in a grid.
pub fn execute_expose_app(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    macos::execute_expose_app(app);
    #[cfg(target_os = "windows")]
    windows::execute_expose_app(app);
    #[cfg(target_os = "linux")]
    linux::execute_expose_app(app);
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = app;
        log::warn!("app_expose: not supported on this platform");
    }
}

/// Start tile snap (mouse edge snapping). Currently macOS only.
#[cfg(target_os = "macos")]
pub fn start_tile_snap(app: AppHandle) {
    macos::start_tile_snap(app);
}

// ---------------------------------------------------------------------------
// Tests (shared layout math — runs on all platforms)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn display(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.01
    }

    fn rect_approx(r: &Rect, x: f64, y: f64, w: f64, h: f64) -> bool {
        approx(r.x, x) && approx(r.y, y) && approx(r.width, w) && approx(r.height, h)
    }

    // -- TilingLayout::parse --

    #[test]
    fn test_vertical_two_thirds() {
        let d = display(0.0, 0.0, 1920.0, 900.0);
        let top = calculate_target_rect(TilingLayout::TopTwoThirds, &d, 50, 33, 0);
        let bot = calculate_target_rect(TilingLayout::BottomTwoThirds, &d, 50, 33, 0);
        assert!(rect_approx(&top, 0.0, 0.0, 1920.0, 603.0));
        assert!(rect_approx(&bot, 0.0, 297.0, 1920.0, 603.0));
    }

    #[test]
    fn test_parse_all_layouts() {
        assert_eq!(TilingLayout::parse("leftHalf"), Some(TilingLayout::LeftHalf));
        assert_eq!(TilingLayout::parse("rightHalf"), Some(TilingLayout::RightHalf));
        assert_eq!(TilingLayout::parse("topHalf"), Some(TilingLayout::TopHalf));
        assert_eq!(TilingLayout::parse("bottomHalf"), Some(TilingLayout::BottomHalf));
        assert_eq!(TilingLayout::parse("leftThird"), Some(TilingLayout::LeftThird));
        assert_eq!(TilingLayout::parse("centerThird"), Some(TilingLayout::CenterThird));
        assert_eq!(TilingLayout::parse("rightThird"), Some(TilingLayout::RightThird));
        assert_eq!(TilingLayout::parse("topThird"), Some(TilingLayout::TopThird));
        assert_eq!(TilingLayout::parse("middleThird"), Some(TilingLayout::MiddleThird));
        assert_eq!(TilingLayout::parse("bottomThird"), Some(TilingLayout::BottomThird));
        assert_eq!(TilingLayout::parse("leftTwoThirds"), Some(TilingLayout::LeftTwoThirds));
        assert_eq!(TilingLayout::parse("rightTwoThirds"), Some(TilingLayout::RightTwoThirds));
        assert_eq!(TilingLayout::parse("topTwoThirds"), Some(TilingLayout::TopTwoThirds));
        assert_eq!(TilingLayout::parse("bottomTwoThirds"), Some(TilingLayout::BottomTwoThirds));
        assert_eq!(TilingLayout::parse("topLeftQuarter"), Some(TilingLayout::TopLeftQuarter));
        assert_eq!(TilingLayout::parse("topRightQuarter"), Some(TilingLayout::TopRightQuarter));
        assert_eq!(TilingLayout::parse("bottomLeftQuarter"), Some(TilingLayout::BottomLeftQuarter));
        assert_eq!(TilingLayout::parse("bottomRightQuarter"), Some(TilingLayout::BottomRightQuarter));
        assert_eq!(TilingLayout::parse("maximize"), Some(TilingLayout::Maximize));
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(TilingLayout::parse(""), None);
        assert_eq!(TilingLayout::parse("unknown"), None);
        assert_eq!(TilingLayout::parse("LeftHalf"), None);
        assert_eq!(TilingLayout::parse("left_half"), None);
    }

    // -- Layout calculation: halves --

    #[test]
    fn test_left_half_default() {
        let d = display(0.0, 0.0, 1920.0, 1080.0);
        let r = calculate_target_rect(TilingLayout::LeftHalf, &d, 50, 33, 0);
        assert!(rect_approx(&r, 0.0, 0.0, 960.0, 1080.0));
    }

    #[test]
    fn test_right_half_default() {
        let d = display(0.0, 0.0, 1920.0, 1080.0);
        let r = calculate_target_rect(TilingLayout::RightHalf, &d, 50, 33, 0);
        assert!(rect_approx(&r, 960.0, 0.0, 960.0, 1080.0));
    }

    #[test]
    fn test_top_half_default() {
        let d = display(0.0, 0.0, 1920.0, 1080.0);
        let r = calculate_target_rect(TilingLayout::TopHalf, &d, 50, 33, 0);
        assert!(rect_approx(&r, 0.0, 0.0, 1920.0, 540.0));
    }

    #[test]
    fn test_bottom_half_default() {
        let d = display(0.0, 0.0, 1920.0, 1080.0);
        let r = calculate_target_rect(TilingLayout::BottomHalf, &d, 50, 33, 0);
        assert!(rect_approx(&r, 0.0, 540.0, 1920.0, 540.0));
    }

    #[test]
    fn test_halves_custom_ratio_60() {
        let d = display(0.0, 0.0, 1000.0, 800.0);
        let left = calculate_target_rect(TilingLayout::LeftHalf, &d, 60, 33, 0);
        let right = calculate_target_rect(TilingLayout::RightHalf, &d, 60, 33, 0);
        assert!(rect_approx(&left, 0.0, 0.0, 600.0, 800.0));
        assert!(rect_approx(&right, 600.0, 0.0, 400.0, 800.0));
    }

    // -- Layout calculation: thirds --

    #[test]
    fn test_thirds_default() {
        let d = display(0.0, 0.0, 900.0, 600.0);
        let left = calculate_target_rect(TilingLayout::LeftThird, &d, 50, 33, 0);
        let center = calculate_target_rect(TilingLayout::CenterThird, &d, 50, 33, 0);
        let right = calculate_target_rect(TilingLayout::RightThird, &d, 50, 33, 0);
        assert!(rect_approx(&left, 0.0, 0.0, 297.0, 600.0));
        assert!(rect_approx(&center, 297.0, 0.0, 306.0, 600.0));
        assert!(rect_approx(&right, 603.0, 0.0, 297.0, 600.0));
    }

    #[test]
    fn test_vertical_thirds() {
        let d = display(0.0, 0.0, 1920.0, 900.0);
        let top = calculate_target_rect(TilingLayout::TopThird, &d, 50, 33, 0);
        let mid = calculate_target_rect(TilingLayout::MiddleThird, &d, 50, 33, 0);
        let bot = calculate_target_rect(TilingLayout::BottomThird, &d, 50, 33, 0);
        assert!(rect_approx(&top, 0.0, 0.0, 1920.0, 297.0));
        assert!(rect_approx(&mid, 0.0, 297.0, 1920.0, 306.0));
        assert!(rect_approx(&bot, 0.0, 603.0, 1920.0, 297.0));
    }

    // -- Layout calculation: two-thirds --

    #[test]
    fn test_two_thirds() {
        let d = display(0.0, 0.0, 900.0, 600.0);
        let left = calculate_target_rect(TilingLayout::LeftTwoThirds, &d, 50, 33, 0);
        let right = calculate_target_rect(TilingLayout::RightTwoThirds, &d, 50, 33, 0);
        assert!(rect_approx(&left, 0.0, 0.0, 603.0, 600.0));
        assert!(rect_approx(&right, 297.0, 0.0, 603.0, 600.0));
    }

    // -- Layout calculation: quarters --

    #[test]
    fn test_quarters_default() {
        let d = display(0.0, 0.0, 1000.0, 800.0);
        let tl = calculate_target_rect(TilingLayout::TopLeftQuarter, &d, 50, 33, 0);
        let tr = calculate_target_rect(TilingLayout::TopRightQuarter, &d, 50, 33, 0);
        let bl = calculate_target_rect(TilingLayout::BottomLeftQuarter, &d, 50, 33, 0);
        let br = calculate_target_rect(TilingLayout::BottomRightQuarter, &d, 50, 33, 0);
        assert!(rect_approx(&tl, 0.0, 0.0, 500.0, 400.0));
        assert!(rect_approx(&tr, 500.0, 0.0, 500.0, 400.0));
        assert!(rect_approx(&bl, 0.0, 400.0, 500.0, 400.0));
        assert!(rect_approx(&br, 500.0, 400.0, 500.0, 400.0));
    }

    // -- Layout calculation: maximize --

    #[test]
    fn test_maximize() {
        let d = display(100.0, 50.0, 1920.0, 1080.0);
        let r = calculate_target_rect(TilingLayout::Maximize, &d, 50, 33, 0);
        assert!(rect_approx(&r, 100.0, 50.0, 1920.0, 1080.0));
    }

    // -- Gap --

    #[test]
    fn test_gap() {
        let d = display(0.0, 0.0, 1000.0, 800.0);
        let r = calculate_target_rect(TilingLayout::Maximize, &d, 50, 33, 10);
        assert!(rect_approx(&r, 10.0, 10.0, 980.0, 780.0));
    }

    #[test]
    fn test_gap_left_half() {
        let d = display(0.0, 0.0, 1000.0, 800.0);
        let r = calculate_target_rect(TilingLayout::LeftHalf, &d, 50, 33, 10);
        assert!(rect_approx(&r, 10.0, 10.0, 490.0, 780.0));
    }

    // -- Multi-monitor offset --

    #[test]
    fn test_layout_with_display_offset() {
        let d = display(1920.0, 0.0, 1920.0, 1080.0);
        let r = calculate_target_rect(TilingLayout::LeftHalf, &d, 50, 33, 0);
        assert!(rect_approx(&r, 1920.0, 0.0, 960.0, 1080.0));
    }

    // -- find_display_for_window --

    #[test]
    fn test_find_display_single() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let win = Rect {
            x: 100.0,
            y: 200.0,
            width: 800.0,
            height: 600.0,
        };
        assert_eq!(find_display_for_window(&win, &displays), 0);
    }

    #[test]
    fn test_find_display_dual_monitor() {
        let displays = vec![
            display(0.0, 0.0, 1920.0, 1080.0),
            display(1920.0, 0.0, 2560.0, 1440.0),
        ];
        let win1 = Rect {
            x: 100.0,
            y: 100.0,
            width: 800.0,
            height: 600.0,
        };
        assert_eq!(find_display_for_window(&win1, &displays), 0);

        let win2 = Rect {
            x: 2000.0,
            y: 100.0,
            width: 800.0,
            height: 600.0,
        };
        assert_eq!(find_display_for_window(&win2, &displays), 1);
    }

    // -- TilingState --

    #[test]
    fn test_tiling_state_new() {
        let state = TilingState::new();
        assert!(state.windows.is_empty());
    }

    // -- Snap zone detection --

    #[test]
    fn test_snap_left_edge() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone(2.0, 540.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::LeftHalf, 0)));
    }

    #[test]
    fn test_snap_right_edge() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone(1918.0, 540.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::RightHalf, 0)));
    }

    #[test]
    fn test_snap_top_edge() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone(960.0, 2.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::Maximize, 0)));
    }

    #[test]
    fn test_snap_top_left_corner() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone(2.0, 20.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::TopLeftQuarter, 0)));
    }

    #[test]
    fn test_snap_top_right_corner() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone(1918.0, 20.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::TopRightQuarter, 0)));
    }

    #[test]
    fn test_snap_bottom_left_corner() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone(2.0, 1060.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::BottomLeftQuarter, 0)));
    }

    #[test]
    fn test_snap_bottom_right_corner() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone(1918.0, 1060.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::BottomRightQuarter, 0)));
    }

    #[test]
    fn test_snap_no_zone_center() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone(960.0, 540.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_snap_second_monitor() {
        let displays = vec![
            display(0.0, 0.0, 1920.0, 1080.0),
            display(1920.0, 0.0, 2560.0, 1440.0),
        ];
        let result = detect_snap_zone(1922.0, 540.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::LeftHalf, 1)));
    }

    #[test]
    fn test_snap_cursor_above_display_into_menu_bar() {
        let displays = vec![display(0.0, 25.0, 1920.0, 1055.0)];
        let result = detect_snap_zone(960.0, 20.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::Maximize, 0)));
    }

    #[test]
    fn test_snap_no_horizontal_overflow() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone(-3.0, 540.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, None);
    }

    // -- build_sorted_window_list --

    fn win(id: i64, name: &str) -> WindowInfo {
        WindowInfo {
            window_id: id,
            owner_pid: 1,
            owner_name: name.to_string(),
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            min_size: None,
        }
    }

    #[test]
    fn test_build_sorted_window_list_empty() {
        let windows: Vec<WindowInfo> = vec![];
        let result = build_sorted_window_list(&windows, 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_build_sorted_window_list_alphabetical() {
        let windows = vec![win(1, "Zoom"), win(2, "Firefox"), win(3, "Alacritty")];
        let result = build_sorted_window_list(&windows, 10);
        assert_eq!(result[0].owner_name, "Alacritty");
        assert_eq!(result[1].owner_name, "Firefox");
        assert_eq!(result[2].owner_name, "Zoom");
    }

    #[test]
    fn test_build_sorted_window_list_case_insensitive() {
        let windows = vec![win(1, "zoom"), win(2, "Firefox"), win(3, "ALACRITTY")];
        let result = build_sorted_window_list(&windows, 10);
        assert_eq!(result[0].owner_name, "ALACRITTY");
        assert_eq!(result[1].owner_name, "Firefox");
        assert_eq!(result[2].owner_name, "zoom");
    }

    #[test]
    fn test_build_sorted_window_list_max_truncates() {
        let windows = vec![
            win(1, "App1"),
            win(2, "App2"),
            win(3, "App3"),
            win(4, "App4"),
        ];
        let result = build_sorted_window_list(&windows, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_build_sorted_window_list_same_app_sorted_by_id() {
        let windows = vec![win(5, "Firefox"), win(2, "Firefox"), win(9, "Firefox")];
        let result = build_sorted_window_list(&windows, 10);
        assert_eq!(result[0].window_id, 2);
        assert_eq!(result[1].window_id, 5);
        assert_eq!(result[2].window_id, 9);
    }

    #[test]
    fn test_build_sorted_window_list_groups_then_sorts() {
        let windows = vec![
            win(3, "Firefox"),
            win(1, "Alacritty"),
            win(4, "Firefox"),
            win(2, "Alacritty"),
        ];
        let result = build_sorted_window_list(&windows, 10);
        // Alacritty group first (alphabetically), sorted by id
        assert_eq!(result[0].window_id, 1);
        assert_eq!(result[1].window_id, 2);
        // Firefox group second
        assert_eq!(result[2].window_id, 3);
        assert_eq!(result[3].window_id, 4);
    }

    // -- layout_grid_on_display --

    #[test]
    fn test_layout_grid_empty() {
        let d = display(0.0, 0.0, 1920.0, 1080.0);
        let ordered: Vec<&WindowInfo> = vec![];
        let count = layout_grid_on_display(&ordered, &d, 10.0, &|_, _| {});
        assert_eq!(count, 0);
    }

    #[test]
    fn test_layout_grid_single_window() {
        let w = win(1, "App");
        let ordered: Vec<&WindowInfo> = vec![&w];
        let d = display(0.0, 0.0, 1000.0, 800.0);
        let rects = std::cell::RefCell::new(Vec::new());
        layout_grid_on_display(&ordered, &d, 10.0, &|_, rect| {
            rects.borrow_mut().push(rect.clone());
        });
        let rects = rects.into_inner();
        assert_eq!(rects.len(), 1);
        // Single window should fill the display minus gaps
        assert!(rects[0].width > 900.0);
        assert!(rects[0].height > 700.0);
    }

    #[test]
    fn test_layout_grid_four_windows() {
        let ws = vec![win(1, "A"), win(2, "B"), win(3, "C"), win(4, "D")];
        let ordered: Vec<&WindowInfo> = ws.iter().collect();
        let d = display(0.0, 0.0, 1000.0, 800.0);
        let rects = std::cell::RefCell::new(Vec::new());
        let count = layout_grid_on_display(&ordered, &d, 0.0, &|_, rect| {
            rects.borrow_mut().push(rect.clone());
        });
        let rects = rects.into_inner();
        assert_eq!(count, 4);
        assert_eq!(rects.len(), 4);
        // 2x2 grid, each cell ~500x400 with gap=0
        for r in &rects {
            assert!((r.width - 500.0).abs() < 1.0);
            assert!((r.height - 400.0).abs() < 1.0);
        }
    }

    #[test]
    fn test_layout_grid_with_gap() {
        let ws = vec![win(1, "A")];
        let ordered: Vec<&WindowInfo> = ws.iter().collect();
        let d = display(0.0, 0.0, 1000.0, 800.0);
        let rects = std::cell::RefCell::new(Vec::new());
        layout_grid_on_display(&ordered, &d, 20.0, &|_, rect| {
            rects.borrow_mut().push(rect.clone());
        });
        let rects = rects.into_inner();
        assert_eq!(rects.len(), 1);
        // gap=20 on all sides: x=20, y=20, w=960, h=760
        assert!((rects[0].x - 20.0).abs() < 1.0);
        assert!((rects[0].y - 20.0).abs() < 1.0);
        assert!((rects[0].width - 960.0).abs() < 1.0);
        assert!((rects[0].height - 760.0).abs() < 1.0);
    }

    // -- find_display_for_window edge cases --

    #[test]
    fn test_find_display_window_outside_all_displays() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let win = Rect {
            x: 5000.0,
            y: 5000.0,
            width: 100.0,
            height: 100.0,
        };
        // Should fall back to closest display (index 0)
        assert_eq!(find_display_for_window(&win, &displays), 0);
    }

    #[test]
    fn test_find_display_three_monitors() {
        let displays = vec![
            display(0.0, 0.0, 1920.0, 1080.0),
            display(1920.0, 0.0, 1920.0, 1080.0),
            display(3840.0, 0.0, 1920.0, 1080.0),
        ];
        let win = Rect {
            x: 4000.0,
            y: 100.0,
            width: 800.0,
            height: 600.0,
        };
        assert_eq!(find_display_for_window(&win, &displays), 2);
    }

    // -- calculate_target_rect edge cases --

    #[test]
    fn test_layout_zero_gap() {
        let d = display(0.0, 0.0, 1920.0, 1080.0);
        let r = calculate_target_rect(TilingLayout::LeftHalf, &d, 50, 33, 0);
        assert!(rect_approx(&r, 0.0, 0.0, 960.0, 1080.0));
    }

    #[test]
    fn test_layout_large_gap() {
        let d = display(0.0, 0.0, 1000.0, 800.0);
        // gap=100 means 200px lost on each axis
        let r = calculate_target_rect(TilingLayout::Maximize, &d, 50, 33, 100);
        assert!(rect_approx(&r, 100.0, 100.0, 800.0, 600.0));
    }

    #[test]
    fn test_layout_half_ratio_extremes() {
        let d = display(0.0, 0.0, 1000.0, 800.0);
        // 0% ratio — left half has 0 width
        let r0 = calculate_target_rect(TilingLayout::LeftHalf, &d, 0, 33, 0);
        assert!(rect_approx(&r0, 0.0, 0.0, 0.0, 800.0));
        // 100% ratio — left half takes full width
        let r100 = calculate_target_rect(TilingLayout::LeftHalf, &d, 100, 33, 0);
        assert!(rect_approx(&r100, 0.0, 0.0, 1000.0, 800.0));
    }

    #[test]
    fn test_layout_small_display() {
        let d = display(0.0, 0.0, 100.0, 80.0);
        let r = calculate_target_rect(TilingLayout::TopLeftQuarter, &d, 50, 33, 0);
        assert!(rect_approx(&r, 0.0, 0.0, 50.0, 40.0));
    }

    #[test]
    fn test_layout_fractional_display_offset() {
        let d = display(0.5, 0.5, 1920.0, 1080.0);
        let r = calculate_target_rect(TilingLayout::Maximize, &d, 50, 33, 0);
        assert!(approx(r.x, 0.5));
        assert!(approx(r.y, 0.5));
    }
}
