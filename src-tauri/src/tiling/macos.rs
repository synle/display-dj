//! macOS window tiling via the Accessibility API.
//!
//! Provides the macOS-specific implementation for window tiling commands
//! (halves, thirds, quarters, maximize, restore, expose).
//! Uses AXUIElement to get the focused window and move/resize it,
//! and NSScreen to get display visible frames (accounting for menu bar and dock).
//!
//! Requires the user to grant Accessibility permission in
//! System Settings > Privacy & Security > Accessibility.

use std::ffi::{c_char, c_void, CString};
use tauri::{AppHandle, Manager};

use super::{
    build_sorted_window_list, calculate_target_rect, find_display_for_window,
    layout_grid_on_display, Rect, TilingLayout, WindowInfo, WindowState,
};

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
    let ptr =
        CFStringCreateWithCString(K_CF_ALLOCATOR_DEFAULT, c.as_ptr(), K_CF_STRING_ENCODING_UTF8);
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
    fn AXUIElementPerformAction(element: CFTypeRef, action: CFStringRef) -> AXError;
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
// Display enumeration (NSScreen visible frames)
// ---------------------------------------------------------------------------

/// Get visible frames for all displays in AX/CoreGraphics coordinates
/// (top-left origin, in points). Accounts for menu bar and dock.
/// Returns displays sorted left-to-right, then top-to-bottom.
#[allow(unexpected_cfgs)]
fn get_display_visible_frames() -> Vec<Rect> {
    unsafe {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};

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

        // Primary screen height for Cocoa -> CG coordinate conversion.
        // MUST use screens[0] (the primary display with the menu bar), NOT
        // mainScreen which returns whichever screen has keyboard focus.
        // The Cocoa coordinate system origin is anchored to the primary display --
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
// Accessibility permission check
// ---------------------------------------------------------------------------

/// Returns true if the app has macOS Accessibility permission.
pub fn is_accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
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

    // Save original position (only on first tile) and update state.
    // Cast u32 CGWindowID to i64 for the HashMap key.
    let wid_key = window_id as i64;
    {
        let state = app.state::<crate::AppState>();
        let mut ts = state.tiling_state.lock().unwrap();
        let entry = ts.windows.entry(wid_key).or_insert(WindowState {
            original: win_rect,
            layout,
            display_index: target_display,
        });
        entry.layout = layout;
        entry.display_index = target_display;
    }

    // Calculate and apply target rect
    let target =
        calculate_target_rect(layout, &displays[target_display], half_ratio, third_ratio, gap);
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

    // Remove state and get original rect. Cast u32 -> i64 for HashMap key.
    let wid_key = window_id as i64;
    let original = {
        let state = app.state::<crate::AppState>();
        let mut ts = state.tiling_state.lock().unwrap();
        ts.windows.remove(&wid_key).map(|ws| ws.original)
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
// Expose -- lay out all windows in a grid for overview
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

/// Get all on-screen normal windows via CGWindowList.
/// Filters out desktop elements, tiny windows (< 50x50), and non-normal layers.
/// "On-screen" includes windows visible on any display, not just the focused one.
/// Returns `super::WindowInfo` structs with `window_id` as `i64`.
fn get_all_windows() -> Vec<WindowInfo> {
    let mut windows = Vec::new();
    unsafe {
        let list = CGWindowListCopyWindowInfo(
            K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS,
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
                window_id: wid as i64,
                owner_pid: pid,
                owner_name: owner,
                bounds,
            });
        }
        CFRelease(list);
    }
    windows
}

// --- Expose state ---

/// Execute the expose command. Lays out all on-screen windows in a grid.
pub fn execute_expose(app: &AppHandle) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("expose: Accessibility permission not granted");
        return;
    }

    spread_expose(app);
}

/// Closure that sets a window rect via AXUIElement, used as the callback
/// for `layout_grid_on_display`.
/// Raise (bring to front) a window via the AXRaise action.
unsafe fn raise_window(window: &CfRef) {
    if let Some(action) = cfstr("AXRaise") {
        AXUIElementPerformAction(window.as_ptr(), action.as_ptr());
    }
}

/// Callback for layout_grid_on_display: set window rect and raise to front.
fn set_window_rect_via_ax(win_info: &WindowInfo, rect: &Rect) {
    unsafe {
        if let Some(ax_win) = get_ax_window_by_id(win_info.owner_pid, win_info.window_id as u32) {
            set_window_rect(&ax_win, rect);
            raise_window(&ax_win);
        }
    }
}

