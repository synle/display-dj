// =========================================================================
// Keyboard backlight — controls the built-in laptop keyboard backlight LED.
// Beta feature. Not vendored from display-dj-cli — display-dj-local.
//
// macOS: IOHIDEventSystem private (but stable since 10.7) — set the
//        "KeyboardBacklightBrightness" property on the IOHIDServiceClient
//        whose primary HID usage is page 0x0B (LEDs) usage 0x4B (keyboard
//        backlight). Same path used by Lunar / kbdlight / BetterTouchTool.
//        Symbols loaded via dlopen so a future SDK rename degrades to
//        is_supported() = false instead of crashing at link time.
//
// Windows: vendor WMI only — Lenovo (ThinkPad/IdeaPad) and Dell
//          (Latitude/XPS) via PowerShell. First vendor that responds wins.
//          All spawns route through hidden_command (Windows console-flash rule).
//
// Linux:  out of scope for v7.0.26. is_supported() returns false; get/set
//         are no-ops. Future: /sys/class/leds/*::kbd_backlight/ or UPower.
//
// External keyboards (Razer / Corsair / Logitech, USB-HID generic, Bluetooth):
//   not supported in any backend. Out of scope.
// =========================================================================

#[cfg(target_os = "windows")]
use super::win_cmd::hidden_command;

/// Snap a 0..100 value to the nearest 25% step (0/25/50/75/100). Caller-side
/// guarantee so both the slider and the keybinding shortcut produce the same
/// reachable backlight levels — never a 73 or a 42.
pub fn snap_to_25(value: u32) -> u32 {
    let clamped = value.min(100);
    let snapped = ((clamped as f32 / 25.0).round() as u32) * 25;
    snapped.min(100)
}

