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

        let screens: *mut Object = msg_send![cls, screens];
        if screens.is_null() {
            return Vec::new();
        }
        let count: usize = msg_send![screens, count];
        if count == 0 {
            return Vec::new();
        }

        // Primary screen height for Cocoa → CG coordinate conversion.
        // MUST use screens[0] (the primary display with the menu bar), NOT
        // mainScreen which returns whichever screen has keyboard focus.
        // The Cocoa coordinate system origin is anchored to the primary display —
        // using the wrong screen's height shifts all Y coordinates by the
        // height difference between the focused and primary screens.
        let primary_screen: *mut Object = msg_send![screens, objectAtIndex: 0usize];
        let primary_frame: CGRect = msg_send![primary_screen, frame];
        let primary_h = primary_frame.size.height;

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
// Accessibility permission check
// ---------------------------------------------------------------------------

/// Returns true if the app has macOS Accessibility permission.
pub fn is_accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Tauri command: check if tiling is supported on this platform.
/// Returns true only on macOS (tiling not yet implemented on other platforms).
#[tauri::command]
pub fn get_tiling_supported() -> bool {
    true // This module is only compiled on macOS
}

/// Tauri command: check if macOS Accessibility permission is granted.
#[tauri::command]
pub fn get_accessibility_trusted() -> bool {
    is_accessibility_trusted()
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

    // Always tile on the display the window is currently on
    let target_display = find_display_for_window(&win_rect, &displays);

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

// ===========================================================================
// Exposé — lay out all windows in a grid for overview
// ===========================================================================

// --- CGWindowList FFI ---

type CFArrayRef = *const c_void;
type CFDictionaryRef = *const c_void;

const K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1;
const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
const K_CG_NULL_WINDOW_ID: u32 = 0;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> CFArrayRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFArrayGetCount(array: CFArrayRef) -> i64;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, idx: i64) -> *const c_void;
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: *const c_void) -> *const c_void;
    fn CFNumberGetValue(number: *const c_void, typ: i64, out: *mut c_void) -> bool;
    fn CFStringGetCStringPtr(string: *const c_void, encoding: u32) -> *const c_char;
    fn CFStringGetCString(
        string: *const c_void,
        buffer: *mut c_char,
        buffer_size: i64,
        encoding: u32,
    ) -> bool;
}

const K_CF_NUMBER_SINT32_TYPE: i64 = 3;
const K_CF_NUMBER_SINT64_TYPE: i64 = 4;

/// Info about an on-screen window from CGWindowList.
#[derive(Debug)]
struct WindowInfo {
    window_id: u32,
    owner_pid: i32,
    owner_name: String,
    layer: i32,
    bounds: Rect,
}

/// Read a CFString value from a CFDictionary.
unsafe fn dict_get_string(dict: CFDictionaryRef, key_str: &str) -> Option<String> {
    let key = cfstr(key_str)?;
    let val = CFDictionaryGetValue(dict, key.as_ptr());
    if val.is_null() {
        return None;
    }
    // Try fast path (direct pointer)
    let ptr = CFStringGetCStringPtr(val, K_CF_STRING_ENCODING_UTF8);
    if !ptr.is_null() {
        return Some(std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned());
    }
    // Slow path (copy to buffer)
    let mut buf = [0i8; 512];
    if CFStringGetCString(val, buf.as_mut_ptr(), 512, K_CF_STRING_ENCODING_UTF8) {
        Some(
            std::ffi::CStr::from_ptr(buf.as_ptr())
                .to_string_lossy()
                .into_owned(),
        )
    } else {
        None
    }
}

/// Read an i32 from a CFNumber in a CFDictionary.
unsafe fn dict_get_i32(dict: CFDictionaryRef, key_str: &str) -> Option<i32> {
    let key = cfstr(key_str)?;
    let val = CFDictionaryGetValue(dict, key.as_ptr());
    if val.is_null() {
        return None;
    }
    let mut out: i32 = 0;
    if CFNumberGetValue(val, K_CF_NUMBER_SINT32_TYPE, &mut out as *mut i32 as *mut c_void) {
        Some(out)
    } else {
        None
    }
}

/// Read an i64 from a CFNumber in a CFDictionary.
unsafe fn dict_get_i64(dict: CFDictionaryRef, key_str: &str) -> Option<i64> {
    let key = cfstr(key_str)?;
    let val = CFDictionaryGetValue(dict, key.as_ptr());
    if val.is_null() {
        return None;
    }
    let mut out: i64 = 0;
    if CFNumberGetValue(val, K_CF_NUMBER_SINT64_TYPE, &mut out as *mut i64 as *mut c_void) {
        Some(out)
    } else {
        None
    }
}

