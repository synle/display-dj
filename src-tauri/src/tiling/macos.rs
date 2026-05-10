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
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Remembers the PID of the app most recently sent to back via
/// `move_window_to_back` / `move_app_to_back`, so the next
/// `move_window_to_front` / `move_app_to_front` can target that app instead
/// of "whatever is currently focused" — which would just be the *other*
/// app we activated to push the original behind.
///
/// Acts as a single-slot LIFO stack: each back overwrites the previous
/// memory; each front consumes it. If the memory is empty (or the stored
/// PID is no longer alive / has no visible window), front falls back to the
/// currently focused window.
static LAST_BACKED_PID: Mutex<Option<i32>> = Mutex::new(None);

/// Record `pid` as "the app we just sent to back," so a subsequent
/// move-to-front can bring it back even though focus has shifted to a
/// different app. Logs at info level.
fn remember_backed_pid(pid: i32) {
    if let Ok(mut g) = LAST_BACKED_PID.lock() {
        *g = Some(pid);
        log::info!("remember_backed_pid: stored pid={} for next moveToFront", pid);
    }
}

/// Take the remembered backed PID, if any. Returns the PID and clears the
/// memory so the next call after this one falls back to the focused window.
fn take_backed_pid() -> Option<i32> {
    LAST_BACKED_PID.lock().ok().and_then(|mut g| g.take())
}

use super::{
    build_sorted_window_list, calculate_target_rect, find_display_for_window,
    layout_across_displays, plan_expose, plan_expose_app, plan_layout_preset, Rect,
    TilingLayout, WindowInfo, WindowState,
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
    fn CFNumberCreate(
        allocator: CFAllocatorRef,
        the_type: i64,
        value_ptr: *const c_void,
    ) -> CFTypeRef;
    fn CFArrayCreate(
        allocator: CFAllocatorRef,
        values: *const CFTypeRef,
        num_values: i64,
        callbacks: *const c_void,
    ) -> CFArrayRef;
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

    /// Consume the CfRef and return the raw pointer WITHOUT calling CFRelease.
    /// The caller is responsible for releasing the pointer (or wrapping it in
    /// a new CfRef on another thread).
    fn into_raw(self) -> CFTypeRef {
        let ptr = self.0;
        std::mem::forget(self); // skip Drop
        ptr
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
    /// Public API: returns the PID of the process that owns this AXUIElement.
    /// Used to bridge focused-window AX elements back to NSRunningApplication
    /// for app activation (move-to-front, etc.).
    fn AXUIElementGetPid(element: CFTypeRef, pid: *mut i32) -> AXError;
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

/// Get full frames for all displays in AX/CoreGraphics coordinates
/// (top-left origin, in points). Includes menu bar and dock area (NSScreen.frame).
/// Used to detect pseudo-fullscreen windows (browser F11 / video fullscreen)
/// that cover the entire display but don't set AXFullScreen.
#[allow(unexpected_cfgs)]
fn get_display_full_frames() -> Vec<Rect> {
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

        let primary_screen: *mut Object = msg_send![screens, objectAtIndex: 0usize];
        let primary_frame: CGRect = msg_send![primary_screen, frame];
        let primary_h = primary_frame.size.height;

        let mut frames = Vec::with_capacity(count);
        for i in 0..count {
            let screen: *mut Object = msg_send![screens, objectAtIndex: i];
            // frame: full display including menu bar/dock (Cocoa coords)
            let full: CGRect = msg_send![screen, frame];
            // Convert to CG/AX coords
            frames.push(Rect {
                x: full.origin.x,
                y: primary_h - full.origin.y - full.size.height,
                width: full.size.width,
                height: full.size.height,
            });
        }

        frames.sort_by(|a, b| {
            a.x.partial_cmp(&b.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        });

        frames
    }
}

/// Check if a window covers an entire display (including menu bar area).
/// Used to detect browser F11 fullscreen or video fullscreen which don't
/// set the native AXFullScreen attribute.
fn is_pseudo_fullscreen(win_bounds: &Rect, full_frames: &[Rect]) -> bool {
    for d in full_frames {
        // Window must roughly match the display's full frame (within 5px)
        if (win_bounds.x - d.x).abs() < 5.0
            && (win_bounds.y - d.y).abs() < 5.0
            && (win_bounds.width - d.width).abs() < 5.0
            && (win_bounds.height - d.height).abs() < 5.0
        {
            return true;
        }
    }
    false
}

/// Send an Escape keystroke to the current frontmost app via CGEvent.
/// Used to exit browser F11 fullscreen or video player fullscreen.
unsafe fn send_escape_key() {
    extern "C" {
        fn CGEventCreateKeyboardEvent(
            source: *const c_void,
            virtual_key: u16,
            key_down: bool,
        ) -> *mut c_void;
        fn CGEventPost(tap: u32, event: *mut c_void);
    }
    const K_CG_SESSION_EVENT_TAP: u32 = 1;
    const K_VK_ESCAPE: u16 = 53;

    let key_down = CGEventCreateKeyboardEvent(std::ptr::null(), K_VK_ESCAPE, true);
    if !key_down.is_null() {
        CGEventPost(K_CG_SESSION_EVENT_TAP, key_down);
        CFRelease(key_down);
    }
    let key_up = CGEventCreateKeyboardEvent(std::ptr::null(), K_VK_ESCAPE, false);
    if !key_up.is_null() {
        CGEventPost(K_CG_SESSION_EVENT_TAP, key_up);
        CFRelease(key_up);
    }
}

/// Activate (bring to front) an app by PID via NSRunningApplication.
unsafe fn activate_app_by_pid(pid: i32) {
    activate_app_by_pid_with_options(pid, 2);
}

/// Activate an app by PID with explicit NSApplicationActivationOptions.
///
/// Common option masks:
///   - `2` = `NSApplicationActivateIgnoringOtherApps` (1 << 1) — make this app
///     active, but only its key/main window is brought forward.
///   - `3` = `NSApplicationActivateAllWindows | NSApplicationActivateIgnoringOtherApps`
///     (1 | 2) — make this app active AND raise every one of its windows above
///     all other apps' windows.
unsafe fn activate_app_by_pid_with_options(pid: i32, options: u64) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let cls = match objc::runtime::Class::get("NSRunningApplication") {
        Some(c) => c,
        None => return,
    };
    let app: *mut Object =
        msg_send![cls, runningApplicationWithProcessIdentifier: pid];
    if app.is_null() {
        return;
    }
    let _: bool = msg_send![app, activateWithOptions: options];
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

/// Get the window title (AXTitle) and owning app name for debug logging.
unsafe fn get_window_debug_info(window: &CfRef) -> String {
    extern "C" {
        fn CFStringGetLength(s: CFStringRef) -> i64;
        fn CFStringGetCString(
            s: CFStringRef,
            buffer: *mut c_char,
            buffer_size: i64,
            encoding: u32,
        ) -> bool;
    }
    let read_str = |attr_name: &str| -> String {
        let attr = match cfstr(attr_name) {
            Some(a) => a,
            None => return String::new(),
        };
        let mut val: CFTypeRef = std::ptr::null();
        if AXUIElementCopyAttributeValue(window.as_ptr(), attr.as_ptr(), &mut val)
            != K_AX_ERROR_SUCCESS
            || val.is_null()
        {
            return String::new();
        }
        let len = CFStringGetLength(val);
        let buf_size = len * 4 + 1;
        let mut buf = vec![0u8; buf_size as usize];
        let ok = CFStringGetCString(
            val,
            buf.as_mut_ptr() as *mut c_char,
            buf_size,
            K_CF_STRING_ENCODING_UTF8,
        );
        CFRelease(val);
        if ok {
            let c_str = std::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char);
            c_str.to_string_lossy().into_owned()
        } else {
            String::new()
        }
    };
    let title = read_str("AXTitle");
    // Truncate long titles
    let title_short = if title.len() > 40 {
        format!("{}…", &title[..40])
    } else {
        title
    };
    format!("'{}'", title_short)
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

/// Query the minimum size a window allows via AXMinimumSize.
unsafe fn get_window_min_size(window: &CfRef) -> Option<(f64, f64)> {
    let attr = cfstr("AXMinimumSize")?;
    let mut val: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(window.as_ptr(), attr.as_ptr(), &mut val)
        != K_AX_ERROR_SUCCESS
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
    Some((size.width, size.height))
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
/// If the saved original rect is oversized (≥ 85% of the display in both
/// dimensions), a smart restore size is used instead: 60% of the smallest
/// display, but no smaller than the app's own minimum size (AXMinimumSize).
fn execute_restore(app: &AppHandle) {
    if unsafe { !AXIsProcessTrusted() } {
        log::warn!(
            "tiling restore: Accessibility permission not granted. \
             Go to System Settings > Privacy & Security > Accessibility and add this app."
        );
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
            rect.x, rect.y, rect.width, rect.height,
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
                min_size: None,
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

/// Read the PID of the process that owns an AXUIElement (e.g. a window).
/// Used to bridge a focused-window AX element back to NSRunningApplication
/// for app activation.
unsafe fn get_window_pid(window: &CfRef) -> Option<i32> {
    let mut pid: i32 = 0;
    if AXUIElementGetPid(window.as_ptr(), &mut pid) == K_AX_ERROR_SUCCESS && pid > 0 {
        Some(pid)
    } else {
        None
    }
}

/// Bring the currently focused window to the front.
///
/// 1. Activates the owning app (NSRunningApplication, opts=2)
/// 2. Performs `AXRaise` on the window so it's the topmost within the app.
///
/// No-op if Accessibility permission isn't granted or there's no focused window.
pub fn move_window_to_front(_app: &AppHandle) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("move_window_to_front: Accessibility permission not granted");
        return;
    }
    // If the most recent action was a moveToBack, the focused app is
    // whatever we activated to push the original app behind — not the
    // window the user wants to bring forward. Prefer the remembered
    // "last backed PID" so a back/front pair forms a natural undo.
    if let Some(remembered_pid) = take_backed_pid() {
        unsafe {
            activate_app_by_pid(remembered_pid);
            // Raise the frontmost AX window of that app so it's the
            // topmost window within the (now-active) app.
            if let Some((w, _wid)) = get_all_ax_windows_for_pid(remembered_pid).into_iter().next() {
                raise_window(&w);
            }
        }
        log::info!(
            "move_window_to_front: brought back remembered pid={} (was sent to back earlier)",
            remembered_pid,
        );
        return;
    }
    unsafe {
        let window = match get_focused_window() {
            Some(w) => w,
            None => {
                log::info!("move_window_to_front: no focused window");
                return;
            }
        };
        let wid = get_window_id(&window);
        let pid = get_window_pid(&window);
        if let Some(p) = pid {
            activate_app_by_pid(p);
        }
        raise_window(&window);
        log::info!(
            "move_window_to_front: dispatched (pid={:?}, wid={:?})",
            pid, wid,
        );
    }
}

/// Bring all windows of the focused app above all other apps' windows.
///
/// Uses `NSApplicationActivateAllWindows | NSApplicationActivateIgnoringOtherApps`
/// (opts=3) plus an explicit `AXRaise` loop on every AX window of the app as a
/// belt-and-suspenders fallback (some macOS versions don't fully respect the
/// `NSApplicationActivateAllWindows` flag). The originally focused window is
/// raised last so it remains topmost.
///
/// No-op if Accessibility permission isn't granted or there's no focused window.
pub fn move_app_to_front(_app: &AppHandle) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("move_app_to_front: Accessibility permission not granted");
        return;
    }
    // Same back/front-pair reasoning as `move_window_to_front`: if a
    // moveToBack just ran, the focused app is the one we activated to
    // push the original behind. Prefer the remembered PID so all of its
    // windows come back together.
    let remembered = take_backed_pid();
    unsafe {
        let (focused, pid) = if let Some(p) = remembered {
            // Synthesize a "focused window" from the first AX window of
            // the remembered app. If it has no AX windows, fall through
            // to the regular focused-window path below.
            match get_all_ax_windows_for_pid(p).into_iter().next() {
                Some((w, _wid)) => {
                    log::info!(
                        "move_app_to_front: bringing back remembered pid={} (was sent to back earlier)",
                        p,
                    );
                    (w, p)
                }
                None => {
                    log::info!(
                        "move_app_to_front: remembered pid={} has no AX windows; falling back to focused",
                        p,
                    );
                    let f = match get_focused_window() {
                        Some(w) => w,
                        None => {
                            log::info!("move_app_to_front: no focused window");
                            return;
                        }
                    };
                    let fp = match get_window_pid(&f) {
                        Some(fp) => fp,
                        None => {
                            log::info!("move_app_to_front: could not resolve PID for focused window");
                            return;
                        }
                    };
                    (f, fp)
                }
            }
        } else {
            let f = match get_focused_window() {
                Some(w) => w,
                None => {
                    log::info!("move_app_to_front: no focused window");
                    return;
                }
            };
            let fp = match get_window_pid(&f) {
                Some(fp) => fp,
                None => {
                    log::info!("move_app_to_front: could not resolve PID for focused window");
                    return;
                }
            };
            (f, fp)
        };
        // Activate with NSApplicationActivateAllWindows so macOS raises every
        // window of the app above other apps' windows.
        activate_app_by_pid_with_options(pid, 3);
        // Belt-and-suspenders: explicitly raise each AX window. Some macOS
        // versions ignore NSApplicationActivateAllWindows; AXRaise is reliable.
        for (w, _wid) in get_all_ax_windows_for_pid(pid) {
            raise_window(&w);
        }
        // Re-raise the originally focused window so it remains topmost
        // (AXRaise is "raise within app", so the last raise wins).
        raise_window(&focused);
    }
}

/// Send a window to the absolute bottom of the global z-order via CGSOrderWindow.
/// `order = -1` (kCGSOrderBelow), `relative_to = 0` (absolute, behind everything).
unsafe fn send_window_to_back_by_id(wid: u32) {
    let cid = CGSMainConnectionID();
    let _ = CGSOrderWindow(cid, wid, -1, 0);
}

/// Activate the next visible app (any window from a different PID than
/// `excluded_pid`) so the excluded app loses "active app" status.
///
/// On macOS, every window of the active app sits above every window of every
/// inactive app — that grouping is enforced by the window server. So calling
/// `CGSOrderWindow(below, 0)` on a window of the *active* app only reorders
/// it within that app's windows, leaving it visually on top of all other
/// apps' windows. To genuinely push the user's window behind everything, we
/// also have to activate a different app, which makes the original app
/// inactive and drops all its windows below the newly active one.
///
/// Picks the frontmost window in the global z-order whose PID differs from
/// `excluded_pid`. Returns true if an app was activated, false if no other
/// app has a visible normal-layer window (e.g. only one app is on screen).
unsafe fn activate_next_app_excluding_pid(excluded_pid: i32) -> bool {
    // get_all_windows() returns normal-layer (layer 0), on-screen,
    // non-tiny windows in front-to-back z-order — exactly the candidate
    // set we want for "what should become active instead".
    for w in get_all_windows() {
        if w.owner_pid != excluded_pid && w.owner_pid > 0 {
            log::info!(
                "activate_next_app_excluding_pid: activating pid={} ('{}') wid={}",
                w.owner_pid, w.owner_name, w.window_id,
            );
            activate_app_by_pid(w.owner_pid);
            return true;
        }
    }
    log::info!(
        "activate_next_app_excluding_pid: no other app with a visible window (excluded_pid={})",
        excluded_pid,
    );
    false
}

/// Send the focused window to the back of the global z-order.
///
/// There is no public AX API for "lower window," so we use the private
/// CGS API `CGSOrderWindow` — the standard approach used by yabai,
/// Rectangle, and AeroSpace. The next-frontmost window naturally takes
/// focus once this one is lowered.
///
/// No-op if Accessibility permission isn't granted or there's no focused window.
pub fn move_window_to_back(_app: &AppHandle) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("move_window_to_back: Accessibility permission not granted");
        return;
    }
    unsafe {
        let window = match get_focused_window() {
            Some(w) => w,
            None => {
                log::info!("move_window_to_back: no focused window");
                return;
            }
        };
        let wid = match get_window_id(&window) {
            Some(id) => id,
            None => {
                log::info!("move_window_to_back: could not get CGWindowID");
                return;
            }
        };
        let pid = get_window_pid(&window).unwrap_or(0);
        send_window_to_back_by_id(wid);
        // Lowering alone is not visible if this window's app is the active
        // app on macOS — the active app's windows always sit above every
        // other app's windows. Activate another app to drop this app
        // (and its now-lowered window) into the inactive layer.
        if pid > 0 {
            activate_next_app_excluding_pid(pid);
            // Remember which PID we just sent back so a subsequent
            // moveToFront can bring it back, even though focus has now
            // shifted to the app we just activated.
            remember_backed_pid(pid);
        }
        log::info!(
            "move_window_to_back: CGSOrderWindow sent (wid={}, pid={})",
            wid, pid,
        );
    }
}