/// Spread all windows into grids, filling display 1 first then overflowing to display 2, etc.
/// Each display holds up to `max_windows` before overflowing to the next.
fn spread_expose(app: &AppHandle) {
    let (max_per_display, gap) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        (prefs.tiling.expose_max_windows as usize, prefs.tiling.gap)
    };

    // Normalize: unminimize and un-fullscreen all windows first
    let (unmin, unfs) = normalize_all_windows();
    if unmin > 0 || unfs > 0 {
        log::info!(
            "expose: normalized {} unminimized, {} un-fullscreened",
            unmin,
            unfs
        );
        // Brief pause to let macOS finish animations before re-fetching
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // Re-fetch windows after normalization
    let all_windows = get_all_windows();
    if all_windows.is_empty() {
        log::info!("expose: no windows found");
        return;
    }

    let displays = get_display_visible_frames();
    if displays.is_empty() {
        return;
    }

    // Total cap = per-display cap * number of displays
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
        layout_grid_on_display(slice, display, g, &set_window_rect_via_ax);
        log::info!("expose: placed {} windows on display {}", count, i);
        offset += count;
    }

    log::info!(
        "expose: spread {} windows across {} displays",
        n.min(offset),
        displays.len()
    );
}

/// Get an AXUIElement for a specific window by PID and CGWindowID.
/// Enumerates the app's windows and matches by _AXUIElementGetWindow.
unsafe fn get_ax_window_by_id(pid: i32, target_wid: u32) -> Option<CfRef> {
    let app_key = cfstr("AXApplication")?;
    let _ = app_key; // not needed -- we create element from PID

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
// Window normalization -- unminimize and un-fullscreen before expose
// ===========================================================================

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

/// Check if a window is minimized via AXMinimized.
unsafe fn is_window_minimized(ax_win: &CfRef) -> bool {
    let attr = match cfstr("AXMinimized") {
        Some(a) => a,
        None => return false,
    };
    let mut val: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(ax_win.as_ptr(), attr.as_ptr(), &mut val)
        != K_AX_ERROR_SUCCESS
    {
        return false;
    }
    if val.is_null() {
        return false;
    }
    extern "C" {
        static kCFBooleanTrue: CFTypeRef;
    }
    let result = val == kCFBooleanTrue;
    CFRelease(val);
    result
}

/// Check if a window is in native fullscreen via AXFullScreen.
unsafe fn is_window_fullscreen(ax_win: &CfRef) -> bool {
    let attr = match cfstr("AXFullScreen") {
        Some(a) => a,
        None => return false,
    };
    let mut val: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(ax_win.as_ptr(), attr.as_ptr(), &mut val)
        != K_AX_ERROR_SUCCESS
    {
        return false;
    }
    if val.is_null() {
        return false;
    }
    extern "C" {
        static kCFBooleanTrue: CFTypeRef;
    }
    let result = val == kCFBooleanTrue;
    CFRelease(val);
    result
}

/// Set AXFullScreen on a window.
unsafe fn set_window_fullscreen(ax_win: &CfRef, fullscreen: bool) -> bool {
    let attr = match cfstr("AXFullScreen") {
        Some(a) => a,
        None => return false,
    };
    extern "C" {
        static kCFBooleanTrue: CFTypeRef;
        static kCFBooleanFalse: CFTypeRef;
    }
    let val = if fullscreen {
        kCFBooleanTrue
    } else {
        kCFBooleanFalse
    };
    AXUIElementSetAttributeValue(ax_win.as_ptr(), attr.as_ptr(), val) == K_AX_ERROR_SUCCESS
}

/// Get all AX windows for a given app PID. Returns (window_element, window_id) pairs.
unsafe fn get_all_ax_windows_for_pid(pid: i32) -> Vec<(CfRef, u32)> {
    let mut result = Vec::new();
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
        fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    }
    let app_el = match CfRef::new(AXUIElementCreateApplication(pid)) {
        Some(e) => e,
        None => return result,
    };
    let attr = match cfstr("AXWindows") {
        Some(a) => a,
        None => return result,
    };
    let mut windows_ref: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(app_el.as_ptr(), attr.as_ptr(), &mut windows_ref)
        != K_AX_ERROR_SUCCESS
    {
        return result;
    }
    let windows_arr = match CfRef::new(windows_ref) {
        Some(a) => a,
        None => return result,
    };
    let count = CFArrayGetCount(windows_arr.as_ptr() as CFArrayRef);
    for i in 0..count {
        let win_el = CFArrayGetValueAtIndex(windows_arr.as_ptr() as CFArrayRef, i);
        if win_el.is_null() {
            continue;
        }
        let mut wid: u32 = 0;
        if _AXUIElementGetWindow(win_el, &mut wid) == K_AX_ERROR_SUCCESS {
            CFRetain(win_el);
            if let Some(cf) = CfRef::new(win_el) {
                result.push((cf, wid));
            }
        }
    }
    result
}