/// Read the bounds dict (X, Y, Width, Height) from a CGWindowList entry.
unsafe fn dict_get_bounds(dict: CFDictionaryRef) -> Option<Rect> {
    let key = cfstr("kCGWindowBounds")?;
    let bounds_dict = CFDictionaryGetValue(dict, key.as_ptr());
    if bounds_dict.is_null() {
        return None;
    }
    let x = dict_get_i32(bounds_dict, "X")? as f64;
    let y = dict_get_i32(bounds_dict, "Y")? as f64;
    let w = dict_get_i32(bounds_dict, "Width")? as f64;
    let h = dict_get_i32(bounds_dict, "Height")? as f64;
    Some(Rect {
        x,
        y,
        width: w,
        height: h,
    })
}

/// Get all normal windows via CGWindowList (including minimized and other-space windows).
/// Filters out desktop elements, tiny windows (< 50x50), and non-normal layers.
fn get_all_windows() -> Vec<WindowInfo> {
    let mut windows = Vec::new();
    unsafe {
        let list = CGWindowListCopyWindowInfo(
            K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS,
            K_CG_NULL_WINDOW_ID,
        );
        if list.is_null() {
            return windows;
        }
        let count = CFArrayGetCount(list);
        for i in 0..count {
            let dict = CFArrayGetValueAtIndex(list, i);
            if dict.is_null() {
                continue;
            }

            let layer = dict_get_i32(dict, "kCGWindowLayer").unwrap_or(-1);
            // Only normal windows (layer 0)
            if layer != 0 {
                continue;
            }

            let wid = dict_get_i32(dict, "kCGWindowNumber").unwrap_or(0) as u32;
            let pid = dict_get_i32(dict, "kCGWindowOwnerPID").unwrap_or(0);
            let owner = dict_get_string(dict, "kCGWindowOwnerName").unwrap_or_default();
            let bounds = match dict_get_bounds(dict) {
                Some(b) => b,
                None => continue,
            };

            // Skip tiny windows (toolbars, status items, etc.)
            if bounds.width < 50.0 || bounds.height < 50.0 {
                continue;
            }

            windows.push(WindowInfo {
                window_id: wid,
                owner_pid: pid,
                owner_name: owner,
                layer,
                bounds,
            });
        }
        CFRelease(list);
    }
    windows
}

// --- Exposé state ---

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether exposé is currently active (windows are spread out).
static EXPOSE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Saved positions of all windows before exposé, keyed by CGWindowID.
static EXPOSE_SAVED: std::sync::OnceLock<std::sync::Mutex<HashMap<u32, Rect>>> =
    std::sync::OnceLock::new();

fn expose_saved() -> &'static std::sync::Mutex<HashMap<u32, Rect>> {
    EXPOSE_SAVED.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Execute the exposé command. Toggles between spread and restore.
pub fn execute_expose(app: &AppHandle) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("expose: Accessibility permission not granted");
        return;
    }

    // Toggle: if active, restore all; if inactive, spread all
    if EXPOSE_ACTIVE.load(Ordering::Relaxed) {
        restore_expose(app);
    } else {
        spread_expose(app);
    }
}