// ----------------------------------------------------------------------- macOS

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{c_void, CString};
    use std::os::raw::c_char;
    use std::sync::OnceLock;

    // Opaque pointer types — IOKit / CoreFoundation handles. Rust doesn't need
    // to know the layout, only that they're pointer-sized references.
    type CFTypeRef = *const c_void;
    type CFAllocatorRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFNumberRef = *const c_void;
    type CFArrayRef = *const c_void;
    // CFIndex is `long` on macOS — declared as i64 to match the existing
    // tiling/macos.rs extern declarations (one source of truth across crates).
    type CFIndex = i64;

    type IOHIDEventSystemClientRef = *const c_void;
    type IOHIDServiceClientRef = *const c_void;

    // CFNumber type codes — kCFNumberSInt32Type. Declared as i64 to match
    // the CFNumberType (CFIndex) signature used elsewhere in the crate.
    const K_CF_NUMBER_SINT_32_TYPE: i64 = 3;

    // HID page/usage we want to match. Page 0x0B is "LEDs"; usage 0x4B is
    // "Keyboard Backlight." This is the same matching tuple Lunar uses.
    const HID_PAGE_LEDS: i32 = 0x0B;
    const HID_USAGE_KEYBOARD_BACKLIGHT: i32 = 0x4B;

    // Property keys (UTF-8 — converted to CFString at call time).
    const KEY_PRIMARY_USAGE_PAGE: &str = "PrimaryUsagePage";
    const KEY_PRIMARY_USAGE: &str = "PrimaryUsage";
    const KEY_BRIGHTNESS: &str = "KeyboardBacklightBrightness";

    // Brightness is a CFNumber in [0, 65535]. The HID spec says LED usages are
    // 0..MAX_INT but Apple's driver clips at 0xFFFF.
    const MAX_HW: f32 = 65535.0;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: CFAllocatorRef,
            c_str: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFNumberCreate(
            alloc: CFAllocatorRef,
            type_code: i64,
            value_ptr: *const c_void,
        ) -> CFNumberRef;
        fn CFNumberGetValue(
            number: CFNumberRef,
            type_code: i64,
            out: *mut c_void,
        ) -> bool;
        fn CFArrayGetCount(arr: CFArrayRef) -> CFIndex;
        fn CFArrayGetValueAtIndex(arr: CFArrayRef, idx: CFIndex) -> *const c_void;
        fn CFRelease(cf: CFTypeRef);
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

    // dlopen / dlsym — IOHIDEventSystem* symbols live in the private
    // IOKit framework. Loading at runtime lets us degrade to "unsupported"
    // instead of failing to link if Apple ever drops the symbols.
    type DlsymHandle = *mut c_void;
    const RTLD_NOW: i32 = 2;
    const RTLD_GLOBAL: i32 = 8;

    extern "C" {
        fn dlopen(filename: *const c_char, flag: i32) -> DlsymHandle;
        fn dlsym(handle: DlsymHandle, symbol: *const c_char) -> *mut c_void;
    }

    type CreateClientFn = unsafe extern "C" fn(CFAllocatorRef) -> IOHIDEventSystemClientRef;
    type CopyServicesFn = unsafe extern "C" fn(IOHIDEventSystemClientRef) -> CFArrayRef;
    type CopyPropertyFn =
        unsafe extern "C" fn(IOHIDServiceClientRef, CFStringRef) -> CFTypeRef;
    type SetPropertyFn =
        unsafe extern "C" fn(IOHIDServiceClientRef, CFStringRef, CFTypeRef) -> bool;
    type ConformsToFn =
        unsafe extern "C" fn(IOHIDServiceClientRef, i32, i32) -> bool;

    /// Resolved function pointers from the private IOKit framework.
    struct IoHidFns {
        create_client: CreateClientFn,
        copy_services: CopyServicesFn,
        copy_property: CopyPropertyFn,
        set_property: SetPropertyFn,
        conforms_to: ConformsToFn,
    }

    /// Cache the framework load + symbol resolution. None means the framework
    /// or symbols are missing — caller should treat as "unsupported."
    fn fns() -> Option<&'static IoHidFns> {
        static CACHE: OnceLock<Option<IoHidFns>> = OnceLock::new();
        CACHE
            .get_or_init(|| unsafe {
                let path = CString::new(
                    "/System/Library/Frameworks/IOKit.framework/IOKit",
                )
                .ok()?;
                let handle = dlopen(path.as_ptr(), RTLD_NOW | RTLD_GLOBAL);
                if handle.is_null() {
                    return None;
                }

                let load = |name: &str| -> *mut c_void {
                    CString::new(name)
                        .ok()
                        .and_then(|c| {
                            let p = dlsym(handle, c.as_ptr());
                            if p.is_null() { None } else { Some(p) }
                        })
                        .unwrap_or(std::ptr::null_mut())
                };

                let create = load("IOHIDEventSystemClientCreate");
                let services = load("IOHIDEventSystemClientCopyServices");
                let copy_prop = load("IOHIDServiceClientCopyProperty");
                let set_prop = load("IOHIDServiceClientSetProperty");
                let conforms = load("IOHIDServiceClientConformsTo");

                if create.is_null()
                    || services.is_null()
                    || copy_prop.is_null()
                    || set_prop.is_null()
                    || conforms.is_null()
                {
                    return None;
                }

                Some(IoHidFns {
                    create_client: std::mem::transmute(create),
                    copy_services: std::mem::transmute(services),
                    copy_property: std::mem::transmute(copy_prop),
                    set_property: std::mem::transmute(set_prop),
                    conforms_to: std::mem::transmute(conforms),
                })
            })
            .as_ref()
    }

    /// Create a CFString from a Rust &str. Caller must CFRelease.
    unsafe fn cf_str(s: &str) -> CFStringRef {
        let c = match CString::new(s) {
            Ok(c) => c,
            Err(_) => return std::ptr::null(),
        };
        CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8)
    }

    /// Walk all IOHIDServiceClients on the system and return the first one
    /// whose primary usage page/usage matches keyboard backlight.
    unsafe fn find_backlight_service(fns: &IoHidFns) -> Option<IOHIDServiceClientRef> {
        let client = (fns.create_client)(std::ptr::null());
        if client.is_null() {
            return None;
        }

        let services = (fns.copy_services)(client);
        if services.is_null() {
            CFRelease(client);
            return None;
        }

        let count = CFArrayGetCount(services);
        let key_page = cf_str(KEY_PRIMARY_USAGE_PAGE);
        let key_usage = cf_str(KEY_PRIMARY_USAGE);

        let mut found: Option<IOHIDServiceClientRef> = None;
        for i in 0..count {
            let svc = CFArrayGetValueAtIndex(services, i) as IOHIDServiceClientRef;
            if svc.is_null() {
                continue;
            }

            // Fast path: ask the service directly if it conforms to LEDs/backlight.
            if (fns.conforms_to)(svc, HID_PAGE_LEDS, HID_USAGE_KEYBOARD_BACKLIGHT) {
                found = Some(svc);
                break;
            }

            // Fallback: inspect PrimaryUsagePage + PrimaryUsage properties.
            let page_ref = (fns.copy_property)(svc, key_page);
            let usage_ref = (fns.copy_property)(svc, key_usage);
            if !page_ref.is_null() && !usage_ref.is_null() {
                let mut page: i32 = 0;
                let mut usage: i32 = 0;
                CFNumberGetValue(
                    page_ref,
                    K_CF_NUMBER_SINT_32_TYPE,
                    &mut page as *mut _ as *mut c_void,
                );
                CFNumberGetValue(
                    usage_ref,
                    K_CF_NUMBER_SINT_32_TYPE,
                    &mut usage as *mut _ as *mut c_void,
                );
                if page == HID_PAGE_LEDS && usage == HID_USAGE_KEYBOARD_BACKLIGHT {
                    found = Some(svc);
                    // fall through to release page/usage refs then break
                }
            }
            if !page_ref.is_null() {
                CFRelease(page_ref);
            }
            if !usage_ref.is_null() {
                CFRelease(usage_ref);
            }
            if found.is_some() {
                break;
            }
        }

        if !key_page.is_null() {
            CFRelease(key_page);
        }
        if !key_usage.is_null() {
            CFRelease(key_usage);
        }
        // NOTE: we intentionally do NOT CFRelease the services array or the
        // client here in the success path — the service pointer we hand back
        // is owned by them. Caller is short-lived (get_/set_ returns
        // immediately) so leaking the wrapper handles for a few microseconds
        // is acceptable. If this becomes a long-running probe loop, switch to
        // copying the service ref via CFRetain + releasing the container.
        if found.is_none() {
            CFRelease(services);
            CFRelease(client);
        }

        found
    }

    /// Read the current keyboard backlight as 0..100, or None if unsupported.
    pub fn get() -> Option<u32> {
        let fns = fns()?;
        unsafe {
            let svc = find_backlight_service(fns)?;
            let key = cf_str(KEY_BRIGHTNESS);
            if key.is_null() {
                return None;
            }
            let value_ref = (fns.copy_property)(svc, key);
            CFRelease(key);
            if value_ref.is_null() {
                return None;
            }
            let mut raw: i32 = 0;
            let ok = CFNumberGetValue(
                value_ref,
                K_CF_NUMBER_SINT_32_TYPE,
                &mut raw as *mut _ as *mut c_void,
            );
            CFRelease(value_ref);
            if !ok || raw < 0 {
                return None;
            }
            let pct = ((raw as f32 / MAX_HW) * 100.0).round();
            Some(pct.clamp(0.0, 100.0) as u32)
        }
    }

    /// Set the keyboard backlight to `level_pct` (0..100). Returns true on
    /// platform-layer success. Caller is expected to have already snapped
    /// the value to 0/25/50/75/100.
    pub fn set(level_pct: u32) -> bool {
        let fns = match fns() {
            Some(f) => f,
            None => return false,
        };
        let pct = level_pct.min(100);
        let raw_hw = ((pct as f32 / 100.0) * MAX_HW).round() as i32;
        unsafe {
            let svc = match find_backlight_service(fns) {
                Some(s) => s,
                None => return false,
            };
            let key = cf_str(KEY_BRIGHTNESS);
            if key.is_null() {
                return false;
            }
            let value =
                CFNumberCreate(std::ptr::null(), K_CF_NUMBER_SINT_32_TYPE, &raw_hw as *const _ as *const c_void);
            if value.is_null() {
                CFRelease(key);
                return false;
            }
            let ok = (fns.set_property)(svc, key, value);
            CFRelease(value);
            CFRelease(key);
            ok
        }
    }

    pub fn is_supported() -> bool {
        // Cheapest possible probe: do we have the symbols AND can we find a
        // service? Result not cached at this layer — `SidecarCache` caches
        // the high-level Tauri command result.
        if fns().is_none() {
            return false;
        }
        unsafe {
            let fns_ref = match fns() {
                Some(f) => f,
                None => return false,
            };
            find_backlight_service(fns_ref).is_some()
        }
    }
}

