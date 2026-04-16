//! macOS window tiling via the Accessibility API.
//!
//! Provides window tiling commands (halves, thirds, quarters, maximize, restore)
//! for macOS. Uses AXUIElement to get the focused window and move/resize it,
//! and NSScreen to get display visible frames (accounting for menu bar and dock).
//!
//! Requires the user to grant Accessibility permission in
//! System Settings > Privacy & Security > Accessibility.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CString};
use tauri::{AppHandle, Manager};

// ---------------------------------------------------------------------------
// CoreFoundation FFI
// ---------------------------------------------------------------------------

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFAllocatorRef = *const c_void;

const K_CF_ALLOCATOR_DEFAULT: CFAllocatorRef = std::ptr::null();
const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
}

/// RAII wrapper for CoreFoundation objects. Calls CFRelease on drop.
struct CfRef(CFTypeRef);

impl CfRef {
    /// Wrap a CF pointer, returning None if null.
    fn new(ptr: CFTypeRef) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self(ptr))
        }
    }

    /// Return the raw pointer.
    fn as_ptr(&self) -> CFTypeRef {
        self.0
    }
}

impl Drop for CfRef {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0);
        }
    }
}

/// Create a CFString from a Rust &str. Returns a CfRef that auto-releases.
unsafe fn cfstr(s: &str) -> Option<CfRef> {
    let c = CString::new(s).ok()?;
    let ptr = CFStringCreateWithCString(K_CF_ALLOCATOR_DEFAULT, c.as_ptr(), K_CF_STRING_ENCODING_UTF8);
    CfRef::new(ptr)
}

// ---------------------------------------------------------------------------
// Accessibility API FFI
// ---------------------------------------------------------------------------

type AXError = i32;

const K_AX_ERROR_SUCCESS: AXError = 0;
const K_AX_VALUE_TYPE_CG_POINT: u32 = 1;
const K_AX_VALUE_TYPE_CG_SIZE: u32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateSystemWide() -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXValueCreate(value_type: u32, value: *const c_void) -> CFTypeRef;
    fn AXValueGetValue(value: CFTypeRef, value_type: u32, value_out: *mut c_void) -> bool;
    /// Private API: bridges AXUIElement to CGWindowID.
    /// Used by AeroSpace, Rectangle, and other tiling WMs.
    /// Available since macOS 10.6, confirmed working through macOS 26.
    fn _AXUIElementGetWindow(element: CFTypeRef, window_id: *mut u32) -> AXError;
}

// ---------------------------------------------------------------------------
// Geometry types (compatible with CoreGraphics C structs)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

/// CGRect used for NSScreen frame/visibleFrame via objc msg_send.
#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

unsafe impl objc::Encode for CGPoint {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{CGPoint=dd}") }
    }
}

unsafe impl objc::Encode for CGSize {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{CGSize=dd}") }
    }
}

unsafe impl objc::Encode for CGRect {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}") }
    }
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A rectangle in screen coordinates (top-left origin, points — matches AX API).
#[derive(Clone, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    /// Center X coordinate.
    fn cx(&self) -> f64 {
        self.x + self.width / 2.0
    }
    /// Center Y coordinate.
    fn cy(&self) -> f64 {
        self.y + self.height / 2.0
    }
}

/// One of the 17 supported tiling layouts.
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
struct WindowState {
    /// Original position/size before first tile (for restore).
    original: Rect,
    /// Current tiling layout applied to this window.
    layout: TilingLayout,
    /// Display index the window is currently tiled on.
    display_index: usize,
}