/// Spread all windows into a grid on the current display.
fn spread_expose(app: &AppHandle) {
    // Read preferences
    let (max_windows, sort_by, gap) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        (
            prefs.tiling.expose_max_windows as usize,
            prefs.tiling.expose_sort_by.clone(),
            prefs.tiling.gap,
        )
    };

    // Get all on-screen windows
    let all_windows = get_all_windows();
    if all_windows.is_empty() {
        log::info!("expose: no windows found");
        return;
    }

    // Get displays
    let displays = get_display_visible_frames();
    if displays.is_empty() {
        return;
    }

    // Determine which display to use (where the frontmost window is)
    let target_display = find_display_for_window(&all_windows[0].bounds, &displays);
    let display = &displays[target_display];

    // Group by owner_pid, keeping insertion order
    let mut app_groups: Vec<(String, i32, Vec<&WindowInfo>)> = Vec::new();
    for w in &all_windows {
        if let Some(group) = app_groups.iter_mut().find(|(_, pid, _)| *pid == w.owner_pid) {
            group.2.push(w);
        } else {
            app_groups.push((w.owner_name.clone(), w.owner_pid, vec![w]));
        }
    }

    // Sort app groups
    if sort_by == "alphabetical" {
        app_groups.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    }
    // "recent" keeps the CGWindowList order (already sorted by most recently used)

    // Flatten into ordered window list, capped at max
    let ordered: Vec<&WindowInfo> = app_groups
        .iter()
        .flat_map(|(_, _, wins)| wins.iter().copied())
        .take(max_windows)
        .collect();

    if ordered.is_empty() {
        return;
    }

    let n = ordered.len();
    let cols = (n as f64).sqrt().ceil() as usize;
    let rows = (n + cols - 1) / cols;

    let g = gap as f64;
    let cell_w = (display.width - g * (cols as f64 + 1.0)) / cols as f64;
    let cell_h = (display.height - g * (rows as f64 + 1.0)) / rows as f64;

    // Save original positions and move windows
    let mut saved = expose_saved().lock().unwrap();
    saved.clear();

    for (idx, win_info) in ordered.iter().enumerate() {
        let col = idx % cols;
        let row = idx / cols;
        let x = display.x + g + col as f64 * (cell_w + g);
        let y = display.y + g + row as f64 * (cell_h + g);

        // Save original position
        saved.insert(win_info.window_id, win_info.bounds.clone());

        // Move window using AX API: find AXUIElement by PID + window ID
        unsafe {
            if let Some(ax_win) = get_ax_window_by_id(win_info.owner_pid, win_info.window_id) {
                set_window_rect(
                    &ax_win,
                    &Rect {
                        x,
                        y,
                        width: cell_w,
                        height: cell_h,
                    },
                );
            }
        }
    }

    EXPOSE_ACTIVE.store(true, Ordering::Relaxed);
    log::info!(
        "expose: spread {} windows into {}x{} grid on display {}",
        n,
        cols,
        rows,
        target_display
    );
}

/// Restore all windows to their pre-exposé positions.
fn restore_expose(_app: &AppHandle) {
    let mut saved = expose_saved().lock().unwrap();

    for (&wid, rect) in saved.iter() {
        // We need PID to create AXUIElement — scan all windows to find it
        let all_windows = get_all_windows();
        if let Some(info) = all_windows.iter().find(|w| w.window_id == wid) {
            unsafe {
                if let Some(ax_win) = get_ax_window_by_id(info.owner_pid, wid) {
                    set_window_rect(&ax_win, rect);
                }
            }
        }
    }

    let count = saved.len();
    saved.clear();
    EXPOSE_ACTIVE.store(false, Ordering::Relaxed);
    log::info!("expose: restored {} windows", count);
}

/// Get an AXUIElement for a specific window by PID and CGWindowID.
/// Enumerates the app's windows and matches by _AXUIElementGetWindow.
unsafe fn get_ax_window_by_id(pid: i32, target_wid: u32) -> Option<CfRef> {
    let app_key = cfstr("AXApplication")?;
    let _ = app_key; // not needed — we create element from PID

    // Create AXUIElement for the app
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    }
    let app_el = CfRef::new(AXUIElementCreateApplication(pid))?;

    // Get the app's windows
    let attr = cfstr("AXWindows")?;
    let mut windows_ref: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(app_el.as_ptr(), attr.as_ptr(), &mut windows_ref)
        != K_AX_ERROR_SUCCESS
    {
        return None;
    }
    let windows_arr = CfRef::new(windows_ref)?;

    // Iterate windows and find the one matching target_wid
    let count = CFArrayGetCount(windows_arr.as_ptr() as CFArrayRef);
    for i in 0..count {
        let win_el = CFArrayGetValueAtIndex(windows_arr.as_ptr() as CFArrayRef, i);
        if win_el.is_null() {
            continue;
        }
        let mut wid: u32 = 0;
        if _AXUIElementGetWindow(win_el, &mut wid) == K_AX_ERROR_SUCCESS && wid == target_wid {
            // Retain the element since CFArrayGetValueAtIndex doesn't give us ownership
            extern "C" {
                fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
            }
            CFRetain(win_el);
            return CfRef::new(win_el);
        }
    }

    None
}

// ===========================================================================
// App Exposé — show only the active app's windows, minimize everything else
// ===========================================================================

/// Whether app exposé is currently active.
static APP_EXPOSE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Saved positions of the active app's windows before app exposé, keyed by CGWindowID.
static APP_EXPOSE_SAVED: std::sync::OnceLock<std::sync::Mutex<HashMap<u32, Rect>>> =
    std::sync::OnceLock::new();

/// Window IDs of windows we minimized during app exposé (so we can unminimize them on restore).
static APP_EXPOSE_MINIMIZED: std::sync::OnceLock<std::sync::Mutex<Vec<(i32, u32)>>> =
    std::sync::OnceLock::new();