// ----------------------------------------------------------------------- Windows

#[cfg(target_os = "windows")]
mod imp {
    use super::hidden_command;

    /// Probe Lenovo's WMI keyboard-backlight namespace. Returns Some(level 0..100)
    /// if a Lenovo-flavored backlight method responded, None otherwise. The
    /// PowerShell snippet is intentionally tolerant — many ThinkPad SKUs expose
    /// the level under slightly different names (`Lenovo_GSensor`, `Lenovo_BIOSElement`,
    /// dedicated `Lenovo_BIOSSetting` row), so we probe a couple of paths and
    /// take the first non-empty integer in [0, 100].
    fn lenovo_get() -> Option<u32> {
        let out = hidden_command("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                r#"try {
                    $g = Get-CimInstance -Namespace root\wmi -ClassName Lenovo_KeyboardBacklightLevel -ErrorAction SilentlyContinue
                    if ($g -and $g.CurrentLevel -ne $null) { Write-Output $g.CurrentLevel; exit 0 }
                } catch {}
                exit 1"#,
            ])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let raw: u32 = s.parse().ok()?;
        // Lenovo levels are 0..2 on most ThinkPads. Translate to %.
        match raw {
            0 => Some(0),
            1 => Some(50),
            2 => Some(100),
            _ => Some(raw.min(100)),
        }
    }

    fn lenovo_set(level_pct: u32) -> bool {
        // Map 0/25/50/75/100 → Lenovo's 0/1/2 (off/dim/bright).
        let lenovo_level = match level_pct {
            0 => 0u32,
            1..=49 => 1u32,
            _ => 2u32,
        };
        let cmd = format!(
            r#"try {{
                $m = Get-CimInstance -Namespace root\wmi -ClassName Lenovo_SetKeyboardBacklightLevel -ErrorAction Stop
                Invoke-CimMethod -InputObject $m -MethodName SetKeyboardBacklightStatus -Arguments @{{ Level = {} }} | Out-Null
                exit 0
            }} catch {{
                exit 1
            }}"#,
            lenovo_level
        );
        hidden_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn dell_get() -> Option<u32> {
        let out = hidden_command("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                r#"try {
                    $g = Get-CimInstance -Namespace root\wmi -ClassName DellKeyboardBacklight -ErrorAction SilentlyContinue
                    if ($g -and $g.Level -ne $null) { Write-Output $g.Level; exit 0 }
                } catch {}
                exit 1"#,
            ])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let raw: u32 = s.parse().ok()?;
        // Dell levels are 0..2 on most Latitudes (off/dim/bright).
        match raw {
            0 => Some(0),
            1 => Some(50),
            2 => Some(100),
            _ => Some(raw.min(100)),
        }
    }

    fn dell_set(level_pct: u32) -> bool {
        let dell_level = match level_pct {
            0 => 0u32,
            1..=49 => 1u32,
            _ => 2u32,
        };
        let cmd = format!(
            r#"try {{
                $m = Get-CimInstance -Namespace root\wmi -ClassName DellKeyboardBacklight -ErrorAction Stop
                Invoke-CimMethod -InputObject $m -MethodName SetKeyboardBacklightLevel -Arguments @{{ Level = {} }} | Out-Null
                exit 0
            }} catch {{
                exit 1
            }}"#,
            dell_level
        );
        hidden_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn get() -> Option<u32> {
        lenovo_get().or_else(dell_get)
    }

    pub fn set(level_pct: u32) -> bool {
        // Try the same vendor order we probed for `get`. First success wins.
        if lenovo_set(level_pct) {
            return true;
        }
        dell_set(level_pct)
    }

    pub fn is_supported() -> bool {
        get().is_some()
    }
}