/// Normalize all windows: unminimize minimized windows and exit fullscreen.
/// Returns the number of windows that were changed (for logging).
/// After calling this, the caller should re-fetch the window list since
/// windows may now be visible that weren't before.
fn normalize_all_windows() -> (usize, usize) {
    // Collect unique PIDs from on-screen windows first, then also check
    // all running GUI apps for minimized windows (which won't appear in CGWindowList).
    let mut pids: Vec<i32> = Vec::new();

    // Get on-screen windows for their PIDs
    let on_screen = get_all_windows();
    for w in &on_screen {
        if !pids.contains(&w.owner_pid) {
            pids.push(w.owner_pid);
        }
    }

    let mut unminimized = 0;
    let mut unfullscreened = 0;

    unsafe {
        for &pid in &pids {
            let ax_windows = get_all_ax_windows_for_pid(pid);
            for (ax_win, _wid) in &ax_windows {
                if is_window_minimized(ax_win) {
                    if set_window_minimized(ax_win, false) {
                        unminimized += 1;
                    }
                }
                if is_window_fullscreen(ax_win) {
                    if set_window_fullscreen(ax_win, false) {
                        unfullscreened += 1;
                    }
                }
            }
        }
    }

    (unminimized, unfullscreened)
}

// ===========================================================================
// App Expose -- show only the active app's windows in a grid
// ===========================================================================

/// Execute the app expose command. Lays out the frontmost app's windows in a grid.
pub fn execute_expose_app(app: &AppHandle) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("app_expose: Accessibility permission not granted");
        return;
    }

    spread_expose_app(app);
}

/// Spread only the frontmost app's windows into a grid, filling display 1 first then overflowing.
fn spread_expose_app(app: &AppHandle) {
    let (max_per_display, gap) = {
        let state = app.state::<crate::AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        (prefs.tiling.expose_max_windows as usize, prefs.tiling.gap)
    };

    // Identify the target app before normalization (frontmost window)
    let pre_windows = get_all_windows();
    if pre_windows.is_empty() {
        log::info!("app_expose: no windows found");
        return;
    }
    let target_pid = pre_windows[0].owner_pid;
    let target_app = pre_windows[0].owner_name.clone();

    // Normalize: unminimize and un-fullscreen all windows of the target app
    let (unmin, unfs) = normalize_all_windows();
    if unmin > 0 || unfs > 0 {
        log::info!(
            "app_expose: normalized {} unminimized, {} un-fullscreened",
            unmin,
            unfs
        );
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // Re-fetch windows after normalization
    let all_windows = get_all_windows();
    if all_windows.is_empty() {
        return;
    }

    let displays = get_display_visible_frames();
    if displays.is_empty() {
        return;
    }

    // Total cap = per-display cap * number of displays
    let total_cap = max_per_display * displays.len();

    // Filter to target app's windows, sorted by window_id for determinism
    let mut app_windows: Vec<&WindowInfo> = all_windows
        .iter()
        .filter(|w| w.owner_pid == target_pid)
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
        layout_grid_on_display(slice, display, g, &set_window_rect_via_ax);
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

// ===========================================================================
// Tile Snap -- mouse edge snapping with preview overlay
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
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let cls = objc::runtime::Class::get("NSWindow").unwrap();
    let window: *mut Object = msg_send![cls, alloc];
    let rect = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 1.0,
            height: 1.0,
        },
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
    // NSStatusWindowLevel = 25 -- above dragged windows
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
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};

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

// --- Snap zone detection (macOS two-pass version) ---

/// Detect which snap zone the cursor is in, if any (macOS two-pass version).
/// Uses a two-pass approach: first checks displays whose bounds contain the cursor
/// (exact match), then checks displays where the cursor is just outside (overflow
/// into menu bar/dock). This prevents the margin expansion from stealing a cursor
/// that belongs to an adjacent display.
/// Returns the target layout and display index.
/// `side_edge`: pixel trigger for left/right/bottom edges.
/// `top_edge`: pixel trigger for top edge (maximize).
/// `corner`: pixel trigger for corner zones (quarters).
fn detect_snap_zone_macos(
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
                // Only allow vertical overflow (top/bottom -- menu bar and dock).
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

            // Clamp cursor vertically to display bounds -- treats "above/below
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

/// Mutable state tracked during a drag for snap zone detection.
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

/// CGEventTap callback -- runs on the event tap background thread.
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
                get_focused_window().and_then(|w| get_window_rect(&w).map(|r| (r.x, r.y)))
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

            let zone = detect_snap_zone_macos(
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

            let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
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