/// Get the saved-positions mutex for app exposé.
fn app_expose_saved() -> &'static std::sync::Mutex<HashMap<u32, Rect>> {
    APP_EXPOSE_SAVED.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Get the minimized-windows list for app exposé.
fn app_expose_minimized() -> &'static std::sync::Mutex<Vec<(i32, u32)>> {
    APP_EXPOSE_MINIMIZED.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Set the AXMinimized attribute on a window.
unsafe fn set_window_minimized(ax_win: &CfRef, minimized: bool) -> bool {
    let attr = match cfstr("AXMinimized") {
        Some(a) => a,
        None => return false,
    };
    extern "C" {
        static kCFBooleanTrue: CFTypeRef;
        static kCFBooleanFalse: CFTypeRef;
    }
    let val = if minimized {
        kCFBooleanTrue
    } else {
        kCFBooleanFalse
    };
    AXUIElementSetAttributeValue(ax_win.as_ptr(), attr.as_ptr(), val) == K_AX_ERROR_SUCCESS
}

/// Execute the app exposé command. Toggles between spread and restore.
/// When activated, minimizes all windows not belonging to the frontmost app,
/// then lays out the frontmost app's windows in a grid (like Exposé but app-scoped).
pub fn execute_expose_app(app: &AppHandle) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("app_expose: Accessibility permission not granted");
        return;
    }

    if APP_EXPOSE_ACTIVE.load(Ordering::Relaxed) {
        restore_expose_app(app);
    } else {
        spread_expose_app(app);
    }
}

/// Spread only the frontmost app's windows into a grid, minimizing all others.
fn spread_expose_app(app: &AppHandle) {
    let (max_windows, gap) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        (prefs.tiling.expose_max_windows as usize, prefs.tiling.gap)
    };

    let all_windows = get_all_windows();
    if all_windows.is_empty() {
        log::info!("app_expose: no windows found");
        return;
    }

    // The frontmost window determines the target app
    let target_pid = all_windows[0].owner_pid;
    let target_app = all_windows[0].owner_name.clone();

    // Split into app windows vs other windows
    let app_windows: Vec<&WindowInfo> = all_windows
        .iter()
        .filter(|w| w.owner_pid == target_pid)
        .take(max_windows)
        .collect();
    let other_windows: Vec<&WindowInfo> = all_windows
        .iter()
        .filter(|w| w.owner_pid != target_pid)
        .collect();

    if app_windows.is_empty() {
        return;
    }

    // Get displays
    let displays = get_display_visible_frames();
    if displays.is_empty() {
        return;
    }

    let target_display = find_display_for_window(&app_windows[0].bounds, &displays);
    let display = &displays[target_display];

    // Minimize all other windows
    let mut minimized_list = app_expose_minimized().lock().unwrap();
    minimized_list.clear();
    for w in &other_windows {
        unsafe {
            if let Some(ax_win) = get_ax_window_by_id(w.owner_pid, w.window_id) {
                if set_window_minimized(&ax_win, true) {
                    minimized_list.push((w.owner_pid, w.window_id));
                }
            }
        }
    }

    // Grid layout for the app's windows
    let n = app_windows.len();
    let cols = (n as f64).sqrt().ceil() as usize;
    let rows = (n + cols - 1) / cols;
    let g = gap as f64;
    let cell_w = (display.width - g * (cols as f64 + 1.0)) / cols as f64;
    let cell_h = (display.height - g * (rows as f64 + 1.0)) / rows as f64;

    let mut saved = app_expose_saved().lock().unwrap();
    saved.clear();

    for (idx, win_info) in app_windows.iter().enumerate() {
        let col = idx % cols;
        let row = idx / cols;
        let x = display.x + g + col as f64 * (cell_w + g);
        let y = display.y + g + row as f64 * (cell_h + g);

        saved.insert(win_info.window_id, win_info.bounds.clone());

        unsafe {
            if let Some(ax_win) = get_ax_window_by_id(win_info.owner_pid, win_info.window_id) {
                set_window_rect(
                    &ax_win,
                    &Rect {
                        x,
                        y,
                        width: cell_w,
                        height: cell_h,
                    },
                );
            }
        }
    }

    APP_EXPOSE_ACTIVE.store(true, Ordering::Relaxed);
    log::info!(
        "app_expose: spread {} windows of '{}' into {}x{} grid, minimized {} others",
        n,
        target_app,
        cols,
        rows,
        minimized_list.len()
    );
}

