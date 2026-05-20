// Vendored from display-dj-cli: in-process port of the brightness/contrast/theme/
// volume/wallpaper engine that previously ran as an HTTP sidecar.
//
// Platform modules — only one is compiled per OS (like build-time if/else).
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "linux")]
pub mod linux;

// Windows-only helper for spawning powershell/reg without flashing a console
// window. Local to display-dj — not present in the display-dj-cli upstream.
#[cfg(target_os = "windows")]
pub mod win_cmd;

pub mod theme;
pub mod volume;
pub mod keyboard_backlight;
pub mod wallpaper;
pub mod display;

use serde::{Deserialize, Serialize};

// VCP (Virtual Control Panel) codes — standard DDC/CI register addresses.
// These are the same across all monitor brands.
pub const VCP_BRIGHTNESS: u8 = 0x10;
pub const VCP_CONTRAST: u8 = 0x12;
pub const BUILTIN_ID: &str = "builtin"; // &str = borrowed string literal baked into the binary

// =========================================================================
// Shared types — used by all platform modules
// =========================================================================

/// Per-display info exposed to UI code and the platform modules.
#[derive(Serialize, Deserialize, Clone)]
pub struct DisplayInfo {
    pub id: String,             // "builtin", "1", "2", ...
    pub name: String,           // human-readable name from the monitor's EDID
    pub display_type: String,   // "builtin" or "external"
    pub brightness: Option<u32>, // Option = nullable — Some(75) or None
    pub contrast: Option<u32>,
    pub ddc_supported: bool,
    /// Physical screen rect for this monitor as `(left, top, width, height)` in
    /// **global physical pixels** (the coordinate space Win32 / NSScreen use for
    /// multi-monitor layouts). Used by the soft-overlay brightness fallback to
    /// size and position a per-monitor click-through dimming window.
    ///
    /// Populated on Windows (from `MONITORINFOEXW.rcMonitor`). On macOS and
    /// Linux this is currently always `None` — see the TODOs in `core::macos`
    /// and `core::linux`. The overlay fallback works on Windows out of the
    /// box; cross-platform support requires filling this in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor_rect: Option<(i32, i32, i32, i32)>,
}

// Trait = interface. Each platform module implements this for its display types.
// &mut self = mutable reference to the object (like `this` but must opt in to mutation).
// &self = read-only reference.
pub trait DisplayControl {
    fn get_brightness(&mut self) -> Option<u32>;
    fn get_contrast(&mut self) -> Option<u32>;
    fn set_brightness(&mut self, value: u16, mode: &str) -> bool;
    fn set_contrast(&mut self, value: u16) -> bool;
    fn reset_gamma(&self);
}

// No &self — these are static methods called on the type itself (Platform::enumerate()).
// Box<dyn DisplayControl> = heap-allocated trait object — lets us store different
// concrete types (BuiltinControl, ExternalControl) in the same Vec.
pub trait Platform {
    fn enumerate() -> Vec<(DisplayInfo, Box<dyn DisplayControl>)>;
    fn reset_all_gamma();
    fn debug_info() -> serde_json::Value;
}

/// Match a display by ID, name (case-insensitive), or "0" as alias for builtin.
pub fn matches_display(info: &DisplayInfo, query: &str) -> bool {
    if query == "0" {
        return info.id == BUILTIN_ID;
    }
    info.id == query || info.name.to_lowercase() == query.to_lowercase()
}

// Platform alias — picks the right implementation at compile time so the rest of
// the crate can write `<PlatformImpl as Platform>::enumerate()` without cfg juggling.
#[cfg(target_os = "macos")]
pub type PlatformImpl = macos::MacPlatform;
#[cfg(target_os = "windows")]
pub type PlatformImpl = windows::WinPlatform;
#[cfg(target_os = "linux")]
pub type PlatformImpl = linux::LinuxPlatform;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_info(id: &str, name: &str) -> DisplayInfo {
        DisplayInfo {
            id: id.to_string(),
            name: name.to_string(),
            display_type: "external".to_string(),
            brightness: Some(50),
            contrast: Some(50),
            ddc_supported: true,
            monitor_rect: None,
        }
    }

    /// VCP register addresses are fixed by the DDC/CI spec.
    #[test]
    fn test_vcp_codes() {
        assert_eq!(VCP_BRIGHTNESS, 0x10);
        assert_eq!(VCP_CONTRAST, 0x12);
        assert_eq!(BUILTIN_ID, "builtin");
    }

    /// matches_display matches by exact id.
    #[test]
    fn test_matches_display_by_id() {
        let info = make_info("1", "Dell U2720Q");
        assert!(matches_display(&info, "1"));
        assert!(!matches_display(&info, "2"));
    }

    /// matches_display matches by case-insensitive name.
    #[test]
    fn test_matches_display_by_name_case_insensitive() {
        let info = make_info("1", "Dell U2720Q");
        assert!(matches_display(&info, "dell u2720q"));
        assert!(matches_display(&info, "DELL U2720Q"));
        assert!(matches_display(&info, "Dell U2720Q"));
        assert!(!matches_display(&info, "samsung"));
    }

    /// matches_display: "0" is an alias for the builtin display only.
    #[test]
    fn test_matches_display_zero_is_builtin_alias() {
        let builtin = make_info(BUILTIN_ID, "Built-in Display");
        let external = make_info("1", "Dell");
        assert!(matches_display(&builtin, "0"));
        assert!(!matches_display(&external, "0"));
    }

    /// DisplayInfo serializes to JSON cleanly with optional fields preserved.
    #[test]
    fn test_display_info_serde_roundtrip() {
        let info = make_info("1", "Dell");
        let json = serde_json::to_string(&info).unwrap();
        let parsed: DisplayInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "1");
        assert_eq!(parsed.name, "Dell");
        assert_eq!(parsed.brightness, Some(50));
        assert_eq!(parsed.contrast, Some(50));
        assert!(parsed.ddc_supported);
    }

    /// DisplayInfo with monitor_rect serializes/deserializes correctly.
    #[test]
    fn test_display_info_monitor_rect_serde() {
        let mut info = make_info("1", "Dell");
        info.monitor_rect = Some((0, 0, 1920, 1080));
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"monitor_rect\":[0,0,1920,1080]"));
        let parsed: DisplayInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.monitor_rect, Some((0, 0, 1920, 1080)));
    }

    /// DisplayInfo with None monitor_rect omits the field (skip_serializing_if).
    #[test]
    fn test_display_info_monitor_rect_none_omitted() {
        let info = make_info("1", "Dell");
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("monitor_rect"));
    }

    /// DisplayInfo brightness/contrast can be None and parsed back as None.
    #[test]
    fn test_display_info_none_brightness_contrast() {
        let mut info = make_info("1", "Dell");
        info.brightness = None;
        info.contrast = None;
        let json = serde_json::to_string(&info).unwrap();
        let parsed: DisplayInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.brightness, None);
        assert_eq!(parsed.contrast, None);
    }

    /// Backward-compatible deserialization: old JSON without monitor_rect parses.
    #[test]
    fn test_display_info_back_compat_no_monitor_rect() {
        let json = r#"{
            "id": "1",
            "name": "Dell",
            "display_type": "external",
            "brightness": 50,
            "contrast": 50,
            "ddc_supported": true
        }"#;
        let parsed: DisplayInfo = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.id, "1");
        assert_eq!(parsed.monitor_rect, None);
    }
}