/// Send every window of the focused app to the back of the global z-order.
///
/// Iterates `get_all_ax_windows_for_pid` and calls `CGSOrderWindow(below, 0)`
/// on each. Each successive call moves that window to the absolute bottom,
/// so iterating in z-order (frontmost first) leaves the originally
/// frontmost-of-app on top of the bottom-stack — preserving the relative
/// in-app order.
///
/// No-op if Accessibility permission isn't granted or there's no focused window.
pub fn move_app_to_back(_app: &AppHandle) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("move_app_to_back: Accessibility permission not granted");
        return;
    }
    unsafe {
        let focused = match get_focused_window() {
            Some(w) => w,
            None => {
                log::info!("move_app_to_back: no focused window");
                return;
            }
        };
        let pid = match get_window_pid(&focused) {
            Some(p) => p,
            None => {
                log::info!("move_app_to_back: could not resolve PID for focused window");
                return;
            }
        };
        for (_w, wid) in get_all_ax_windows_for_pid(pid) {
            send_window_to_back_by_id(wid);
        }
        // Same reasoning as `move_window_to_back`: lowering is invisible
        // while this app is the active app. Activate another app to push
        // every window of this app into the inactive layer.
        activate_next_app_excluding_pid(pid);
        // Remember which PID we just sent back so a subsequent
        // moveToFront can bring its windows back.
        remember_backed_pid(pid);
        log::info!("move_app_to_back: dispatched (pid={})", pid);
    }
}

/// Check if the focused window is the global topmost window.
///
/// `CGWindowListCopyWindowInfo` (via `get_all_windows()`) returns normal
/// (layer-0) on-screen windows in front-to-back z-order. We compare the
/// focused window's CGWindowID against the first entry.
///
/// `pub(super)` so the shared z-order self-test in `tiling/mod.rs` can read
/// live front/back state when `DISPLAY_DJ_ZORDER_SELFTEST=1`.
pub(super) fn is_focused_window_at_front() -> bool {
    let focused_id = unsafe {
        match get_focused_window().and_then(|w| get_window_id(&w)) {
            Some(id) => id as i64,
            None => return false,
        }
    };
    let z_order: Vec<i64> = get_all_windows().iter().map(|w| w.window_id).collect();
    super::is_window_at_front(focused_id, &z_order)
}

/// Toggle: if the focused window is the global topmost, send it to back;
/// otherwise bring it to front. Stateless — decided per-call from the live
/// z-order, not from saved state.
pub fn toggle_window_front_back(app: &AppHandle) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("toggle_window_front_back: Accessibility permission not granted");
        return;
    }
    if is_focused_window_at_front() {
        move_window_to_back(app);
    } else {
        move_window_to_front(app);
    }
}

/// Toggle: if the focused window is the global topmost, send the whole app
/// to back; otherwise bring the whole app to front.
pub fn toggle_app_front_back(app: &AppHandle) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("toggle_app_front_back: Accessibility permission not granted");
        return;
    }
    if is_focused_window_at_front() {
        move_app_to_back(app);
    } else {
        move_app_to_front(app);
    }
}