/// Restore all windows after app exposé: unminimize others, restore app window positions.
fn restore_expose_app(_app: &AppHandle) {
    // Restore app windows to original positions
    let mut saved = app_expose_saved().lock().unwrap();
    let all_windows = get_all_windows();

    for (&wid, rect) in saved.iter() {
        if let Some(info) = all_windows.iter().find(|w| w.window_id == wid) {
            unsafe {
                if let Some(ax_win) = get_ax_window_by_id(info.owner_pid, wid) {
                    set_window_rect(&ax_win, rect);
                }
            }
        }
    }
    let app_count = saved.len();
    saved.clear();

    // Unminimize windows we minimized
    let mut minimized_list = app_expose_minimized().lock().unwrap();
    let mut restored = 0;
    for &(pid, wid) in minimized_list.iter() {
        unsafe {
            if let Some(ax_win) = get_ax_window_by_id(pid, wid) {
                if set_window_minimized(&ax_win, false) {
                    restored += 1;
                }
            }
        }
    }
    minimized_list.clear();

    APP_EXPOSE_ACTIVE.store(false, Ordering::Relaxed);
    log::info!(
        "app_expose: restored {} app windows, unminimized {} others",
        app_count,
        restored
    );
}

// ===========================================================================
// Tile Snap — mouse edge snapping with preview overlay
// ===========================================================================

// --- CGEvent FFI ---

type CGEventRef = *mut c_void;
type CGEventTapProxy = *mut c_void;
type CFMachPortRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type DispatchQueue = *mut c_void;

const K_CG_SESSION_EVENT_TAP: u32 = 1;
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFFFFFE;

/// CGEventTap callback signature.
type CGEventTapCallBack = extern "C" fn(
    proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: i64,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
}

extern "C" {
    fn dispatch_async_f(
        queue: DispatchQueue,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
    /// `_dispatch_main_q` is the actual symbol for the main dispatch queue.
    /// `dispatch_get_main_queue()` is a macro/inline that returns `&_dispatch_main_q`.
    static _dispatch_main_q: std::ffi::c_char;
    static kCFRunLoopCommonModes: CFStringRef;
}

/// Returns a pointer to the main dispatch queue.
fn dispatch_get_main_queue() -> DispatchQueue {
    unsafe { &_dispatch_main_q as *const _ as DispatchQueue }
}

// --- Overlay window (NSWindow, main thread only) ---

/// Commands dispatched to the main thread for overlay window management.
enum OverlayCmd {
    Show { x: f64, y: f64, w: f64, h: f64 },
    Hide,
}

/// Global overlay NSWindow pointer. Only accessed from the main thread.
static OVERLAY_PTR: std::sync::OnceLock<std::sync::atomic::AtomicUsize> =
    std::sync::OnceLock::new();

/// Initialize the overlay on the main thread. Call once during app setup.
fn init_overlay_on_main_thread() {
    OVERLAY_PTR.get_or_init(|| {
        let ptr = unsafe { create_overlay_window() };
        std::sync::atomic::AtomicUsize::new(ptr as usize)
    });
}

/// Create a borderless, transparent, click-through NSWindow for the snap preview.
#[allow(unexpected_cfgs)]
unsafe fn create_overlay_window() -> *mut c_void {
    use objc::{msg_send, sel, sel_impl};
    use objc::runtime::Object;

    let cls = objc::runtime::Class::get("NSWindow").unwrap();
    let window: *mut Object = msg_send![cls, alloc];
    let rect = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize { width: 1.0, height: 1.0 },
    };
    let window: *mut Object = msg_send![window,
        initWithContentRect:rect
        styleMask:0u64
        backing:2u64
        defer:false
    ];

    // Semi-transparent blue background
    let color_cls = objc::runtime::Class::get("NSColor").unwrap();
    let color: *mut Object = msg_send![color_cls,
        colorWithRed:0.2f64
        green:0.5f64
        blue:1.0f64
        alpha:0.2f64
    ];
    let _: () = msg_send![window, setBackgroundColor: color];
    let _: () = msg_send![window, setOpaque: false];
    let _: () = msg_send![window, setHasShadow: false];
    // NSStatusWindowLevel = 25 — above dragged windows
    let _: () = msg_send![window, setLevel: 25i64];
    // Click-through: mouse events pass through to windows below
    let _: () = msg_send![window, setIgnoresMouseEvents: true];
    let _: () = msg_send![window, setReleasedWhenClosed: false];

    window as *mut c_void
}