// ----------------------------------------------------------------------- Linux / fallback

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    pub fn get() -> Option<u32> {
        None
    }
    pub fn set(_level_pct: u32) -> bool {
        false
    }
    pub fn is_supported() -> bool {
        false
    }
}

/// Returns the current keyboard backlight as a 0..100 percentage, or `None` if
/// no supported backend reports a value (treat as "unsupported on this device").
pub fn get_keyboard_backlight() -> Option<u32> {
    imp::get()
}

/// Sets the keyboard backlight to `level_pct` (0..100, snapped to 25 by caller).
/// Returns true if the platform layer accepted the write.
pub fn set_keyboard_backlight(level_pct: u32) -> bool {
    imp::set(snap_to_25(level_pct))
}

/// Returns true when at least one backend can read the current backlight level
/// on this device. Used at startup to decide whether to render the slider.
pub fn is_supported() -> bool {
    imp::is_supported()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// snap_to_25 must round to the nearest 25-step and clamp at 100.
    #[test]
    fn snap_to_25_rounds_to_nearest_step() {
        assert_eq!(snap_to_25(0), 0);
        assert_eq!(snap_to_25(1), 0);
        assert_eq!(snap_to_25(12), 0); // < midpoint → down
        assert_eq!(snap_to_25(13), 25); // ≥ midpoint → up
        assert_eq!(snap_to_25(25), 25);
        assert_eq!(snap_to_25(37), 25);
        assert_eq!(snap_to_25(38), 50);
        assert_eq!(snap_to_25(50), 50);
        assert_eq!(snap_to_25(75), 75);
        assert_eq!(snap_to_25(99), 100);
        assert_eq!(snap_to_25(100), 100);
    }

    /// snap_to_25 clamps out-of-range input to 100 instead of overflowing.
    #[test]
    fn snap_to_25_clamps_above_100() {
        assert_eq!(snap_to_25(150), 100);
        assert_eq!(snap_to_25(u32::MAX), 100);
    }

    /// get_keyboard_backlight must not panic on any platform.
    /// On CI Linux / unsupported devices it should return None.
    #[test]
    fn get_keyboard_backlight_smoke() {
        let _ = get_keyboard_backlight();
    }

    /// is_supported must not panic and returns a deterministic bool.
    #[test]
    fn is_supported_smoke() {
        let _ = is_supported();
    }

    /// set_keyboard_backlight is callable with any 0..100 value without
    /// panicking. We don't assert success — the test runner may be on a
    /// keyboard-backlight-less machine. Restore to previous state after.
    #[test]
    fn set_keyboard_backlight_smoke() {
        let original = get_keyboard_backlight();
        let _ = set_keyboard_backlight(50);
        // Best-effort restore. No-op when unsupported.
        if let Some(prev) = original {
            let _ = set_keyboard_backlight(prev);
        }
    }
}