/// Set a window rect by PID and CGWindowID. Used by shared plan_* functions.
fn set_window_rect_by_id(pid: i32, wid: u32, rect: &Rect) {
    unsafe {
        if let Some(ax_win) = get_ax_window_by_id(pid, wid) {
            set_window_rect(&ax_win, rect);
            raise_window(&ax_win);
        }
    }
}

/// Find the frontmost normal window at a given cursor position using CGWindowList.
/// Returns (pid, wid) of the topmost window whose bounds contain the cursor.
/// Works even when AX get_focused_window() fails (e.g., during Chromium drags).
fn find_window_at_cursor(cx: f64, cy: f64) -> Option<(i32, u32)> {
    let windows = get_all_windows();
    // CGWindowList returns windows in z-order (front to back).
    // Find the first window whose bounds contain the cursor point.
    for w in &windows {
        let b = &w.bounds;
        if cx >= b.x && cx < b.x + b.width && cy >= b.y && cy < b.y + b.height {
            return Some((w.owner_pid, w.window_id as u32));
        }
    }
    // Fallback: return the frontmost window regardless of cursor position
    windows.first().map(|w| (w.owner_pid, w.window_id as u32))
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

/// Spread all windows into grids using the shared plan_expose logic.
fn spread_expose(app: &AppHandle) {
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

    // Move all windows from other Spaces to the current Space so Exposé
    // covers every window, not just the ones on the active virtual desktop.
    let moved = move_all_windows_to_current_space();
    if moved > 0 {
        log::info!("expose: moved {} windows to current space", moved);
        std::thread::sleep(std::time::Duration::from_millis(300));
    }

    // Normalize: unminimize and un-fullscreen all windows first
    let (unmin, unfs) = normalize_all_windows();
    if unmin > 0 || unfs > 0 {
        log::info!("expose: normalized {} unminimized, {} un-fullscreened", unmin, unfs);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let mut all_windows = get_all_windows();
    if all_windows.is_empty() {
        log::info!("expose: no windows found");
        return;
    }

    // Populate min sizes via AX API for adaptive grid layout
    for w in &mut all_windows {
        unsafe {
            if let Some(ax_win) = get_ax_window_by_id(w.owner_pid, w.window_id as u32) {
                w.min_size = get_window_min_size(&ax_win);
            }
        }
    }

    let displays = get_display_visible_frames();
    if displays.is_empty() {
        return;
    }

    // Brief pause before layout to let space-move and normalization
    // animations finish (windows may still be animating into place).
    std::thread::sleep(std::time::Duration::from_millis(200));

    // macOS NSScreen returns points (logical pixels) — no DPI scaling needed
    let min_cell_sizes: Vec<(f64, f64)> = displays.iter().map(|_| (expose_min_w, expose_min_h)).collect();

    let placements = plan_expose(&all_windows, &displays, max_per_display, gap as f64, spread, &min_cell_sizes);
    for p in &placements {
        set_window_rect_by_id(p.owner_pid, p.window_id as u32, &p.target);
    }
    log::info!("expose: placed {} windows across {} displays", placements.len(), displays.len());
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

/// Get PIDs of all running GUI applications via NSWorkspace.
/// Returns regular activation-policy apps (menu-bar apps, not agents/daemons).
unsafe fn get_all_gui_app_pids() -> Vec<i32> {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let ws_cls = match objc::runtime::Class::get("NSWorkspace") {
        Some(c) => c,
        None => return Vec::new(),
    };
    let workspace: *mut Object = msg_send![ws_cls, sharedWorkspace];
    if workspace.is_null() {
        return Vec::new();
    }
    let apps: *mut Object = msg_send![workspace, runningApplications];
    if apps.is_null() {
        return Vec::new();
    }
    let count: usize = msg_send![apps, count];

    let mut pids = Vec::new();
    for i in 0..count {
        let app: *mut Object = msg_send![apps, objectAtIndex: i];
        if app.is_null() {
            continue;
        }
        // activationPolicy: 0 = Regular (GUI), 1 = Accessory, 2 = Prohibited
        let policy: i64 = msg_send![app, activationPolicy];
        if policy == 0 {
            let pid: i32 = msg_send![app, processIdentifier];
            if pid > 0 {
                pids.push(pid);
            }
        }
    }
    pids
}

// ---------------------------------------------------------------------------
// Spaces (virtual desktops) — private CGS API
// ---------------------------------------------------------------------------

// These private CoreGraphics Server APIs are used by major macOS window
// managers (yabai, Amethyst, AeroSpace) and have been stable since macOS 10.6.
// They let us move windows from other Spaces to the current one before Exposé.
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGSMainConnectionID() -> i32;
    /// Returns the Space ID of the currently active Space on the main display.
    fn CGSGetActiveSpace(cid: i32) -> u64;
    /// Move a set of windows to a managed Space. `windows` is a CFArray of
    /// CFNumber(kCFNumberSInt32Type) window IDs. `space` is the Space ID.
    fn CGSMoveWindowsToManagedSpace(cid: i32, windows: CFArrayRef, space: u64) -> i32;
    /// Change a window's z-order via the window server.
    /// `order`: 1 = above, 0 = remove from screen, -1 = below.
    /// `relative_to`: window ID to be relative to, or 0 for absolute (above
    /// everything / below everything).
    /// There is no public AX API to lower a window — this is the standard
    /// approach used by every macOS tiling WM that supports "send to back".
    fn CGSOrderWindow(cid: i32, wid: u32, order: i32, relative_to: u32) -> i32;
}

/// Move all windows from all running GUI apps to the currently active Space.
/// Uses AX API to enumerate windows across all Spaces, then the private CGS
/// API to move them. Gracefully skips if the private APIs fail.
fn move_all_windows_to_current_space() -> usize {
    unsafe {
        let cid = CGSMainConnectionID();
        if cid == 0 {
            log::warn!("expose: CGSMainConnectionID returned 0 — skipping space collapse");
            return 0;
        }
        let current_space = CGSGetActiveSpace(cid);
        if current_space == 0 {
            log::warn!("expose: CGSGetActiveSpace returned 0 — skipping space collapse");
            return 0;
        }

        let pids = get_all_gui_app_pids();
        let mut moved = 0usize;

        for &pid in &pids {
            let ax_windows = get_all_ax_windows_for_pid(pid);
            for (_ax_win, wid) in &ax_windows {
                // Create a CFArray with one CFNumber (the window ID)
                let wid_val = *wid as i32;
                let cf_num = CFNumberCreate(
                    std::ptr::null(),
                    K_CF_NUMBER_SINT32_TYPE,
                    &wid_val as *const i32 as *const c_void,
                );
                if cf_num.is_null() {
                    continue;
                }
                let arr = CFArrayCreate(
                    std::ptr::null(),
                    &cf_num as *const CFTypeRef,
                    1,
                    std::ptr::null(), // no callbacks needed for CFNumber
                );
                if !arr.is_null() {
                    let err = CGSMoveWindowsToManagedSpace(cid, arr, current_space);
                    if err == 0 {
                        moved += 1;
                    }
                    CFRelease(arr);
                }
                CFRelease(cf_num);
            }
        }

        moved
    }
}

/// Normalize all windows: unminimize minimized windows, exit native fullscreen,
/// and exit browser/video pseudo-fullscreen (by sending Escape key).
/// Returns (unminimized_count, unfullscreened_count).
/// After calling this, the caller should re-fetch the window list since
/// windows may now be visible that weren't before.
fn normalize_all_windows() -> (usize, usize) {
    // Use all running GUI app PIDs so we catch minimized windows and
    // windows on other Spaces (not just on-screen ones).
    let pids = unsafe { get_all_gui_app_pids() };

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

    // Detect pseudo-fullscreen windows (browser F11 / video fullscreen):
    // these cover the full display frame but don't set AXFullScreen.
    // Send Escape key to each to exit the browser/player fullscreen.
    let full_frames = get_display_full_frames();
    if !full_frames.is_empty() {
        let on_screen = get_all_windows();
        let mut pseudo_fs_pids: Vec<i32> = Vec::new();
        for w in &on_screen {
            if is_pseudo_fullscreen(&w.bounds, &full_frames) {
                // Check that this window is NOT native fullscreen (already handled above).
                // A native-fullscreen window has its own Space, so it shouldn't
                // appear in get_all_windows() after being un-fullscreened, but
                // guard against double-action anyway.
                let is_native = unsafe {
                    get_ax_window_by_id(w.owner_pid, w.window_id as u32)
                        .map_or(false, |ax| is_window_fullscreen(&ax))
                };
                if !is_native && !pseudo_fs_pids.contains(&w.owner_pid) {
                    pseudo_fs_pids.push(w.owner_pid);
                }
            }
        }
        for &pid in &pseudo_fs_pids {
            unsafe {
                activate_app_by_pid(pid);
                std::thread::sleep(std::time::Duration::from_millis(100));
                send_escape_key();
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            unfullscreened += 1;
            log::info!("expose: sent Escape to exit pseudo-fullscreen (pid={})", pid);
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
/// App Exposé: target app's windows on first displays, others on remaining.
/// Uses shared plan_expose_app logic.
fn spread_expose_app(app: &AppHandle) {
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

    // Identify the target app before normalization (frontmost window)
    let pre_windows = get_all_windows();
    if pre_windows.is_empty() {
        log::info!("app_expose: no windows found");
        return;
    }
    let target_pid = pre_windows[0].owner_pid;
    let target_app = pre_windows[0].owner_name.clone();

    // Move all windows from other Spaces to the current Space
    let moved = move_all_windows_to_current_space();
    if moved > 0 {
        log::info!("app_expose: moved {} windows to current space", moved);
        std::thread::sleep(std::time::Duration::from_millis(300));
    }

    let (unmin, unfs) = normalize_all_windows();
    if unmin > 0 || unfs > 0 {
        log::info!("app_expose: normalized {} unminimized, {} un-fullscreened", unmin, unfs);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let mut all_windows = get_all_windows();
    if all_windows.is_empty() {
        return;
    }

    // Populate min sizes via AX API for adaptive grid layout
    for w in &mut all_windows {
        unsafe {
            if let Some(ax_win) = get_ax_window_by_id(w.owner_pid, w.window_id as u32) {
                w.min_size = get_window_min_size(&ax_win);
            }
        }
    }

    let displays = get_display_visible_frames();
    if displays.is_empty() {
        return;
    }

    // Brief pause before layout to let space-move and normalization
    // animations finish (windows may still be animating into place).
    std::thread::sleep(std::time::Duration::from_millis(200));

    // macOS NSScreen returns points (logical pixels) — no DPI scaling needed
    let min_cell_sizes: Vec<(f64, f64)> = displays.iter().map(|_| (expose_min_w, expose_min_h)).collect();

    let placements = plan_expose_app(&all_windows, target_pid, &displays, max_per_display, gap as f64, spread, &min_cell_sizes);
    for p in &placements {
        set_window_rect_by_id(p.owner_pid, p.window_id as u32, &p.target);
    }
    log::info!(
        "app_expose: placed {} windows (app '{}') across {} displays",
        placements.len(), target_app, displays.len()
    );
}

// ===========================================================================
// Tile Snap -- mouse edge snapping with preview overlay
// ===========================================================================

// --- CGEvent FFI ---

type DispatchQueue = *mut c_void;

/// NSEventType constants matching AppKit headers.
const NS_EVENT_TYPE_LEFT_MOUSE_DOWN: u64 = 1;
const NS_EVENT_TYPE_LEFT_MOUSE_UP: u64 = 2;
const NS_EVENT_TYPE_LEFT_MOUSE_DRAGGED: u64 = 6;

extern "C" {
    fn dispatch_async_f(
        queue: DispatchQueue,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
    /// `_dispatch_main_q` is the actual symbol for the main dispatch queue.
    /// `dispatch_get_main_queue()` is a macro/inline that returns `&_dispatch_main_q`.
    static _dispatch_main_q: std::ffi::c_char;
}

/// Returns a pointer to the main dispatch queue.
fn dispatch_get_main_queue() -> DispatchQueue {
    unsafe { &_dispatch_main_q as *const _ as DispatchQueue }
}

/// Get the current mouse location in CG coordinates (top-left origin, Y down).
/// Converts from Cocoa coords (bottom-left origin, Y up) using the primary screen height.
#[allow(unexpected_cfgs)]
fn get_mouse_location_cg() -> CGPoint {
    unsafe {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};

        // [NSEvent mouseLocation] — Cocoa coords (Y up from bottom-left)
        let cls = objc::runtime::Class::get("NSEvent").unwrap();
        let cocoa_loc: CGPoint = msg_send![cls, mouseLocation];

        // Get primary screen height for coordinate conversion
        let screen_cls = objc::runtime::Class::get("NSScreen").unwrap();
        let screens: *mut Object = msg_send![screen_cls, screens];
        let count: usize = msg_send![screens, count];
        let primary_h = if count > 0 {
            let screen: *mut Object = msg_send![screens, objectAtIndex: 0usize];
            let frame: CGRect = msg_send![screen, frame];
            frame.size.height
        } else {
            1080.0
        };

        CGPoint {
            x: cocoa_loc.x,
            y: primary_h - cocoa_loc.y,
        }
    }
}

// --- Overlay window (NSWindow, main thread only) ---

/// Commands dispatched to the main thread for overlay window management.
enum OverlayCmd {
    Show { x: f64, y: f64, w: f64, h: f64 },
    Hide,
    /// Show drop zone indicators on all displays. Provides immediate visual
    /// feedback that the event tap is alive. Colors: green=top (maximize),
    /// orange=sides (halves), purple=corners (quarters).
    ShowZones {
        displays: Vec<Rect>,
        side_edge: f64,
        top_edge: f64,
        corner: f64,
    },
    /// Hide all drop zone indicators.
    HideZones,
}

/// Global overlay NSWindow pointer. Only accessed from the main thread.
static OVERLAY_PTR: std::sync::OnceLock<std::sync::atomic::AtomicUsize> =
    std::sync::OnceLock::new();

/// Global zone overlay NSWindow pointers. Created lazily, reused across drags.
static ZONE_PTRS: std::sync::OnceLock<std::sync::Mutex<Vec<usize>>> =
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
    // kCGScreenSaverWindowLevel (1000) — above all normal windows including fullscreen
    let _: () = msg_send![window, setLevel: 1000i64];
    // Click-through: mouse events pass through to windows below
    let _: () = msg_send![window, setIgnoresMouseEvents: true];
    let _: () = msg_send![window, setReleasedWhenClosed: false];

    window as *mut c_void
}

/// Create a borderless, transparent, click-through NSWindow with a custom color.
#[allow(unexpected_cfgs)]
unsafe fn create_colored_overlay(r: f64, g: f64, b: f64, alpha: f64) -> *mut c_void {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

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
    let color_cls = objc::runtime::Class::get("NSColor").unwrap();
    let color: *mut Object = msg_send![color_cls,
        colorWithRed:r green:g blue:b alpha:alpha
    ];
    let _: () = msg_send![window, setBackgroundColor: color];
    let _: () = msg_send![window, setOpaque: false];
    let _: () = msg_send![window, setHasShadow: false];
    let _: () = msg_send![window, setLevel: 999i64]; // just below snap overlay (1000)
    let _: () = msg_send![window, setIgnoresMouseEvents: true];
    let _: () = msg_send![window, setReleasedWhenClosed: false];
    window as *mut c_void
}

/// Show a zone overlay window at the given CG coordinates.
#[allow(unexpected_cfgs)]
unsafe fn show_zone_window(window: *mut c_void, x: f64, y: f64, w: f64, h: f64, primary_h: f64) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let win = window as *mut Object;
    let cocoa_y = primary_h - y - h;
    let frame = CGRect {
        origin: CGPoint { x, y: cocoa_y },
        size: CGSize { width: w, height: h },
    };
    let _: () = msg_send![win, setFrame:frame display:true];
    let _: () = msg_send![win, orderFront:std::ptr::null::<Object>()];
}

/// Hide a zone overlay window.
#[allow(unexpected_cfgs)]
unsafe fn hide_zone_window(window: *mut c_void) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let win = window as *mut Object;
    let _: () = msg_send![win, orderOut:std::ptr::null::<Object>()];
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
            OverlayCmd::ShowZones { displays, side_edge, top_edge, corner } => {
                // Get primary screen height for coordinate conversion
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

                // Build zone rects: per display, create top/left/right/corner zones
                // Colors: green=top (maximize), orange=sides (halves), purple=corners
                struct ZoneRect {
                    x: f64, y: f64, w: f64, h: f64,
                    /// 0=top(green), 1=side(orange), 2=corner(purple), 3=bottom-third(teal)
                    kind: u8,
                }
                let mut zones: Vec<ZoneRect> = Vec::new();
                for d in &displays {
                    // Draw order: edges first, corners last (corners overlay edges).
                    // Corners are simple corner×corner rectangles that cover the
                    // edge strips beneath them, making it clear corners win.

                    // Top strip (green) — full width, top_edge tall
                    zones.push(ZoneRect {
                        x: d.x, y: d.y,
                        w: d.width, h: top_edge,
                        kind: 0,
                    });
                    // Left strip (orange) — side_edge wide, full height
                    zones.push(ZoneRect {
                        x: d.x, y: d.y,
                        w: side_edge, h: d.height,
                        kind: 1,
                    });
                    // Right strip (orange) — side_edge wide, full height
                    zones.push(ZoneRect {
                        x: d.x + d.width - side_edge, y: d.y,
                        w: side_edge, h: d.height,
                        kind: 1,
                    });
                    // Corner rectangles (purple) — drawn last, overlay edges
                    // Top-left
                    zones.push(ZoneRect {
                        x: d.x, y: d.y,
                        w: corner, h: corner,
                        kind: 2,
                    });
                    // Top-right
                    zones.push(ZoneRect {
                        x: d.x + d.width - corner, y: d.y,
                        w: corner, h: corner,
                        kind: 2,
                    });
                    // Bottom-left
                    zones.push(ZoneRect {
                        x: d.x, y: d.y + d.height - corner,
                        w: corner, h: corner,
                        kind: 2,
                    });
                    // Bottom-right
                    zones.push(ZoneRect {
                        x: d.x + d.width - corner, y: d.y + d.height - corner,
                        w: corner, h: corner,
                        kind: 2,
                    });

                    // Bottom-third zones (teal) — small rectangles at 25%, 50%,
                    // 75% horizontal offsets on the bottom edge. Match the
                    // hit-test rects in `build_snap_zones`: width=corner,
                    // height=top_edge × 4/3 (a third taller for an easier
                    // hit target), centered on each offset.
                    let third_w = corner;
                    let third_h = top_edge * 4.0 / 3.0;
                    let bottom_y = d.y + d.height - third_h;
                    for offset in &[0.25, 0.50, 0.75] {
                        let cx = d.x + d.width * offset;
                        zones.push(ZoneRect {
                            x: cx - third_w / 2.0, y: bottom_y,
                            w: third_w, h: third_h,
                            kind: 3,
                        });
                    }
                }

                // Ensure we have enough zone windows, creating new ones as needed
                let zone_ptrs = ZONE_PTRS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
                let mut ptrs = zone_ptrs.lock().unwrap();
                while ptrs.len() < zones.len() {
                    // Create windows for each kind with different colors
                    // We don't know the kind yet, so create neutral ones and set color per-use
                    ptrs.push(0); // placeholder
                }

                // Create/reuse windows and position them
                for (i, zone) in zones.iter().enumerate() {
                    if ptrs[i] == 0 {
                        let (r, g, b) = match zone.kind {
                            0 => (0.2, 0.8, 0.3), // green for top/maximize
                            1 => (1.0, 0.6, 0.1), // orange for sides
                            2 => (0.6, 0.3, 0.9), // purple for corners
                            _ => (0.0, 0.75, 0.7), // teal for bottom thirds
                        };
                        ptrs[i] = create_colored_overlay(r, g, b, 0.25) as usize;
                    }
                    show_zone_window(
                        ptrs[i] as *mut c_void,
                        zone.x, zone.y, zone.w, zone.h,
                        primary_h,
                    );
                }
                // Hide any extra windows from a previous ShowZones with more zones
                for i in zones.len()..ptrs.len() {
                    if ptrs[i] != 0 {
                        hide_zone_window(ptrs[i] as *mut c_void);
                    }
                }
            }
            OverlayCmd::HideZones => {
                let zone_ptrs = ZONE_PTRS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
                if let Ok(ptrs) = zone_ptrs.lock() {
                    for &p in ptrs.iter() {
                        if p != 0 {
                            hide_zone_window(p as *mut c_void);
                        }
                    }
                }
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
/// Build all snap zone rectangles for all displays. Each zone is a named
/// rectangle — cursor is either inside it or not. No clamping, no deltas,
/// no shared-edge math. Corners first (higher priority), then edges.
/// This produces the exact same rectangles drawn by the drop zone indicators.
fn build_snap_zones(
    displays: &[Rect],
    side_edge: f64,
    top_edge: f64,
    corner: f64,
) -> Vec<(Rect, TilingLayout, usize)> {
    let mut zones = Vec::new();
    for (i, d) in displays.iter().enumerate() {
        // Corners first — checked before edges so they win overlaps
        zones.push((Rect { x: d.x, y: d.y, width: corner, height: corner },
            TilingLayout::TopLeftQuarter, i));
        zones.push((Rect { x: d.x + d.width - corner, y: d.y, width: corner, height: corner },
            TilingLayout::TopRightQuarter, i));
        zones.push((Rect { x: d.x, y: d.y + d.height - corner, width: corner, height: corner },
            TilingLayout::BottomLeftQuarter, i));
        zones.push((Rect { x: d.x + d.width - corner, y: d.y + d.height - corner, width: corner, height: corner },
            TilingLayout::BottomRightQuarter, i));
        // Top edge (full width)
        zones.push((Rect { x: d.x, y: d.y, width: d.width, height: top_edge },
            TilingLayout::Maximize, i));
        // Left edge (full height)
        zones.push((Rect { x: d.x, y: d.y, width: side_edge, height: d.height },
            TilingLayout::LeftHalf, i));
        // Right edge (full height)
        zones.push((Rect { x: d.x + d.width - side_edge, y: d.y, width: side_edge, height: d.height },
            TilingLayout::RightHalf, i));
        // Bottom-third zones — small rectangles at 25%, 50%, 75% horizontal
        // offsets along the bottom edge. Each is `corner` wide × (top_edge × 4/3)
        // tall (33% taller than the top strip so the bottom drop targets are
        // easier to hit), centered on its offset. They don't overlap bottom
        // corners (offsets > corner+corner/2 for any reasonable display), but
        // corners are listed first anyway so they win priority.
        let third_w = corner;
        let third_h = top_edge * 4.0 / 3.0;
        let bottom_y = d.y + d.height - third_h;
        for &(offset, layout) in &[
            (0.25, TilingLayout::LeftThird),
            (0.50, TilingLayout::CenterThird),
            (0.75, TilingLayout::RightThird),
        ] {
            let cx = d.x + d.width * offset;
            zones.push((Rect {
                x: cx - third_w / 2.0,
                y: bottom_y,
                width: third_w,
                height: third_h,
            }, layout, i));
        }
    }
    zones
}

/// Detect which snap zone the cursor is in. Checks if the cursor point
/// is inside any zone rectangle. First match wins — corners are listed
/// before edges so they take priority at overlapping areas.
fn detect_snap_zone_macos(
    cx: f64,
    cy: f64,
    displays: &[Rect],
    side_edge: f64,
    top_edge: f64,
    corner: f64,
) -> Option<(TilingLayout, usize)> {
    let zones = build_snap_zones(displays, side_edge, top_edge, corner);
    for (rect, layout, display_idx) in &zones {
        if cx >= rect.x && cx < rect.x + rect.width
            && cy >= rect.y && cy < rect.y + rect.height
        {
            return Some((*layout, *display_idx));
        }
    }
    None
}

// --- Aero snap state and event tap ---

/// Shared state between the NSEvent global monitor handler and the main app.
struct SnapContext {
    app: AppHandle,
    state: std::sync::Mutex<SnapState>,
}

// Safety: AppHandle is Send+Sync. SnapState is behind a Mutex.
unsafe impl Send for SnapContext {}
unsafe impl Sync for SnapContext {}

/// Mutable state tracked during a drag for snap zone detection.
struct SnapState {
    dragging: bool,
    /// Whether the drag has moved more than the confirmation threshold (10px).
    /// Until confirmed, snap zone detection is skipped.
    drag_confirmed: bool,
    /// Whether the focused window's position has changed since drag start.
    /// Only true for title-bar drags (window moves). False for resizes,
    /// content drags, or clicks. Snap zones only activate when this is true.
    window_is_moving: bool,
    /// Cursor position at mouse_down — used for the drag confirmation threshold.
    drag_start_cursor: Option<(f64, f64)>,
    /// Window position and size at mouse_down — captured lazily on first confirmed drag
    /// to keep the mouse_down handler fast (avoids AX API calls in the callback).
    /// Position is used for move detection; size is used to distinguish moves from resizes.
    drag_start_window_pos: Option<(f64, f64)>,
    drag_start_window_size: Option<(f64, f64)>,
    /// Window title captured at drag_confirmed for debug logging.
    drag_window_title: String,
    current_layout: Option<TilingLayout>,
    current_display: usize,
    /// The pre-calculated target rect for the current snap zone.
    /// Set when the overlay is shown, used directly on mouse_up to
    /// move the window — no re-detection or recalculation needed.
    current_target_rect: Option<Rect>,
    displays: Vec<Rect>,
    half_ratio: u32,
    third_ratio: u32,
    gap: u32,
    side_edge_trigger: f64,
    top_edge_trigger: f64,
    corner_trigger: f64,
    /// Last cursor position logged during drag (throttle: only log when
    /// cursor moves ≥50px from last logged position).
    last_log_cursor: Option<(f64, f64)>,
}

/// Determine if a window drag is a title-bar move (not a resize or content drag).
/// Returns true when the window position changed by more than 5px.
///
/// Previous versions also checked that the window size stayed the same, but
/// Chromium-based browsers (Chrome, Brave, Edge) change the window size during
/// title-bar drags (un-maximize, tab tear-off, DPI transitions). The size
/// check caused false negatives for these browsers. Since the 10px drag
/// confirmation threshold + snap zone geometry already prevent false positives,
/// checking position alone is sufficient.
fn is_window_move(
    start_pos: (f64, f64),
    _start_size: Option<(f64, f64)>,
    cur_rect: &Rect,
) -> bool {
    let pos_dx = (cur_rect.x - start_pos.0).abs();
    let pos_dy = (cur_rect.y - start_pos.1).abs();
    pos_dx > 5.0 || pos_dy > 5.0
}

/// Handle a mouse event from the NSEvent global monitor.
/// Called on the main thread by AppKit. Must stay fast — spawn threads
/// for any AX API calls.
fn handle_snap_event(ctx: &SnapContext, event_type: u64, cursor: CGPoint) {
    // Check if tiling and tile snap are both enabled.
    let snap_enabled = ctx
        .app
        .try_state::<crate::AppState>()
        .and_then(|s| {
            s.preferences
                .try_lock()
                .ok()
                .map(|p| p.tiling.enabled && p.tiling.tile_snap_enabled)
        })
        .unwrap_or(false); // default to disabled if lock is contended
    if !snap_enabled {
        return;
    }

    let mut state = match ctx.state.try_lock() {
        Ok(s) => s,
        Err(_) => return, // skip if contended
    };

    match event_type {
        NS_EVENT_TYPE_LEFT_MOUSE_DOWN => {
            state.dragging = true;
            state.drag_confirmed = false;
            state.window_is_moving = false;
            state.drag_start_cursor = Some((cursor.x, cursor.y));
            state.drag_start_window_pos = None;
            state.drag_start_window_size = None;
            state.drag_window_title = String::new();
            state.current_layout = None;
            state.current_target_rect = None;
            state.last_log_cursor = None;
        }

        NS_EVENT_TYPE_LEFT_MOUSE_DRAGGED => {
            if !state.dragging {
                return;
            }

            // 10px movement threshold before confirming this is a real drag.
            if !state.drag_confirmed {
                if let Some((sx, sy)) = state.drag_start_cursor {
                    let dx = (cursor.x - sx).abs();
                    let dy = (cursor.y - sy).abs();
                    if dx < 10.0 && dy < 10.0 {
                        return; // not a real drag yet
                    }
                } else {
                    return;
                }

                // Drag confirmed — lazy-load display frames, preferences, and
                // window position.
                state.drag_confirmed = true;
                let (start_rect, win_info) = unsafe {
                    let w = get_focused_window();
                    let r = w.as_ref().and_then(|w| get_window_rect(w));
                    let info = w.as_ref().map_or(String::from("?"), |w| get_window_debug_info(w));
                    (r, info)
                };
                state.drag_start_window_pos = start_rect.as_ref().map(|r| (r.x, r.y));
                state.drag_start_window_size = start_rect.as_ref().map(|r| (r.width, r.height));
                state.drag_window_title = win_info.clone();
                state.displays = get_display_visible_frames();
                let prefs = ctx
                    .app
                    .try_state::<crate::AppState>()
                    .and_then(|s| s.preferences.try_lock().ok().map(|p| p.tiling.clone()));
                if let Some(tp) = prefs {
                    state.half_ratio = tp.half_ratio;
                    state.third_ratio = tp.third_ratio;
                    state.gap = tp.gap;
                    state.side_edge_trigger = tp.side_edge_trigger as f64;
                    state.top_edge_trigger = tp.top_edge_trigger as f64;
                    state.corner_trigger = tp.corner_trigger as f64;
                }
                if let Some(dbg_state) = ctx.app.try_state::<crate::AppState>() {
                    let display_info: Vec<String> = state.displays.iter().enumerate().map(|(i, d)| {
                        format!("D{}({:.0},{:.0} {:.0}x{:.0})", i, d.x, d.y, d.width, d.height)
                    }).collect();
                    crate::config::write_debug_log(
                        &dbg_state,
                        &format!(
                            "tile_snap: drag_confirmed — window={}, cursor=({:.0},{:.0}), \
                             start_pos={:?}, start_size={:?}, displays=[{}], \
                             edge_triggers=(side={:.0}, top={:.0}, corner={:.0}), \
                             tiling=(half={}, third={}, gap={})",
                            win_info,
                            cursor.x, cursor.y,
                            state.drag_start_window_pos,
                            state.drag_start_window_size,
                            display_info.join(", "),
                            state.side_edge_trigger, state.top_edge_trigger, state.corner_trigger,
                            state.half_ratio, state.third_ratio, state.gap,
                        ),
                    );
                }
            }

            if state.displays.is_empty() {
                return;
            }

            // Check if the window is actually moving (title bar drag) vs
            // resizing or content drag. Only activate snap for window moves.
            if !state.window_is_moving {
                if let Some((sx, sy)) = state.drag_start_window_pos {
                    let cur_rect = unsafe {
                        get_focused_window().and_then(|w| get_window_rect(&w))
                    };
                    if let Some(r) = cur_rect {
                        if is_window_move((sx, sy), state.drag_start_window_size, &r) {
                            state.window_is_moving = true;
                            if let Some(dbg_state) = ctx.app.try_state::<crate::AppState>() {
                                crate::config::write_debug_log(
                                    &dbg_state,
                                    &format!(
                                        "tile_snap: window_is_moving — start=({:.0},{:.0}), now=({:.0},{:.0} {:.0}x{:.0})",
                                        sx, sy, r.x, r.y, r.width, r.height,
                                    ),
                                );
                            }

                            // Now show drop zone indicators
                            dispatch_overlay(OverlayCmd::ShowZones {
                                displays: state.displays.clone(),
                                side_edge: state.side_edge_trigger,
                                top_edge: state.top_edge_trigger,
                                corner: state.corner_trigger,
                            });
                        }
                    }
                } else {
                    // No start position captured (e.g., no focused window).
                    // Assume it's a window move if we got this far.
                    state.window_is_moving = true;
                    dispatch_overlay(OverlayCmd::ShowZones {
                        displays: state.displays.clone(),
                        side_edge: state.side_edge_trigger,
                        top_edge: state.top_edge_trigger,
                        corner: state.corner_trigger,
                    });
                }
                if !state.window_is_moving {
                    return; // not a window move (resize or content drag) — skip snap
                }
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
                    if state.current_layout != Some(layout) || state.current_display != display_idx
                    {
                        if let Some(dbg_state) = ctx.app.try_state::<crate::AppState>() {
                            let d = &state.displays[display_idx];
                            crate::config::write_debug_log(
                                &dbg_state,
                                &format!(
                                    "tile_snap: zone detected — layout={:?}, display={} ({:.0},{:.0} {:.0}x{:.0}), cursor=({:.0},{:.0})",
                                    layout, display_idx, d.x, d.y, d.width, d.height, cursor.x, cursor.y,
                                ),
                            );
                        }
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
                        state.current_target_rect = Some(target);
                    }
                }
                None => {
                    // Clear snap when cursor leaves the zone. The user must
                    // release inside a zone for the snap to apply. This
                    // prevents re-snapping when dragging a window OUT of a
                    // snapped position (e.g., dragging a maximized window
                    // down — cursor starts in the top zone but should not
                    // re-maximize on release).
                    if state.current_layout.is_some() {
                        state.current_layout = None;
                        state.current_target_rect = None;
                        dispatch_overlay(OverlayCmd::Hide);
                    }
                }
            }

            // Throttled drag position logging: only log when cursor moved ≥50px
            // from last logged position. Helps debug without spamming.
            let should_log_pos = state.last_log_cursor.map_or(true, |(lx, ly)| {
                (cursor.x - lx).abs() >= 50.0 || (cursor.y - ly).abs() >= 50.0
            });
            if should_log_pos {
                state.last_log_cursor = Some((cursor.x, cursor.y));
                if let Some(dbg_state) = ctx.app.try_state::<crate::AppState>() {
                    crate::config::write_debug_log(
                        &dbg_state,
                        &format!(
                            "tile_snap: dragging {} — cursor=({:.0},{:.0}), zone={:?}",
                            state.drag_window_title,
                            cursor.x, cursor.y,
                            state.current_layout,
                        ),
                    );
                }
            }
        }

        NS_EVENT_TYPE_LEFT_MOUSE_UP => {
            let target_rect = state.current_target_rect.clone();
            let layout = state.current_layout;
            let display_idx = state.current_display;
            let win_title = state.drag_window_title.clone();
            state.dragging = false;
            state.drag_confirmed = false;
            state.window_is_moving = false;
            state.current_layout = None;
            state.current_target_rect = None;

            // Capture the focused window BEFORE hiding overlays.
            // Chromium browsers may make the focused window temporarily
            // unavailable during drag. Retry up to 3 times with short delays.
            let focused = unsafe {
                let mut win = get_focused_window();
                if win.is_none() {
                    for _ in 0..3 {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        win = get_focused_window();
                        if win.is_some() {
                            break;
                        }
                    }
                }
                win
            };

            // Hide overlay and zone indicators
            dispatch_overlay(OverlayCmd::Hide);
            dispatch_overlay(OverlayCmd::HideZones);

            // If we have a target rect from the zone detection, just move
            // the window there directly. No re-detection, no recalculation.
            // The target rect is exactly what the overlay preview showed.
            if let (Some(rect), Some(layout)) = (target_rect, layout) {
                if let Some(dbg_state) = ctx.app.try_state::<crate::AppState>() {
                    crate::config::write_debug_log(
                        &dbg_state,
                        &format!(
                            "tile_snap: snapping {} — layout={:?}, display={}, target=({:.0},{:.0} {:.0}x{:.0}), cursor=({:.0},{:.0})",
                            win_title, layout, display_idx, rect.x, rect.y, rect.width, rect.height, cursor.x, cursor.y,
                        ),
                    );
                }
                // Apply the snap using the pre-captured window ref, or fall
                // back to finding the window at the cursor via CGWindowList
                // (works when AX get_focused_window fails during Chromium drags).
                if let Some(ref window) = focused {
                    unsafe {
                        set_window_rect(window, &rect);
                    }
                } else {
                    // Fallback: find the window at the cursor via CGWindowList
                    // and use set_window_rect_by_id (PID + CGWindowID).
                    if let Some((pid, wid)) = find_window_at_cursor(cursor.x, cursor.y) {
                        if let Some(dbg_state) = ctx.app.try_state::<crate::AppState>() {
                            crate::config::write_debug_log(
                                &dbg_state,
                                &format!(
                                    "tile_snap: snap {} via CGWindowList fallback — pid={}, wid={}",
                                    win_title, pid, wid,
                                ),
                            );
                        }
                        set_window_rect_by_id(pid, wid, &rect);
                    } else {
                        if let Some(dbg_state) = ctx.app.try_state::<crate::AppState>() {
                            crate::config::write_debug_log(
                                &dbg_state,
                                &format!(
                                    "tile_snap: FAILED snap {} — no window found at cursor or via AX API",
                                    win_title,
                                ),
                            );
                        }
                    }
                }
            } else {
                if let Some(dbg_state) = ctx.app.try_state::<crate::AppState>() {
                    crate::config::write_debug_log(
                        &dbg_state,
                        &format!("tile_snap: mouse_up {} — no zone active, cursor=({:.0},{:.0})", win_title, cursor.x, cursor.y),
                    );
                }
            }
        }

        _ => {}
    }
}

/// Execute a layout preset by name or index. Enumerates windows, matches by
/// app name, and tiles each matched window according to the preset's rules.
/// Apply a layout preset using shared plan_layout_preset logic.
pub fn execute_layout_preset(app: &AppHandle, name_or_index: &str) {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("layout_preset: Accessibility permission not granted");
        return;
    }

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

    let displays = get_display_visible_frames();
    if displays.is_empty() {
        log::warn!("layout_preset: no displays found");
        return;
    }

    let placements = plan_layout_preset(&windows, &preset, &displays, half_ratio, third_ratio, gap);
    log::info!("layout_preset: '{}' placing {} windows", preset.name, placements.len());
    for p in &placements {
        set_window_rect_by_id(p.owner_pid, p.window_id as u32, &p.target);
    }
}

/// Start the tile snap NSEvent global monitor.
/// Call once during app setup. Requires Accessibility permission for tiling
/// actions (AX API), but the event monitor itself works without it.
/// Must be dispatched to the main thread (Cocoa requirement).
#[allow(unexpected_cfgs)]
pub fn start_tile_snap(app: AppHandle) {
    let trusted = unsafe { AXIsProcessTrusted() };
    let displays = get_display_visible_frames();
    let display_info: Vec<String> = displays.iter().enumerate().map(|(i, d)| {
        format!("D{}({:.0},{:.0} {:.0}x{:.0})", i, d.x, d.y, d.width, d.height)
    }).collect();
    // Log to debug file so bundled app failures are visible
    if let Some(state) = app.try_state::<crate::AppState>() {
        let prefs_info = state.preferences.lock().ok().map(|p| {
            format!(
                "tiling.enabled={}, snap_enabled={}, side={}, top={}, corner={}",
                p.tiling.enabled, p.tiling.tile_snap_enabled,
                p.tiling.side_edge_trigger, p.tiling.top_edge_trigger, p.tiling.corner_trigger,
            )
        }).unwrap_or_else(|| "prefs lock failed".into());
        crate::config::write_debug_log(
            &state,
            &format!(
                "tile_snap: starting (NSEvent) — AXIsProcessTrusted={}, build={}, displays=[{}], {}",
                trusted, env!("IS_DEV_BUILD"), display_info.join(", "), prefs_info,
            ),
        );
    }
    if !trusted {
        log::warn!("tile_snap: Accessibility permission not granted, skipping");
        return;
    }

    // Create overlay window on the main thread
    init_overlay_on_main_thread();

    let ctx = std::sync::Arc::new(SnapContext {
        app,
        state: std::sync::Mutex::new(SnapState {
            dragging: false,
            drag_confirmed: false,
            window_is_moving: false,
            drag_start_cursor: None,
            drag_start_window_pos: None,
            drag_start_window_size: None,
            current_layout: None,
            current_display: 0,
            current_target_rect: None,
            displays: displays.clone(),
            half_ratio: 50,
            third_ratio: 33,
            gap: 0,
            side_edge_trigger: 18.0,
            top_edge_trigger: 18.0,
            corner_trigger: 30.0,
            drag_window_title: String::new(),
            last_log_cursor: None,
        }),
    });

    // Register the NSEvent global monitor on the main thread.
    // The handler closure runs on the main thread automatically by AppKit.
    // We use dispatch_async_f to ensure registration happens on the main thread.
    let ctx_for_registration = ctx.clone();
    let raw = std::sync::Arc::into_raw(ctx_for_registration) as usize;

    extern "C" fn register_monitor(raw_ptr: *mut c_void) {
        let raw = raw_ptr as usize;
        let ctx = unsafe { std::sync::Arc::from_raw(raw as *const SnapContext) };

        // Log that we're registering
        if let Some(state) = ctx.app.try_state::<crate::AppState>() {
            crate::config::write_debug_log(
                &state,
                "tile_snap: registering NSEvent global monitor on main thread",
            );
        }

        unsafe {
            use objc::runtime::Object;
            use objc::{msg_send, sel, sel_impl};

            let mask: u64 = (1 << NS_EVENT_TYPE_LEFT_MOUSE_DOWN)
                | (1 << NS_EVENT_TYPE_LEFT_MOUSE_UP)
                | (1 << NS_EVENT_TYPE_LEFT_MOUSE_DRAGGED);

            // Create an Objective-C block that calls our Rust handler.
            let ctx_for_block = ctx.clone();
            let handler = block::ConcreteBlock::new(move |event: *mut Object| {
                // Wrap in catch_unwind: panics cannot unwind through the
                // Objective-C block boundary (abort would crash the app).
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Get event type: [event type] returns NSEventType (NSUInteger).
                    // Use Sel::register("type") + raw objc_msgSend because `type`
                    // is a Rust keyword and msg_send![event, r#type] causes issues.
                    extern "C" {
                        fn objc_msgSend(obj: *mut Object, sel: objc::runtime::Sel) -> usize;
                    }
                    let type_sel = objc::runtime::Sel::register("type");
                    let event_type = objc_msgSend(event, type_sel) as u64;
                    // Get cursor in CG coordinates (top-left origin)
                    let cursor = get_mouse_location_cg();
                    handle_snap_event(&ctx_for_block, event_type, cursor);
                }));
            });
            let handler = handler.copy(); // heap-allocate so it outlives this scope

            let cls = objc::runtime::Class::get("NSEvent").unwrap();
            let monitor: *mut Object = msg_send![
                cls,
                addGlobalMonitorForEventsMatchingMask: mask
                handler: &*handler
            ];

            if monitor.is_null() {
                log::warn!("tile_snap: NSEvent addGlobalMonitorForEventsMatchingMask returned nil");
                if let Some(state) = ctx.app.try_state::<crate::AppState>() {
                    crate::config::write_debug_log(
                        &state,
                        "tile_snap: NSEvent monitor registration failed — returned nil",
                    );
                }
            } else {
                log::info!("tile_snap: NSEvent global monitor registered successfully");
                if let Some(state) = ctx.app.try_state::<crate::AppState>() {
                    let exe_path = std::env::current_exe()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| "unknown".into());
                    let is_app_bundle = exe_path.contains(".app/Contents/MacOS");
                    crate::config::write_debug_log(
                        &state,
                        &format!(
                            "tile_snap: NSEvent monitor active — exe={}, bundle={}, mask=0x{:X}",
                            exe_path, is_app_bundle, mask,
                        ),
                    );
                }
            }

            // Leak the handler and monitor — they live for the app's lifetime.
            // The handler RcBlock must stay alive as long as the monitor is active.
            std::mem::forget(handler);
            std::mem::forget(monitor);
        }

        // Leak the Arc so the SnapContext lives for the app's lifetime.
        std::mem::forget(ctx);
    }

    unsafe {
        dispatch_async_f(
            dispatch_get_main_queue(),
            raw as *mut c_void,
            register_monitor,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a Rect for tests.
    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, width: w, height: h }
    }

    // --- is_window_move tests ---

    /// Title bar drag: position moved, size unchanged → true.
    #[test]
    fn test_is_window_move_title_bar_drag() {
        let start_pos = (100.0, 200.0);
        let start_size = Some((800.0, 600.0));
        let cur = rect(150.0, 250.0, 800.0, 600.0);
        assert!(is_window_move(start_pos, start_size, &cur));
    }

    /// Resize from bottom/right: position same, size changed → false.
    #[test]
    fn test_is_window_move_resize_bottom_right() {
        let start_pos = (100.0, 200.0);
        let start_size = Some((800.0, 600.0));
        let cur = rect(100.0, 200.0, 900.0, 700.0);
        assert!(!is_window_move(start_pos, start_size, &cur));
    }

    /// Resize from top edge: position changed >5px → true (size check removed
    /// to support Chromium browsers that change size during title-bar drags).
    #[test]
    fn test_is_window_move_resize_top_edge() {
        let start_pos = (100.0, 200.0);
        let start_size = Some((800.0, 600.0));
        let cur = rect(100.0, 150.0, 800.0, 650.0);
        assert!(is_window_move(start_pos, start_size, &cur));
    }

    /// Resize from left edge: position changed >5px → true.
    #[test]
    fn test_is_window_move_resize_left_edge() {
        let start_pos = (100.0, 200.0);
        let start_size = Some((800.0, 600.0));
        let cur = rect(50.0, 200.0, 850.0, 600.0);
        assert!(is_window_move(start_pos, start_size, &cur));
    }

    /// Resize from top-left corner: position changed >5px → true.
    #[test]
    fn test_is_window_move_resize_top_left_corner() {
        let start_pos = (100.0, 200.0);
        let start_size = Some((800.0, 600.0));
        let cur = rect(50.0, 150.0, 850.0, 650.0);
        assert!(is_window_move(start_pos, start_size, &cur));
    }

    /// Content drag or click: no position or size change → false.
    #[test]
    fn test_is_window_move_content_drag() {
        let start_pos = (100.0, 200.0);
        let start_size = Some((800.0, 600.0));
        let cur = rect(100.0, 200.0, 800.0, 600.0);
        assert!(!is_window_move(start_pos, start_size, &cur));
    }

    /// Tiny jitter within threshold: position moved < 5px → false.
    #[test]
    fn test_is_window_move_tiny_jitter() {
        let start_pos = (100.0, 200.0);
        let start_size = Some((800.0, 600.0));
        let cur = rect(103.0, 202.0, 800.0, 600.0);
        assert!(!is_window_move(start_pos, start_size, &cur));
    }

    /// No start size captured: position changed → true (assumes move).
    #[test]
    fn test_is_window_move_no_start_size() {
        let start_pos = (100.0, 200.0);
        let cur = rect(200.0, 300.0, 800.0, 600.0);
        assert!(is_window_move(start_pos, None, &cur));
    }

    /// Browser un-maximize: position changed + both dims shrunk → true.
    /// Chrome/Brave un-maximize the window when dragging a maximized title bar,
    /// changing both position and size. This should count as a move.
    #[test]
    fn test_is_window_move_browser_unmaximize() {
        // Window was maximized at (0, 0, 2560, 1440), now shrunk during drag
        let start_pos = (0.0, 0.0);
        let start_size = Some((2560.0, 1440.0));
        let cur = rect(200.0, 100.0, 1200.0, 800.0);
        assert!(is_window_move(start_pos, start_size, &cur));
    }

    /// Edge resize that grows one dim: position changed >5px → true.
    #[test]
    fn test_is_window_move_resize_grows_width() {
        let start_pos = (100.0, 200.0);
        let start_size = Some((800.0, 600.0));
        let cur = rect(50.0, 200.0, 900.0, 550.0);
        assert!(is_window_move(start_pos, start_size, &cur));
    }

    /// Tiny size change within threshold (3px): treated as move, not resize.
    #[test]
    fn test_is_window_move_tiny_size_jitter() {
        let start_pos = (100.0, 200.0);
        let start_size = Some((800.0, 600.0));
        // Position moved significantly, size changed by only 2px (within 3px threshold)
        let cur = rect(200.0, 300.0, 802.0, 601.0);
        assert!(is_window_move(start_pos, start_size, &cur));
    }

    // --- detect_snap_zone_macos tests (simple rectangle hit-test) ---

    /// Three side-by-side monitors matching the user's setup.
    fn three_monitors() -> Vec<Rect> {
        vec![
            rect(-5120.0, -80.0, 2560.0, 1409.0), // D0: left
            rect(-2560.0, -80.0, 2560.0, 1409.0), // D1: center
            rect(0.0, 40.0, 2056.0, 1289.0),      // D2: right
        ]
    }

    /// Left edge of D0 → LeftHalf on D0.
    #[test]
    fn test_snap_zone_left_edge_d0() {
        let displays = three_monitors();
        let result = detect_snap_zone_macos(-5115.0, 500.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::LeftHalf, 0)));
    }

    /// Right edge of D0 (inside D0 bounds) → RightHalf on D0.
    #[test]
    fn test_snap_zone_right_edge_d0() {
        let displays = three_monitors();
        let result = detect_snap_zone_macos(-2565.0, 500.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::RightHalf, 0)));
    }

    /// At exact boundary x=-2560, cursor belongs to D1 → LeftHalf on D1.
    #[test]
    fn test_snap_zone_at_boundary_is_next_display() {
        let displays = three_monitors();
        let result = detect_snap_zone_macos(-2560.0, 500.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::LeftHalf, 1)));
    }

    /// Right edge of D1 (inside D1 bounds) → RightHalf on D1.
    #[test]
    fn test_snap_zone_right_edge_d1() {
        let displays = three_monitors();
        let result = detect_snap_zone_macos(-5.0, 500.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::RightHalf, 1)));
    }

    /// Right edge of D2 (last pixel inside bounds).
    #[test]
    fn test_snap_zone_right_edge_d2() {
        let displays = three_monitors();
        let result = detect_snap_zone_macos(2050.0, 500.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::RightHalf, 2)));
    }

    /// Top-left corner of D0 → TopLeftQuarter (corners before edges).
    #[test]
    fn test_snap_zone_corner_priority() {
        let displays = three_monitors();
        let result = detect_snap_zone_macos(-5100.0, -60.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::TopLeftQuarter, 0)));
    }

    /// Top edge center of D1 → Maximize.
    #[test]
    fn test_snap_zone_top_center_maximize() {
        let displays = three_monitors();
        let result = detect_snap_zone_macos(-1280.0, -75.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::Maximize, 1)));
    }

    /// Center of D1 → no snap zone.
    #[test]
    fn test_snap_zone_center_no_zone() {
        let displays = three_monitors();
        let result = detect_snap_zone_macos(-1280.0, 500.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, None);
    }

    /// Vertical monitors — top edge of bottom monitor.
    #[test]
    fn test_snap_zone_vertical_top_of_bottom() {
        let displays = vec![
            rect(0.0, 0.0, 2560.0, 1440.0),
            rect(0.0, 1440.0, 2560.0, 1440.0),
        ];
        let result = detect_snap_zone_macos(1280.0, 1445.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::Maximize, 1)));
    }

    /// Bottom-left corner of top monitor (vertical setup).
    #[test]
    fn test_snap_zone_vertical_bottom_left_corner() {
        let displays = vec![
            rect(0.0, 0.0, 2560.0, 1440.0),
            rect(0.0, 1440.0, 2560.0, 1440.0),
        ];
        let result = detect_snap_zone_macos(5.0, 1420.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::BottomLeftQuarter, 0)));
    }

    // --- build_snap_zones tests ---

    /// Zone rectangles match the visual drop zone indicators exactly.
    #[test]
    fn test_build_snap_zones_single_display() {
        let displays = vec![rect(0.0, 0.0, 1920.0, 1080.0)];
        let zones = build_snap_zones(&displays, 18.0, 18.0, 50.0);
        // 4 corners + 3 edges + 3 bottom thirds = 10 zones per display
        assert_eq!(zones.len(), 10);
        // First 4 are corners
        assert_eq!(zones[0].1, TilingLayout::TopLeftQuarter);
        assert_eq!(zones[1].1, TilingLayout::TopRightQuarter);
        assert_eq!(zones[2].1, TilingLayout::BottomLeftQuarter);
        assert_eq!(zones[3].1, TilingLayout::BottomRightQuarter);
        // Then edges
        assert_eq!(zones[4].1, TilingLayout::Maximize);
        assert_eq!(zones[5].1, TilingLayout::LeftHalf);
        assert_eq!(zones[6].1, TilingLayout::RightHalf);
        // Then bottom thirds (25%, 50%, 75%)
        assert_eq!(zones[7].1, TilingLayout::LeftThird);
        assert_eq!(zones[8].1, TilingLayout::CenterThird);
        assert_eq!(zones[9].1, TilingLayout::RightThird);
    }

    /// Three monitors produce 30 zones (10 per display).
    #[test]
    fn test_build_snap_zones_three_displays() {
        let displays = three_monitors();
        let zones = build_snap_zones(&displays, 18.0, 18.0, 50.0);
        assert_eq!(zones.len(), 30);
    }

    /// Bottom-third zones are positioned at 25%, 50%, 75% horizontal offsets,
    /// `corner` wide, (top_edge × 4/3) tall, anchored to the bottom edge.
    #[test]
    fn test_build_snap_zones_bottom_thirds() {
        let displays = vec![rect(0.0, 0.0, 1920.0, 1080.0)];
        let zones = build_snap_zones(&displays, 18.0, 18.0, 50.0);
        let expected_h = 18.0 * 4.0 / 3.0;
        // LeftThird at 25%
        let (r, layout, _) = &zones[7];
        assert_eq!(*layout, TilingLayout::LeftThird);
        assert!((r.x - (1920.0 * 0.25 - 25.0)).abs() < 0.01);
        assert!((r.y - (1080.0 - expected_h)).abs() < 0.01);
        assert!((r.width - 50.0).abs() < 0.01);
        assert!((r.height - expected_h).abs() < 0.01);
        // CenterThird at 50%
        let (r, layout, _) = &zones[8];
        assert_eq!(*layout, TilingLayout::CenterThird);
        assert!((r.x - (1920.0 * 0.50 - 25.0)).abs() < 0.01);
        // RightThird at 75%
        let (r, layout, _) = &zones[9];
        assert_eq!(*layout, TilingLayout::RightThird);
        assert!((r.x - (1920.0 * 0.75 - 25.0)).abs() < 0.01);
    }

    /// Cursor near 25% of bottom edge → LeftThird snap.
    #[test]
    fn test_snap_zone_bottom_left_third() {
        let displays = vec![rect(0.0, 0.0, 1920.0, 1080.0)];
        // 25% of 1920 = 480, bottom y = 1080 - 18 = 1062..1080
        let result = detect_snap_zone_macos(480.0, 1070.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::LeftThird, 0)));
    }

    /// Cursor near 50% of bottom edge → CenterThird snap.
    #[test]
    fn test_snap_zone_bottom_center_third() {
        let displays = vec![rect(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone_macos(960.0, 1070.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::CenterThird, 0)));
    }

    /// Cursor near 75% of bottom edge → RightThird snap.
    #[test]
    fn test_snap_zone_bottom_right_third() {
        let displays = vec![rect(0.0, 0.0, 1920.0, 1080.0)];
        let result = detect_snap_zone_macos(1440.0, 1070.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::RightThird, 0)));
    }

    /// Cursor at the bottom-left corner → BottomLeftQuarter, not LeftThird.
    /// Corners are listed first in `build_snap_zones` so they win priority
    /// even though the bottom-third zone shares the same bottom-edge band.
    #[test]
    fn test_snap_zone_corner_beats_bottom_third() {
        let displays = vec![rect(0.0, 0.0, 1920.0, 1080.0)];
        // Inside the bottom-left 50×50 corner (0..50, 1030..1080)
        let result = detect_snap_zone_macos(10.0, 1070.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::BottomLeftQuarter, 0)));
    }

    /// Cursor above the bottom-third strip (in the empty middle) → no zone.
    /// Verifies the bottom-third zones are anchored to the bottom edge and
    /// don't reach up into the display body.
    #[test]
    fn test_snap_zone_above_bottom_third_no_zone() {
        let displays = vec![rect(0.0, 0.0, 1920.0, 1080.0)];
        // 50% horizontally (would hit CenterThird) but well above the bottom strip
        let result = detect_snap_zone_macos(960.0, 800.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, None);
    }

    /// Top-right corner rect of D0 has correct position and size.
    #[test]
    fn test_build_snap_zones_corner_rect() {
        let displays = vec![rect(100.0, 200.0, 1920.0, 1080.0)];
        let zones = build_snap_zones(&displays, 18.0, 18.0, 50.0);
        // TopRightQuarter is zones[1]
        let (r, layout, idx) = &zones[1];
        assert_eq!(*layout, TilingLayout::TopRightQuarter);
        assert_eq!(*idx, 0);
        assert!((r.x - (100.0 + 1920.0 - 50.0)).abs() < 0.01);
        assert!((r.y - 200.0).abs() < 0.01);
        assert!((r.width - 50.0).abs() < 0.01);
        assert!((r.height - 50.0).abs() < 0.01);
    }

    /// Right edge rect spans full display height.
    #[test]
    fn test_build_snap_zones_right_edge_rect() {
        let displays = vec![rect(0.0, 0.0, 1920.0, 1080.0)];
        let zones = build_snap_zones(&displays, 18.0, 18.0, 50.0);
        // RightHalf is zones[6]
        let (r, layout, _) = &zones[6];
        assert_eq!(*layout, TilingLayout::RightHalf);
        assert!((r.x - (1920.0 - 18.0)).abs() < 0.01);
        assert!((r.y - 0.0).abs() < 0.01);
        assert!((r.width - 18.0).abs() < 0.01);
        assert!((r.height - 1080.0).abs() < 0.01);
    }

    /// Adjacent displays: cursor just inside D0's right edge → RightHalf D0.
    /// Cursor just inside D1's left edge → LeftHalf D1. No overlap confusion.
    #[test]
    fn test_snap_zone_adjacent_no_overlap() {
        let displays = vec![
            rect(0.0, 0.0, 1000.0, 800.0),
            rect(1000.0, 0.0, 1000.0, 800.0),
        ];
        // 1px inside D0's right edge zone (x=983, zone starts at 982)
        let r1 = detect_snap_zone_macos(983.0, 400.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(r1, Some((TilingLayout::RightHalf, 0)));
        // 1px inside D1's left edge zone (x=1001)
        let r2 = detect_snap_zone_macos(1001.0, 400.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(r2, Some((TilingLayout::LeftHalf, 1)));
    }

    /// Cursor outside all displays → None.
    #[test]
    fn test_snap_zone_outside_all_displays() {
        let displays = three_monitors();
        let result = detect_snap_zone_macos(5000.0, 5000.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, None);
    }

    /// Corner takes priority over edge when both overlap.
    #[test]
    fn test_snap_zone_corner_beats_edge() {
        let displays = vec![rect(0.0, 0.0, 1920.0, 1080.0)];
        // Point at (5, 5) — inside top-left corner (50x50) AND top edge AND left edge
        let result = detect_snap_zone_macos(5.0, 5.0, &displays, 18.0, 18.0, 50.0);
        assert_eq!(result, Some((TilingLayout::TopLeftQuarter, 0)));
    }
}