/// Dispatch an overlay command to the main thread.
fn dispatch_overlay(cmd: OverlayCmd) {
    let boxed = Box::into_raw(Box::new(cmd)) as *mut c_void;
    unsafe {
        dispatch_async_f(dispatch_get_main_queue(), boxed, run_overlay_cmd);
    }
}

/// Callback executed on the main thread to show/hide the overlay.
#[allow(unexpected_cfgs)]
extern "C" fn run_overlay_cmd(ctx: *mut c_void) {
    let cmd = unsafe { Box::from_raw(ctx as *mut OverlayCmd) };
    let ptr = match OVERLAY_PTR.get() {
        Some(p) => p.load(std::sync::atomic::Ordering::Relaxed) as *mut c_void,
        None => return,
    };
    if ptr.is_null() {
        return;
    }

    unsafe {
        use objc::{msg_send, sel, sel_impl};
        use objc::runtime::Object;

        let window = ptr as *mut Object;

        match *cmd {
            OverlayCmd::Show { x, y, w, h } => {
                // Convert CG coords (top-left origin) to Cocoa coords (bottom-left origin)
                let cls = objc::runtime::Class::get("NSScreen").unwrap();
                let screens: *mut Object = msg_send![cls, screens];
                let count: usize = msg_send![screens, count];
                let primary_h = if count > 0 {
                    let screen: *mut Object = msg_send![screens, objectAtIndex: 0usize];
                    let frame: CGRect = msg_send![screen, frame];
                    frame.size.height
                } else {
                    1080.0
                };

                let cocoa_y = primary_h - y - h;
                let frame = CGRect {
                    origin: CGPoint { x, y: cocoa_y },
                    size: CGSize { width: w, height: h },
                };
                let _: () = msg_send![window, setFrame:frame display:true];
                let _: () = msg_send![window, orderFront:std::ptr::null::<Object>()];
            }
            OverlayCmd::Hide => {
                let _: () = msg_send![window, orderOut:std::ptr::null::<Object>()];
            }
        }
    }
}

// --- Snap zone detection ---

