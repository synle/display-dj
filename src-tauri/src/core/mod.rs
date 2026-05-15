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