/// Runtime tiling state, keyed by CGWindowID.
pub struct TilingState {
    windows: HashMap<u32, WindowState>,
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
// Display enumeration (NSScreen visible frames)
// ---------------------------------------------------------------------------

/// Get visible frames for all displays in AX/CoreGraphics coordinates
/// (top-left origin, in points). Accounts for menu bar and dock.
/// Returns displays sorted left-to-right, then top-to-bottom.
#[allow(unexpected_cfgs)]
fn get_display_visible_frames() -> Vec<Rect> {
    unsafe {
        use objc::{msg_send, sel, sel_impl};
        use objc::runtime::Object;

        let cls = match objc::runtime::Class::get("NSScreen") {
            Some(c) => c,
            None => return Vec::new(),
        };

        // Primary screen height for Cocoa → CG coordinate conversion
        let main_screen: *mut Object = msg_send![cls, mainScreen];
        if main_screen.is_null() {
            return Vec::new();
        }
        let main_frame: CGRect = msg_send![main_screen, frame];
        let primary_h = main_frame.size.height;

        let screens: *mut Object = msg_send![cls, screens];
        if screens.is_null() {
            return Vec::new();
        }
        let count: usize = msg_send![screens, count];

        let mut frames = Vec::with_capacity(count);
        for i in 0..count {
            let screen: *mut Object = msg_send![screens, objectAtIndex: i];
            // visibleFrame: Cocoa coords (origin at bottom-left, Y up)
            let vis: CGRect = msg_send![screen, visibleFrame];
            // Convert to CG/AX coords: ax_y = primary_height - cocoa_y - height
            frames.push(Rect {
                x: vis.origin.x,
                y: primary_h - vis.origin.y - vis.size.height,
                width: vis.size.width,
                height: vis.size.height,
            });
        }

        // Sort left-to-right, then top-to-bottom
        frames.sort_by(|a, b| {
            a.x.partial_cmp(&b.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        });

        frames
    }
}

// ---------------------------------------------------------------------------
// Accessibility API helpers
// ---------------------------------------------------------------------------

/// Get the AXUIElementRef for the currently focused window.
unsafe fn get_focused_window() -> Option<CfRef> {
    let system = CfRef::new(AXUIElementCreateSystemWide())?;

    // Focused application
    let attr = cfstr("AXFocusedApplication")?;
    let mut app_ref: CFTypeRef = std::ptr::null();
    let err = AXUIElementCopyAttributeValue(system.as_ptr(), attr.as_ptr(), &mut app_ref);
    if err != K_AX_ERROR_SUCCESS {
        return None;
    }
    let app = CfRef::new(app_ref)?;

    // Focused window of that application
    let attr = cfstr("AXFocusedWindow")?;
    let mut win_ref: CFTypeRef = std::ptr::null();
    let err = AXUIElementCopyAttributeValue(app.as_ptr(), attr.as_ptr(), &mut win_ref);
    if err != K_AX_ERROR_SUCCESS {
        return None;
    }
    CfRef::new(win_ref)
}

/// Get the CGWindowID for an AXUIElement window (uses private API).
unsafe fn get_window_id(window: &CfRef) -> Option<u32> {
    let mut wid: u32 = 0;
    if _AXUIElementGetWindow(window.as_ptr(), &mut wid) == K_AX_ERROR_SUCCESS && wid != 0 {
        Some(wid)
    } else {
        None
    }
}

/// Read the current position and size of a window.
unsafe fn get_window_rect(window: &CfRef) -> Option<Rect> {
    // Position
    let attr = cfstr("AXPosition")?;
    let mut val: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(window.as_ptr(), attr.as_ptr(), &mut val) != K_AX_ERROR_SUCCESS
    {
        return None;
    }
    let val_ref = CfRef::new(val)?;
    let mut point = CGPoint { x: 0.0, y: 0.0 };
    if !AXValueGetValue(
        val_ref.as_ptr(),
        K_AX_VALUE_TYPE_CG_POINT,
        &mut point as *mut CGPoint as *mut c_void,
    ) {
        return None;
    }

    // Size
    let attr = cfstr("AXSize")?;
    let mut val: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(window.as_ptr(), attr.as_ptr(), &mut val) != K_AX_ERROR_SUCCESS
    {
        return None;
    }
    let val_ref = CfRef::new(val)?;
    let mut size = CGSize {
        width: 0.0,
        height: 0.0,
    };
    if !AXValueGetValue(
        val_ref.as_ptr(),
        K_AX_VALUE_TYPE_CG_SIZE,
        &mut size as *mut CGSize as *mut c_void,
    ) {
        return None;
    }

    Some(Rect {
        x: point.x,
        y: point.y,
        width: size.width,
        height: size.height,
    })
}

/// Move a window to the given position (in points).
unsafe fn set_window_position(window: &CfRef, x: f64, y: f64) -> bool {
    let mut point = CGPoint { x, y };
    let value = AXValueCreate(
        K_AX_VALUE_TYPE_CG_POINT,
        &mut point as *mut CGPoint as *mut c_void,
    );
    let val_ref = match CfRef::new(value) {
        Some(v) => v,
        None => return false,
    };
    let attr = match cfstr("AXPosition") {
        Some(a) => a,
        None => return false,
    };
    AXUIElementSetAttributeValue(window.as_ptr(), attr.as_ptr(), val_ref.as_ptr())
        == K_AX_ERROR_SUCCESS
}

/// Resize a window to the given dimensions (in points).
unsafe fn set_window_size(window: &CfRef, w: f64, h: f64) -> bool {
    let mut size = CGSize { width: w, height: h };
    let value = AXValueCreate(
        K_AX_VALUE_TYPE_CG_SIZE,
        &mut size as *mut CGSize as *mut c_void,
    );
    let val_ref = match CfRef::new(value) {
        Some(v) => v,
        None => return false,
    };
    let attr = match cfstr("AXSize") {
        Some(a) => a,
        None => return false,
    };
    AXUIElementSetAttributeValue(window.as_ptr(), attr.as_ptr(), val_ref.as_ptr())
        == K_AX_ERROR_SUCCESS
}

/// Move and resize a window. Sets position, then size, then position again
/// (some apps adjust position after resize).
unsafe fn set_window_rect(window: &CfRef, rect: &Rect) {
    set_window_position(window, rect.x, rect.y);
    set_window_size(window, rect.width, rect.height);
    // Re-set position: some apps shift after resize
    set_window_position(window, rect.x, rect.y);
}

// ---------------------------------------------------------------------------
// Layout calculation
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
fn find_display_for_window(rect: &Rect, displays: &[Rect]) -> usize {
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

    // Check accessibility permission
    if unsafe { !AXIsProcessTrusted() } {
        log::warn!(
            "tiling: Accessibility permission not granted. \
             Go to System Settings > Privacy & Security > Accessibility and add this app."
        );
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

    // Get focused window info (no locks held during AX calls)
    let (window, window_id, win_rect) = unsafe {
        let window = match get_focused_window() {
            Some(w) => w,
            None => {
                log::info!("tiling: no focused window");
                return;
            }
        };
        let wid = match get_window_id(&window) {
            Some(id) => id,
            None => {
                log::info!("tiling: could not get window ID");
                return;
            }
        };
        let rect = match get_window_rect(&window) {
            Some(r) => r,
            None => {
                log::info!("tiling: could not get window rect");
                return;
            }
        };
        (window, wid, rect)
    };

    // Get displays
    let displays = get_display_visible_frames();
    if displays.is_empty() {
        log::warn!("tiling: no displays found");
        return;
    }

    let current_display = find_display_for_window(&win_rect, &displays);

    // Determine target display (cycle if same layout already applied)
    let target_display = {
        let state = app.state::<crate::AppState>();
        let ts = state.tiling_state.lock().unwrap();
        if let Some(ws) = ts.windows.get(&window_id) {
            if ws.layout == layout {
                // Same layout — cycle to next display
                (ws.display_index + 1) % displays.len()
            } else {
                current_display
            }
        } else {
            current_display
        }
    };

    // Save original position (only on first tile) and update state
    {
        let state = app.state::<crate::AppState>();
        let mut ts = state.tiling_state.lock().unwrap();
        let entry = ts.windows.entry(window_id).or_insert(WindowState {
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
    unsafe {
        set_window_rect(&window, &target);
    }
}

/// Restore the focused window to its pre-tiled position and size.
fn execute_restore(app: &AppHandle) {
    if unsafe { !AXIsProcessTrusted() } {
        return;
    }

    let window = unsafe {
        match get_focused_window() {
            Some(w) => w,
            None => return,
        }
    };

    let window_id = unsafe {
        match get_window_id(&window) {
            Some(id) => id,
            None => return,
        }
    };

    // Remove state and get original rect
    let original = {
        let state = app.state::<crate::AppState>();
        let mut ts = state.tiling_state.lock().unwrap();
        ts.windows.remove(&window_id).map(|ws| ws.original)
    };

    if let Some(rect) = original {
        log::info!(
            "tiling: restore -> ({}, {}, {}x{})",
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        );
        unsafe {
            set_window_rect(&window, &rect);
        }
    } else {
        log::info!("tiling: no saved state to restore");
    }
}

// ---------------------------------------------------------------------------
// Tests
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
        assert_eq!(
            TilingLayout::parse("topLeftQuarter"),
            Some(TilingLayout::TopLeftQuarter)
        );
        assert_eq!(
            TilingLayout::parse("topRightQuarter"),
            Some(TilingLayout::TopRightQuarter)
        );
        assert_eq!(
            TilingLayout::parse("bottomLeftQuarter"),
            Some(TilingLayout::BottomLeftQuarter)
        );
        assert_eq!(
            TilingLayout::parse("bottomRightQuarter"),
            Some(TilingLayout::BottomRightQuarter)
        );
        assert_eq!(TilingLayout::parse("maximize"), Some(TilingLayout::Maximize));
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(TilingLayout::parse(""), None);
        assert_eq!(TilingLayout::parse("unknown"), None);
        assert_eq!(TilingLayout::parse("LeftHalf"), None); // wrong case
        assert_eq!(TilingLayout::parse("left_half"), None); // snake_case
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
        // left two-thirds = 1.0 - 0.33 = 0.67 => 603
        assert!(rect_approx(&left, 0.0, 0.0, 603.0, 600.0));
        // right two-thirds starts at third = 0.33 => 297
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
        // gap=10 shrinks by 10 on each side
        assert!(rect_approx(&r, 10.0, 10.0, 980.0, 780.0));
    }

    #[test]
    fn test_gap_left_half() {
        let d = display(0.0, 0.0, 1000.0, 800.0);
        let r = calculate_target_rect(TilingLayout::LeftHalf, &d, 50, 33, 10);
        // usable area: x=10, w=980, h=780
        // left half: 50% of 980 = 490
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
        // Window on first display
        let win1 = Rect {
            x: 100.0,
            y: 100.0,
            width: 800.0,
            height: 600.0,
        };
        assert_eq!(find_display_for_window(&win1, &displays), 0);

        // Window on second display
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
}