/// Detect which snap zone the cursor is in, if any.
/// Returns the target layout and display index.
/// `side_edge`: pixel trigger for left/right/bottom edges.
/// `top_edge`: pixel trigger for top edge (maximize).
/// `corner`: pixel trigger for corner zones (quarters).
fn detect_snap_zone(
    cx: f64,
    cy: f64,
    displays: &[Rect],
    side_edge: f64,
    top_edge: f64,
    corner: f64,
) -> Option<(TilingLayout, usize)> {
    // Two-pass: first check displays whose bounds contain the cursor (exact match),
    // then check displays where the cursor is just outside (overflow into menu bar/dock).
    // This prevents the margin expansion from stealing a cursor that belongs to an adjacent display.
    let passes: &[bool] = &[false, true]; // false = exact only, true = with margin
    for &allow_overflow in passes {
        for (i, d) in displays.iter().enumerate() {
            let in_bounds = cx >= d.x && cx < d.x + d.width && cy >= d.y && cy < d.y + d.height;

            if !allow_overflow && !in_bounds {
                continue;
            }
            if allow_overflow && in_bounds {
                continue; // already checked in first pass
            }
            if allow_overflow {
                // Only allow vertical overflow (top/bottom — menu bar and dock).
                // Horizontal overflow would bleed into adjacent side-by-side displays.
                let v_margin = corner.max(top_edge);
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
            let clamped_x = cx;
            let clamped_y = cy.clamp(d.y, d.y + d.height - 1.0);
            let left = clamped_x - d.x;
            let right = d.x + d.width - clamped_x;
            let top = clamped_y - d.y;
            let bottom = d.y + d.height - clamped_y;

        let at_left = left < side_edge;
        let at_right = right < side_edge;
        let at_top = top < top_edge;
        let at_bottom = bottom < side_edge;
        let in_corner_top = top < corner;
        let in_corner_bottom = bottom < corner;
        let in_corner_left = left < corner;
        let in_corner_right = right < corner;

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

        // Cursor is on/near this display but not in a snap zone
        }
    }
    None
}

// --- Aero snap state and event tap ---

/// Shared state between the CGEventTap callback and the main app.
struct SnapContext {
    app: AppHandle,
    tap: std::sync::Mutex<CFMachPortRef>,
    state: std::sync::Mutex<SnapState>,
}

// Safety: the raw pointers in SnapContext (CFMachPortRef) are only accessed
// from the event tap thread after initialization, protected by Mutex.
unsafe impl Send for SnapContext {}
unsafe impl Sync for SnapContext {}

struct SnapState {
    dragging: bool,
    drag_confirmed: bool,
    drag_start_window_pos: Option<(f64, f64)>,
    current_layout: Option<TilingLayout>,
    current_display: usize,
    displays: Vec<Rect>,
    half_ratio: u32,
    third_ratio: u32,
    gap: u32,
    side_edge_trigger: f64,
    top_edge_trigger: f64,
    corner_trigger: f64,
}

/// CGEventTap callback — runs on the event tap background thread.
extern "C" fn snap_event_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    // Re-enable tap if it was disabled by timeout
    if event_type == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT {
        let ctx = unsafe { &*(user_info as *const SnapContext) };
        if let Ok(tap) = ctx.tap.lock() {
            if !tap.is_null() {
                unsafe { CGEventTapEnable(*tap, true) };
            }
        }
        return event;
    }

    let ctx = unsafe { &*(user_info as *const SnapContext) };

    // Check if tiling is enabled
    let enabled = ctx
        .app
        .try_state::<crate::AppState>()
        .and_then(|s| s.preferences.lock().ok().map(|p| p.tiling.enabled))
        .unwrap_or(false);
    if !enabled {
        return event;
    }

    let cursor = unsafe { CGEventGetLocation(event) };

    let mut state = match ctx.state.try_lock() {
        Ok(s) => s,
        Err(_) => return event, // skip if contended
    };

    match event_type {
        K_CG_EVENT_LEFT_MOUSE_DOWN => {
            state.dragging = true;
            state.drag_confirmed = false;
            state.current_layout = None;
            // Get focused window position for later verification
            state.drag_start_window_pos = unsafe {
                get_focused_window()
                    .and_then(|w| get_window_rect(&w).map(|r| (r.x, r.y)))
            };
            // Refresh display frames and preferences
            state.displays = get_display_visible_frames();
            let prefs = ctx
                .app
                .try_state::<crate::AppState>()
                .and_then(|s| s.preferences.lock().ok().map(|p| p.tiling.clone()));
            if let Some(tp) = prefs {
                state.half_ratio = tp.half_ratio;
                state.third_ratio = tp.third_ratio;
                state.gap = tp.gap;
                state.side_edge_trigger = tp.side_edge_trigger as f64;
                state.top_edge_trigger = tp.top_edge_trigger as f64;
                state.corner_trigger = tp.corner_trigger as f64;
            }
        }

        K_CG_EVENT_LEFT_MOUSE_DRAGGED => {
            if !state.dragging || state.displays.is_empty() {
                return event;
            }

            let zone = detect_snap_zone(
                cursor.x,
                cursor.y,
                &state.displays,
                state.side_edge_trigger,
                state.top_edge_trigger,
                state.corner_trigger,
            );

            match zone {
                Some((layout, display_idx)) => {
                    // Only show overlay if layout changed
                    if state.current_layout != Some(layout) || state.current_display != display_idx
                    {
                        state.current_layout = Some(layout);
                        state.current_display = display_idx;
                        let target = calculate_target_rect(
                            layout,
                            &state.displays[display_idx],
                            state.half_ratio,
                            state.third_ratio,
                            state.gap,
                        );
                        dispatch_overlay(OverlayCmd::Show {
                            x: target.x,
                            y: target.y,
                            w: target.width,
                            h: target.height,
                        });
                    }
                }
                None => {
                    if state.current_layout.is_some() {
                        state.current_layout = None;
                        dispatch_overlay(OverlayCmd::Hide);
                    }
                }
            }
        }

        K_CG_EVENT_LEFT_MOUSE_UP => {
            let was_in_zone = state.current_layout;
            let start_pos = state.drag_start_window_pos;
            state.dragging = false;
            state.drag_confirmed = false;
            state.current_layout = None;

            // Hide overlay
            dispatch_overlay(OverlayCmd::Hide);

            // If cursor was in a snap zone, verify the window actually moved
            if let Some(layout) = was_in_zone {
                let window_moved = unsafe {
                    get_focused_window().map_or(false, |w| {
                        let cur_pos = get_window_rect(&w).map(|r| (r.x, r.y));
                        match (start_pos, cur_pos) {
                            (Some((sx, sy)), Some((cx, cy))) => {
                                (cx - sx).abs() > 10.0 || (cy - sy).abs() > 10.0
                            }
                            _ => false,
                        }
                    })
                };

                if window_moved {
                    let layout_str = match layout {
                        TilingLayout::LeftHalf => "leftHalf",
                        TilingLayout::RightHalf => "rightHalf",
                        TilingLayout::Maximize => "maximize",
                        TilingLayout::TopLeftQuarter => "topLeftQuarter",
                        TilingLayout::TopRightQuarter => "topRightQuarter",
                        TilingLayout::BottomLeftQuarter => "bottomLeftQuarter",
                        TilingLayout::BottomRightQuarter => "bottomRightQuarter",
                        _ => return event,
                    };
                    let app = ctx.app.clone();
                    let ls = layout_str.to_string();
                    std::thread::spawn(move || {
                        execute_tile(&app, &ls);
                    });
                }
            }
        }

        _ => {}
    }

    event
}

/// Start the tile snap event tap on a background thread.
/// Call once during app setup. Requires Accessibility permission.
pub fn start_tile_snap(app: AppHandle) {
    // Create overlay window on the main thread
    init_overlay_on_main_thread();

    let ctx = std::sync::Arc::new(SnapContext {
        app,
        tap: std::sync::Mutex::new(std::ptr::null_mut()),
        state: std::sync::Mutex::new(SnapState {
            dragging: false,
            drag_confirmed: false,
            drag_start_window_pos: None,
            current_layout: None,
            current_display: 0,
            displays: Vec::new(),
            half_ratio: 50,
            third_ratio: 33,
            gap: 0,
            side_edge_trigger: 10.0,
            top_edge_trigger: 10.0,
            corner_trigger: 50.0,
        }),
    });

    // Leak the Arc so it lives for the app's lifetime.
    let raw_addr = std::sync::Arc::into_raw(ctx) as usize;

    std::thread::spawn(move || {
        let raw = raw_addr as *mut c_void;
        unsafe {
            let mask: u64 = (1 << K_CG_EVENT_LEFT_MOUSE_DOWN)
                | (1 << K_CG_EVENT_LEFT_MOUSE_UP)
                | (1 << K_CG_EVENT_LEFT_MOUSE_DRAGGED);

            let tap = CGEventTapCreate(
                K_CG_SESSION_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                K_CG_EVENT_TAP_OPTION_LISTEN_ONLY,
                mask,
                snap_event_callback,
                raw,
            );

            if tap.is_null() {
                log::warn!(
                    "tile_snap: Failed to create CGEventTap. \
                     Accessibility permission may not be granted."
                );
                return;
            }

            // Store tap ref for re-enabling on timeout
            let ctx = &*(raw as *const SnapContext);
            if let Ok(mut t) = ctx.tap.lock() {
                *t = tap;
            }

            let source =
                CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            if source.is_null() {
                log::warn!("tile_snap: Failed to create run loop source");
                return;
            }

            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);

            log::info!("tile_snap: Event tap started, listening for window drags");
            CFRunLoopRun(); // blocks forever
        }
    });
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
    fn test_vertical_two_thirds() {
        let d = display(0.0, 0.0, 1920.0, 900.0);
        let top = calculate_target_rect(TilingLayout::TopTwoThirds, &d, 50, 33, 0);
        let bot = calculate_target_rect(TilingLayout::BottomTwoThirds, &d, 50, 33, 0);
        // top two-thirds = 1.0 - 0.33 = 0.67 => 603
        assert!(rect_approx(&top, 0.0, 0.0, 1920.0, 603.0));
        // bottom two-thirds starts at third = 0.33 => 297
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

    // -- Snap zone detection --

    #[test]
    fn test_snap_left_edge() {
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        // Cursor at left edge, middle height
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
        // Cursor at top-left corner (within both edge and corner triggers)
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
        // Cursor in the middle — no snap zone
        let result = detect_snap_zone(960.0, 540.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_snap_second_monitor() {
        let displays = vec![
            display(0.0, 0.0, 1920.0, 1080.0),
            display(1920.0, 0.0, 2560.0, 1440.0),
        ];
        // Left edge of second monitor
        let result = detect_snap_zone(1922.0, 540.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::LeftHalf, 1)));
    }

    #[test]
    fn test_snap_cursor_above_display_into_menu_bar() {
        // Cursor above the display (in menu bar) should still trigger maximize
        let displays = vec![display(0.0, 25.0, 1920.0, 1055.0)];
        // y = 20 is above the display (top at y=25) — should clamp and trigger top edge
        let result = detect_snap_zone(960.0, 20.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, Some((TilingLayout::Maximize, 0)));
    }

    #[test]
    fn test_snap_no_horizontal_overflow() {
        // Cursor past the left edge should NOT trigger — horizontal overflow
        // is disabled to prevent bleeding into adjacent side-by-side displays.
        let displays = vec![display(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone(-3.0, 540.0, &displays, 10.0, 10.0, 50.0);
        assert_eq!(result, None);
    }
}
