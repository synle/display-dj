use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Absolute floor for brightness — never allow less than this regardless of user config.
pub const ABSOLUTE_MIN_BRIGHTNESS: u32 = 5;

/// Tiling window manager preferences.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct TilingPreferences {
    /// Master toggle to enable/disable all tiling features.
    pub enabled: bool,
    /// Percentage for half splits (affects halves and quarter corners).
    pub half_ratio: u32,
    /// Percentage for third splits. Center/middle = 100 - 2*third.
    pub third_ratio: u32,
    /// Gap in points between tiled windows and screen edges.
    pub gap: u32,
    /// Toggle to enable/disable Tile Snap (mouse edge snapping). Only effective
    /// when `enabled` is also true. Defaults to true.
    pub tile_snap_enabled: bool,
    /// Tile Snap: pixel width of the side edge hot zone (left/right/bottom).
    pub side_edge_trigger: u32,
    /// Tile Snap: pixel height of the top edge hot zone (maximize trigger).
    pub top_edge_trigger: u32,
    /// Tile Snap: pixel size of the corner hot zone (quarter tile trigger).
    pub corner_trigger: u32,
    /// Master toggle to enable/disable Exposé features.
    pub expose_enabled: bool,
    /// Exposé: number of columns in the grid.
    pub expose_columns: u32,
    /// Exposé: number of rows in the grid.
    pub expose_rows: u32,
    /// Exposé layout strategy: "spread" distributes windows evenly across all
    /// displays; "fill" packs each display to capacity before using the next.
    pub expose_layout_strategy: String,
    /// Exposé: minimum grid cell width in logical pixels. Cells smaller than
    /// this cause windows to overflow to less-crowded displays. Scaled by
    /// display DPI on Windows (macOS uses points natively).
    pub expose_min_width: u32,
    /// Exposé: minimum grid cell height in logical pixels. Same scaling as width.
    pub expose_min_height: u32,
    /// Legacy field for backward-compatible deserialization of old configs.
    /// Not serialized; on load, if columns/rows are at defaults and this is
    /// non-zero, we migrate sqrt(value) into columns and rows.
    #[serde(default, skip_serializing)]
    pub expose_max_windows: u32,
}

impl Default for TilingPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            half_ratio: 50,
            third_ratio: 33,
            gap: 0,
            tile_snap_enabled: true,
            side_edge_trigger: 18,
            top_edge_trigger: 18,
            corner_trigger: 30,
            expose_enabled: true,
            expose_columns: 3,
            expose_rows: 3,
            expose_layout_strategy: "fill".into(),
            expose_min_width: 400,
            expose_min_height: 300,
            expose_max_windows: 0,
        }
    }
}

/// Returns the default `brightness_mode` value for `MonitorMetadata`.
///
/// Used by `#[serde(default = "...")]` so old `preferences.json` files (written
/// before the `brightnessMode` field existed) deserialize without error and end
/// up with the auto-discovery path enabled.
pub fn default_brightness_mode() -> String {
    "auto".into()
}

/// Per-monitor metadata stored in preferences. Acts as a persistent registry —
/// entries are added when a monitor is first detected and never removed on unplug,
/// so labels and sort order survive across plug/unplug cycles.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonitorMetadata {
    /// Composite unique key: "{api_id}::{api_name}" (e.g. "1::Dell U2723QE")
    pub uid: String,
    /// Raw ID from the display-dj sidecar API (e.g. "1", "builtin")
    pub api_id: String,
    /// Model name from the display-dj sidecar API (e.g. "Dell U2723QE")
    pub api_name: String,
    /// User-set friendly label. Empty string means use api_name.
    pub label: String,
    /// Sort order for UI display. Lower values come first.
    pub sort_order: i32,
    /// Whether the monitor is hidden from the default UI view.
    #[serde(default)]
    pub hidden: bool,
    /// Brightness control strategy for this monitor. One of:
    /// - `"auto"` (default) — try DDC, then gamma, then fall back to the
    ///   soft-overlay window when both hardware paths fail.
    /// - `"ddc"` — DDC/CI only, no overlay (use for panels that work fine over
    ///   I2C; useful to disable the overlay fallback if you have a flicker).
    /// - `"gamma"` — GDI `SetDeviceGammaRamp` only, no overlay.
    /// - `"overlay"` — skip the hardware paths entirely and dim with the
    ///   transparent overlay window. The only mode that works on USB-C
    ///   Samsung Smart Monitors on Intel Iris Xe (no DDC response, gamma
    ///   silently rejected by the driver).
    ///
    /// Defaults to `"auto"` so existing configs and new monitors keep working
    /// without user intervention.
    #[serde(default = "default_brightness_mode")]
    pub brightness_mode: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct NightModeSchedule {
    pub enabled: bool,
    /// "HH:MM" 24-hour format
    pub night_start: String,
    pub night_brightness: u32,
    /// "HH:MM" 24-hour format
    pub day_start: String,
    pub day_brightness: u32,
    /// Optional commands to execute when night mode activates.
    /// When non-empty, these replace the default brightness + dark mode behavior.
    /// Each string is a command (e.g. "command/changeBrightness/20", "command/changeDarkMode/dark").
    pub night_commands: Vec<String>,
    /// Optional commands to execute when day mode activates.
    /// When non-empty, these replace the default brightness + light mode behavior.
    pub day_commands: Vec<String>,
}

impl Default for NightModeSchedule {
    fn default() -> Self {
        Self {
            enabled: false,
            night_start: "21:00".into(),
            night_brightness: 20,
            day_start: "07:00".into(),
            day_brightness: 100,
            night_commands: Vec::new(),
            day_commands: Vec::new(),
        }
    }
}

/// A single rule within a layout preset: match windows by app name and apply a tiling layout.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutRule {
    /// Substring to match against the window's owner/app name (case-insensitive).
    pub app_match: String,
    /// TilingLayout name as a camelCase string (e.g. "leftHalf", "maximize").
    pub layout: String,
    /// Optional 0-based display index. If None, tiles on the window's current display.
    pub display_index: Option<usize>,
}

impl Default for LayoutRule {
    fn default() -> Self {
        Self {
            app_match: String::new(),
            layout: String::new(),
            display_index: None,
        }
    }
}

/// Tracks the wallpaper path set on a specific monitor.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MonitorWallpaper {
    /// UID of the monitor (e.g. "1::Dell U2723QE").
    pub monitor_uid: String,
    /// Path to the wallpaper file in the wallpapers directory.
    pub wallpaper_path: String,
}

/// Wallpaper preferences: fit mode, current wallpaper state, and slideshow config.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct WallpaperPreferences {
    /// How the wallpaper image fits the screen: fill, fit, stretch, center, tile.
    pub fit: String,
    /// Path to the currently active wallpaper in our wallpapers directory (all-monitors).
    pub current_wallpaper_path: Option<String>,
    /// Per-monitor wallpaper state.
    pub per_monitor_wallpapers: Vec<MonitorWallpaper>,
    /// Whether slideshow is enabled (persisted so it resumes on app restart).
    pub slideshow_enabled: bool,
    /// Folder path for slideshow images.
    pub slideshow_folder: Option<String>,
    /// Slideshow interval in minutes (minimum 5).
    pub slideshow_interval_minutes: u32,
    /// Slideshow cycling order: "forward", "backward", "random".
    pub slideshow_order: String,
}

impl Default for WallpaperPreferences {
    fn default() -> Self {
        Self {
            fit: "fill".into(),
            current_wallpaper_path: None,
            per_monitor_wallpapers: Vec::new(),
            slideshow_enabled: false,
            slideshow_folder: None,
            slideshow_interval_minutes: 30,
            slideshow_order: "forward".into(),
        }
    }
}

/// Keyboard backlight (beta) preferences. Controls the slider and command
/// behavior for the built-in laptop keyboard backlight. When `enabled` is
/// false, the UI hides the slider AND the `command/changeKeyboardBacklight/*`
/// command becomes a no-op (a master kill-switch). The slider also auto-hides
/// when the platform layer reports the device as unsupported (e.g. desktop
/// Macs, non-Lenovo/Dell Windows PCs).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct KeyboardBacklightPreferences {
    /// Master toggle. When false the slider is hidden and shortcut commands
    /// are ignored. Default: true (beta — opt-out, not opt-in).
    pub enabled: bool,
}

impl Default for KeyboardBacklightPreferences {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// A named window layout preset containing one or more layout rules.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutPreset {
    /// User-visible name for this preset.
    pub name: String,
    /// Ordered list of rules. Each matched window is tiled once (first matching rule wins).
    pub rules: Vec<LayoutRule>,
}

impl Default for LayoutPreset {
    fn default() -> Self {
        Self {
            name: String::new(),
            rules: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub show_individual_displays: bool,
    pub min_brightness: u32,
    pub key_bindings: Vec<KeyBinding>,
    pub profiles: Vec<Profile>,
    pub night_mode_schedule: NightModeSchedule,
    pub show_contrast: bool,
    pub debug_logging: bool,
    pub launch_at_login: bool,
    pub monitor_configs: Vec<MonitorMetadata>,
    pub tiling: TilingPreferences,
    /// Named window layout presets. Each preset contains rules that match windows
    /// by app name and apply tiling layouts. Triggered via `command/layout/{name_or_index}`.
    pub layout_presets: Vec<LayoutPreset>,
    /// Wallpaper preferences: fit mode and current wallpaper path.
    pub wallpaper: WallpaperPreferences,
    /// Keyboard backlight (beta) preferences. See `KeyboardBacklightPreferences`.
    pub keyboard_backlight: KeyboardBacklightPreferences,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KeyBinding {
    pub key: String,
    pub command: CommandValue,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    pub name: String,
    pub command: CommandValue,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: CommandValue::Single(String::new()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum CommandValue {
    Single(String),
    Multiple(Vec<String>),
}

impl Preferences {
    /// Returns the effective minimum brightness, enforcing the absolute floor.
    pub fn effective_min_brightness(&self) -> u32 {
        self.min_brightness.max(ABSOLUTE_MIN_BRIGHTNESS)
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            show_individual_displays: false,
            min_brightness: 10,
            key_bindings: vec![
                KeyBinding {
                    key: "Shift+Escape".into(),
                    command: CommandValue::Single("command/changeDarkMode/toggle".into()),
                },
                KeyBinding {
                    key: "Shift+F1".into(),
                    command: CommandValue::Single("command/changeProfile/1".into()),
                },
                // Shift+F2: combo binding. Activates profile 2 (Focus) AND
                // dims the keyboard backlight to 0. Mirrors the "dim everything"
                // intent — same shortcut, two coordinated dims.
                KeyBinding {
                    key: "Shift+F2".into(),
                    command: CommandValue::Multiple(vec![
                        "command/changeProfile/2".into(),
                        "command/changeKeyboardBacklight/0".into(),
                    ]),
                },
                KeyBinding {
                    key: "Shift+F3".into(),
                    command: CommandValue::Single("command/changeBrightness/0".into()),
                },
                KeyBinding {
                    key: "Shift+F4".into(),
                    command: CommandValue::Single("command/changeBrightness/50".into()),
                },
                KeyBinding {
                    key: "Shift+F5".into(),
                    command: CommandValue::Single("command/changeBrightness/100".into()),
                },
                KeyBinding {
                    key: "Shift+F10".into(),
                    command: CommandValue::Single("command/changeVolume/0".into()),
                },
                KeyBinding {
                    key: "Shift+F11".into(),
                    command: CommandValue::Single("command/changeVolume/10".into()),
                },
                KeyBinding {
                    key: "Shift+F12".into(),
                    command: CommandValue::Single("command/changeVolume/100".into()),
                },
                // Tiling: thirds via arrow keys
                KeyBinding {
                    key: "Shift+Ctrl+Left".into(),
                    command: CommandValue::Single("command/tile/leftThird".into()),
                },
                KeyBinding {
                    key: "Shift+Ctrl+Right".into(),
                    command: CommandValue::Single("command/tile/rightThird".into()),
                },
                // Tiling: two-thirds via up/down
                KeyBinding {
                    key: "Shift+Ctrl+Up".into(),
                    command: CommandValue::Single("command/tile/leftTwoThirds".into()),
                },
                KeyBinding {
                    key: "Shift+Ctrl+Down".into(),
                    command: CommandValue::Single("command/tile/rightTwoThirds".into()),
                },
                // Tiling: thirds via D/C/G
                KeyBinding {
                    key: "Shift+Ctrl+D".into(),
                    command: CommandValue::Single("command/tile/leftThird".into()),
                },
                KeyBinding {
                    key: "Shift+Ctrl+C".into(),
                    command: CommandValue::Single("command/tile/centerThird".into()),
                },
                KeyBinding {
                    key: "Shift+Ctrl+G".into(),
                    command: CommandValue::Single("command/tile/rightThird".into()),
                },
                // Tiling: quarters via I/O/K/L (keyboard layout)
                KeyBinding {
                    key: "Shift+Ctrl+I".into(),
                    command: CommandValue::Single("command/tile/topLeftQuarter".into()),
                },
                KeyBinding {
                    key: "Shift+Ctrl+O".into(),
                    command: CommandValue::Single("command/tile/topRightQuarter".into()),
                },
                KeyBinding {
                    key: "Shift+Ctrl+K".into(),
                    command: CommandValue::Single("command/tile/bottomLeftQuarter".into()),
                },
                KeyBinding {
                    key: "Shift+Ctrl+L".into(),
                    command: CommandValue::Single("command/tile/bottomRightQuarter".into()),
                },
                // Tiling: maximize
                KeyBinding {
                    key: "Shift+Ctrl+M".into(),
                    command: CommandValue::Single("command/tile/maximize".into()),
                },
                KeyBinding {
                    key: "Shift+Ctrl+/".into(),
                    command: CommandValue::Single("command/tile/maximize".into()),
                },
                // Tiling: exposé
                KeyBinding {
                    key: "Shift+Ctrl+E".into(),
                    command: CommandValue::Single("command/tile/expose".into()),
                },
                KeyBinding {
                    key: "Ctrl+Up".into(),
                    command: CommandValue::Single("command/tile/expose".into()),
                },
                // Tiling: app exposé (current app only)
                KeyBinding {
                    key: "Shift+Ctrl+A".into(),
                    command: CommandValue::Single("command/tile/exposeApp".into()),
                },
                KeyBinding {
                    key: "Ctrl+Down".into(),
                    command: CommandValue::Single("command/tile/exposeApp".into()),
                },
                // Z-order: send all windows of focused app to back / bring
                // them all to front. Mnemonic: Left = back (away), Right =
                // front (toward you). Shift+Ctrl+Super (Cmd on macOS, Win
                // on Windows, Super on Linux) is unbound by default in all
                // three OSes.
                KeyBinding {
                    key: "Shift+Ctrl+Super+Left".into(),
                    command: CommandValue::Single("command/app/moveToBack".into()),
                },
                KeyBinding {
                    key: "Shift+Ctrl+Super+Right".into(),
                    command: CommandValue::Single("command/app/moveToFront".into()),
                },
            ],
            profiles: vec![
                Profile {
                    name: "Presentation".into(),
                    command: CommandValue::Multiple(vec![
                        "command/changeBrightness/100".into(),
                        "command/changeDarkMode/light".into(),
                        "command/changeVolume/50".into(),
                    ]),
                },
                Profile {
                    name: "Focus".into(),
                    command: CommandValue::Multiple(vec![
                        "command/changeBrightness/75".into(),
                        "command/changeDarkMode/dark".into(),
                        "command/changeVolume/30".into(),
                    ]),
                },
                Profile {
                    name: "Daylight".into(),
                    command: CommandValue::Multiple(vec![
                        "command/changeBrightness/100".into(),
                        "command/changeDarkMode/light".into(),
                        "command/changeVolume/100".into(),
                    ]),
                },
            ],
            night_mode_schedule: NightModeSchedule::default(),
            show_contrast: false,
            debug_logging: false,
            launch_at_login: false,
            monitor_configs: Vec::new(),
            tiling: TilingPreferences::default(),
            layout_presets: Vec::new(),
            wallpaper: WallpaperPreferences::default(),
            keyboard_backlight: KeyboardBacklightPreferences::default(),
        }
    }
}

const MAX_DEBUG_LOG_SIZE: u64 = 1024 * 1024; // 1 MB

/// Returns the path to the debug log file in the app's config directory.
/// Dev builds use `debug-dev.log`, production builds use `debug.log`
/// so local testing and installed app logs don't intermix.
pub fn debug_log_path() -> PathBuf {
    let filename = if env!("IS_DEV_BUILD") == "true" {
        "debug-dev.log"
    } else {
        "debug.log"
    };
    config_dir().join(filename)
}

/// Global "is debug logging on" flag for context-free callers (the `log::Log`
/// fanout in `lib.rs`, modules under `core/*`, etc. — anywhere we can't reach
/// `AppState`). `lib.rs::run()` sets this from `preferences.debug_logging`
/// after loading prefs, and `save_preferences` re-syncs it on every save.
///
/// We keep `write_debug_log(state, …)` as the canonical entry point for code
/// that already holds an `AppState`, and `write_debug_log_unbound(…)` for
/// everything else. Both honor this flag and append to the same file, so the
/// resulting log is interleaved in chronological order regardless of which
/// helper produced each line.
pub static DEBUG_LOG_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Append a line to the debug log without an `AppState`. Honors
/// `DEBUG_LOG_ENABLED`; no-op when disabled. Used by the `log::Log` tee
/// installed in `lib.rs::run()` and by direct callers in `core::*` that
/// can't take `AppState` because they live below the Tauri layer.
pub fn write_debug_log_unbound(message: &str) {
    if !DEBUG_LOG_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let path = debug_log_path();
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] {}\n", timestamp, message);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = file.write_all(line.as_bytes());
    }
}

/// Appends a timestamped message to the debug log (no-op if debug logging is disabled).
/// Auto-truncates the log file when it exceeds 1 MB, keeping the last 80%.
/// Uses try_lock to avoid blocking callers on high-frequency paths (e.g., CGEventTap
/// callback) — if the preferences mutex is contended, the log message is silently dropped.
pub fn write_debug_log(state: &crate::AppState, message: &str) {
    let enabled = state
        .preferences
        .try_lock()
        .map(|p| p.debug_logging)
        .unwrap_or(false);
    if !enabled {
        return;
    }

    let path = debug_log_path();

    // When over the size limit, trim the beginning and keep the last 80%
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_DEBUG_LOG_SIZE {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let keep = content.len() * 80 / 100;
                let trim_at = content.len() - keep;
                // Find the next newline after the trim point to avoid splitting a line
                let start = content[trim_at..].find('\n').map(|i| trim_at + i + 1).unwrap_or(trim_at);
                std::fs::write(&path, &content[start..]).ok();
            }
        }
    }

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] {}\n", timestamp, message);

    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Test-only global lock that serializes any test which mutates the
/// `DISPLAY_DJ_CONFIG_DIR` env var. Multiple test modules
/// (display::tests, wallpaper::tests, etc.) point this env var at their
/// own tempdir; without a shared lock they race because env vars are
/// process-global. Each tempdir helper must acquire this same lock.
#[cfg(test)]
pub(crate) static TEST_CONFIG_DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Returns the app's config directory (creates it if it doesn't exist).
///
/// In tests, the `DISPLAY_DJ_CONFIG_DIR` env var can override the default location
/// so disk-write tests don't pollute the dev's real config dir.
pub(crate) fn config_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var("DISPLAY_DJ_CONFIG_DIR") {
        let dir = PathBuf::from(override_dir);
        std::fs::create_dir_all(&dir).ok();
        return dir;
    }
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("display-dj");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Returns the path to the preferences.json file.
fn preferences_path() -> PathBuf {
    config_dir().join("preferences.json")
}

/// Loads preferences from disk, falling back to defaults on missing/malformed JSON.
/// Runs the legacy monitor-configs migration if needed.
pub fn load_preferences() -> Preferences {
    let path = preferences_path();
    let mut prefs = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => {
            let prefs = Preferences::default();
            save_preferences_to_disk(&prefs);
            prefs
        }
    };
    migrate_monitor_configs_if_needed(&mut prefs);
    migrate_expose_grid_if_needed(&mut prefs);
    prefs
}

/// Serializes preferences to pretty JSON and writes to disk.
pub fn save_preferences_to_disk(prefs: &Preferences) {
    let path = preferences_path();
    if let Ok(json) = serde_json::to_string_pretty(prefs) {
        std::fs::write(path, json).ok();
    }
}

/// One-time migration: reads old `monitor-configs.json`, converts entries to
/// `MonitorMetadata`, and stores them in `preferences.monitor_configs`.
/// Renames the old file to `.migrated.json` so migration only runs once.
fn migrate_monitor_configs_if_needed(prefs: &mut Preferences) {
    use std::collections::HashMap;

    let old_path = config_dir().join("monitor-configs.json");
    if !old_path.exists() || !prefs.monitor_configs.is_empty() {
        return;
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OldMonitorConfig {
        id: String,
        name: String,
        sort_order: i32,
        disabled: bool,
    }

    let content = match std::fs::read_to_string(&old_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let old_configs: HashMap<String, OldMonitorConfig> = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(_) => return,
    };

    for (_key, old) in old_configs {
        if old.disabled {
            continue; // drop disabled entries during migration
        }
        prefs.monitor_configs.push(MonitorMetadata {
            uid: format!("{}::unknown", old.id),
            api_id: old.id,
            api_name: "unknown".into(),
            label: old.name,
            sort_order: old.sort_order,
            hidden: false,
            brightness_mode: default_brightness_mode(),
        });
    }

    save_preferences_to_disk(prefs);

    // Rename old file so migration doesn't re-run
    let migrated_path = config_dir().join("monitor-configs.migrated.json");
    std::fs::rename(&old_path, &migrated_path).ok();
}

/// Migrates the legacy `expose_max_windows` (squared total) into the new
/// `expose_columns` / `expose_rows` fields. Runs once: when the old field is
/// present and non-zero, and the new fields are still at their defaults.
fn migrate_expose_grid_if_needed(prefs: &mut Preferences) {
    let old = prefs.tiling.expose_max_windows;
    let defaults = TilingPreferences::default();
    if old == 0 {
        return; // old field not present in the config
    }
    if prefs.tiling.expose_columns != defaults.expose_columns
        || prefs.tiling.expose_rows != defaults.expose_rows
    {
        return; // new fields already set by the user
    }
    let dim = (old as f64).sqrt().round() as u32;
    prefs.tiling.expose_columns = dim;
    prefs.tiling.expose_rows = dim;
    prefs.tiling.expose_max_windows = 0; // clear legacy field
    save_preferences_to_disk(prefs);
}

/// Backs up the current preferences file and resets to defaults.
pub fn reset_to_defaults() {
    let now = chrono::Local::now().format("%Y%m%d_%H%M%S");

    // Backup and reset preferences
    let prefs_path = preferences_path();
    if prefs_path.exists() {
        let backup = config_dir().join(format!("preferences.bak_{}.json", now));
        std::fs::copy(&prefs_path, &backup).ok();
    }
    save_preferences_to_disk(&Preferences::default());
}

// -- Tauri commands --

/// Returns the current in-memory preferences to the frontend.
///
/// WARNING: Do NOT use `write_debug_log()` here. This is a sync command called
/// frequently by the frontend. `write_debug_log` locks `state.preferences` to
/// check `debug_logging`, and that extra mutex contention on a sync Tauri command
/// starves the macOS main-thread run-loop — breaking tray icon click events.
/// Use `log::info!` instead.
#[tauri::command]
pub fn get_preferences(state: tauri::State<'_, crate::AppState>) -> Result<Preferences, String> {
    let t0 = std::time::Instant::now();
    // Log START before acquiring the lock (write_debug_log uses try_lock internally,
    // so it works here since we haven't locked yet).
    write_debug_log(&state, "benchmark: get_preferences — START");
    let prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    let result = prefs.clone();
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
    let n_monitors = result.monitor_configs.len();
    let n_profiles = result.profiles.len();
    // Drop the lock BEFORE logging the END so write_debug_log can acquire it.
    drop(prefs);
    write_debug_log(
        &state,
        &format!(
            "benchmark: get_preferences — {:.1}ms (in-memory, {} monitors, {} profiles)",
            elapsed, n_monitors, n_profiles,
        ),
    );
    Ok(result)
}

/// Saves updated preferences from the frontend, syncs autostart with the OS, and persists to disk.
///
/// WARNING: This MUST remain `async`. Changing to sync `pub fn` causes Tauri
/// on macOS to run the command on a blocking thread that starves the
/// main-thread run-loop, preventing `on_tray_icon_event` from ever firing —
/// both left-click and right-click on the tray icon stop working entirely.
/// (See dropped commit 57a1704 for the broken version.)
///
/// Note: `write_debug_log()` is safe here (unlike in `get_preferences`) because
/// this is async and only called on explicit user save, so the brief mutex lock
/// does not create contention with the main-thread run-loop.
#[tauri::command]
pub async fn save_preferences(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    preferences: Preferences,
) -> Result<(), String> {
    write_debug_log(
        &state,
        &format!(
            "save_preferences: show_contrast={} min_brightness={} launch_at_login={} debug_logging={} monitors={} profiles={} night_mode={}",
            preferences.show_contrast,
            preferences.min_brightness,
            preferences.launch_at_login,
            preferences.debug_logging,
            preferences.monitor_configs.len(),
            preferences.profiles.len(),
            preferences.night_mode_schedule.enabled,
        ),
    );

    // Keep the AppState-less tee gate in lock-step with the saved preference
    // so toggling debug logging in Settings takes effect immediately for
    // `log::info!` calls in `core/*` (which can't see `AppState`).
    DEBUG_LOG_ENABLED.store(
        preferences.debug_logging,
        std::sync::atomic::Ordering::Relaxed,
    );

    // Save to disk and update in-memory state first so the UI isn't blocked
    // even if autostart hangs
    let old_launch_at_login = state
        .preferences
        .lock()
        .map(|p| p.launch_at_login)
        .unwrap_or(false);

    save_preferences_to_disk(&preferences);
    log::info!("save_preferences: written to disk");

    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    *prefs = preferences.clone();
    drop(prefs); // release lock before autostart call
    log::info!("save_preferences: in-memory state updated");

    // Sync autostart with OS only when the value actually changed
    // (autostart.enable/disable can hang on some platforms)
    if preferences.launch_at_login != old_launch_at_login {
        log::info!("save_preferences: autostart changed {} -> {}, syncing", old_launch_at_login, preferences.launch_at_login);
        use tauri_plugin_autostart::ManagerExt;
        let autostart = app.autolaunch();
        let result = if preferences.launch_at_login {
            autostart.enable()
        } else {
            autostart.disable()
        };
        if let Err(e) = result {
            // Non-fatal: preferences are already saved, don't fail the whole operation
            log::error!("save_preferences: autostart failed: {}", e);
        }
        log::info!("save_preferences: autostart synced");
    } else {
        log::info!("save_preferences: launch_at_login unchanged, skipping autostart");
    }

    log::info!("save_preferences: done");
    Ok(())
}

/// Opens the preferences.json file in the OS default editor.
#[tauri::command]
pub fn open_preferences_file() -> Result<(), String> {
    let path = preferences_path();
    // Ensure the file exists before trying to open it
    if !path.exists() {
        save_preferences_to_disk(&Preferences::default());
    }
    open::that(path).map_err(|e| e.to_string())
}

/// Opens the debug.log file in the OS default editor.
#[tauri::command]
pub fn open_debug_log() -> Result<(), String> {
    let path = debug_log_path();
    if !path.exists() {
        std::fs::write(&path, "").ok();
    }
    open::that(path).map_err(|e| e.to_string())
}

/// Opens the app's config directory in the OS file browser.
/// This lets users browse the folder where preferences.json, debug.log, and other
/// app files are stored (e.g. ~/Library/Application Support/display-dj/ on macOS).
#[tauri::command]
pub fn open_app_folder() -> Result<(), String> {
    let dir = config_dir();
    open::that(dir).map_err(|e| e.to_string())
}

/// Set debug_logging in preferences and persist to disk. Used by tray Debug submenu.
pub fn set_debug_logging(state: &crate::AppState, enabled: bool) {
    if let Ok(mut prefs) = state.preferences.lock() {
        prefs.debug_logging = enabled;
        save_preferences_to_disk(&prefs);
    }
}

/// Set tiling.enabled in preferences and persist to disk. Used by tray Tiling submenu.
pub fn set_tiling_enabled(state: &crate::AppState, enabled: bool) {
    if let Ok(mut prefs) = state.preferences.lock() {
        prefs.tiling.enabled = enabled;
        save_preferences_to_disk(&prefs);
    }
}

/// Set tiling.expose_enabled in preferences and persist to disk. Used by tray Exposé submenu.
pub fn set_expose_enabled(state: &crate::AppState, enabled: bool) {
    if let Ok(mut prefs) = state.preferences.lock() {
        prefs.tiling.expose_enabled = enabled;
        save_preferences_to_disk(&prefs);
    }
}

/// Returns the app version string with architecture suffix (e.g. "5.0.0 (arm64)").
/// Version is set at compile time; architecture is detected via `std::env::consts::ARCH`.
#[tauri::command]
pub fn get_app_version() -> String {
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    };
    format!("{} ({})", env!("APP_VERSION"), arch)
}

/// Returns structured about info for the About panel: version, engine,
/// architecture, OS, build date, and homepage URL.
#[tauri::command]
pub fn get_about_info() -> std::collections::HashMap<String, String> {
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    };
    let os = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    };
    let mut info = std::collections::HashMap::new();
    info.insert("version".into(), env!("APP_VERSION").to_string());
    // v7.0.0+ has no sidecar — platform code is vendored in-process under src/core/.
    info.insert("engine".into(), "Tauri + Rust (in-process)".to_string());
    info.insert("arch".into(), arch.to_string());
    info.insert("os".into(), os.to_string());
    info.insert("buildDate".into(), env!("BUILD_DATE").to_string());
    info.insert(
        "homepage".into(),
        "https://github.com/synle/display-dj".to_string(),
    );
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_preferences() {
        let prefs = Preferences::default();
        assert!(!prefs.show_individual_displays);
        assert_eq!(prefs.min_brightness, 10);
        assert_eq!(prefs.key_bindings.len(), 28);
    }

    #[test]
    fn test_effective_min_brightness() {
        let mut prefs = Preferences::default();
        assert_eq!(prefs.effective_min_brightness(), 10);

        prefs.min_brightness = 20;
        assert_eq!(prefs.effective_min_brightness(), 20);

        // Below absolute floor should clamp to 5
        prefs.min_brightness = 3;
        assert_eq!(prefs.effective_min_brightness(), ABSOLUTE_MIN_BRIGHTNESS);

        prefs.min_brightness = 0;
        assert_eq!(prefs.effective_min_brightness(), ABSOLUTE_MIN_BRIGHTNESS);
    }

    #[test]
    fn test_preferences_missing_new_fields_uses_defaults() {
        // Simulates loading an old preferences.json that lacks the new fields
        let json = r#"{
            "showIndividualDisplays": true,
            "keyBindings": []
        }"#;
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert!(prefs.show_individual_displays);
        assert_eq!(prefs.min_brightness, 10);
    }

    #[test]
    fn test_default_keybindings_keys() {
        let prefs = Preferences::default();
        let keys: Vec<&str> = prefs.key_bindings.iter().map(|kb| kb.key.as_str()).collect();
        assert_eq!(keys[0], "Shift+Escape");
        assert_eq!(keys[1], "Shift+F1");
        assert_eq!(keys[7], "Shift+F11");
        assert_eq!(keys[8], "Shift+F12");
    }

    /// Shift+F2 is a combo binding: it must run BOTH changeProfile/2 and
    /// changeKeyboardBacklight/0. Single-command form is a regression.
    #[test]
    fn test_shift_f2_includes_keyboard_backlight_zero() {
        let prefs = Preferences::default();
        let f2 = prefs
            .key_bindings
            .iter()
            .find(|kb| kb.key == "Shift+F2")
            .expect("Shift+F2 default binding must exist");
        match &f2.command {
            CommandValue::Multiple(cmds) => {
                assert!(
                    cmds.iter().any(|c| c == "command/changeProfile/2"),
                    "Shift+F2 must still trigger profile 2 (Focus): {:?}",
                    cmds
                );
                assert!(
                    cmds.iter().any(|c| c == "command/changeKeyboardBacklight/0"),
                    "Shift+F2 must also dim the keyboard backlight to 0: {:?}",
                    cmds
                );
            }
            CommandValue::Single(s) => panic!(
                "Shift+F2 must be CommandValue::Multiple now, got Single({:?})",
                s
            ),
        }
    }

    /// Old preferences.json files written before keyboardBacklight existed
    /// must deserialize with enabled = true (opt-out, not opt-in).
    #[test]
    fn test_keyboard_backlight_missing_field_defaults_enabled() {
        let json = r#"{ "keyBindings": [] }"#;
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert!(prefs.keyboard_backlight.enabled);
    }

    /// KeyboardBacklightPreferences roundtrips through serde with camelCase.
    #[test]
    fn test_keyboard_backlight_preferences_serde_roundtrip() {
        let kb = KeyboardBacklightPreferences { enabled: false };
        let json = serde_json::to_string(&kb).unwrap();
        assert!(json.contains("\"enabled\":false"));
        let parsed: KeyboardBacklightPreferences = serde_json::from_str(&json).unwrap();
        assert!(!parsed.enabled);
    }

    /// Default KeyboardBacklightPreferences is enabled = true (beta opt-out).
    #[test]
    fn test_keyboard_backlight_preferences_default_enabled() {
        let kb = KeyboardBacklightPreferences::default();
        assert!(kb.enabled);
    }

    #[test]
    fn test_command_value_single_serialization() {
        let cmd = CommandValue::Single("command/changeBrightness/50".into());
        let json = serde_json::to_string(&cmd).unwrap();
        assert_eq!(json, "\"command/changeBrightness/50\"");
    }

    #[test]
    fn test_command_value_multiple_serialization() {
        let cmd = CommandValue::Multiple(vec![
            "command/changeDarkMode/dark".into(),
            "command/changeBrightness/10".into(),
        ]);
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("command/changeDarkMode/dark"));
        assert!(json.contains("command/changeBrightness/10"));
    }

    #[test]
    fn test_command_value_single_deserialization() {
        let json = "\"command/changeBrightness/100\"";
        let cmd: CommandValue = serde_json::from_str(json).unwrap();
        match cmd {
            CommandValue::Single(s) => assert_eq!(s, "command/changeBrightness/100"),
            _ => panic!("Expected Single variant"),
        }
    }

    #[test]
    fn test_command_value_multiple_deserialization() {
        let json = "[\"command/changeDarkMode/dark\",\"command/changeBrightness/10\"]";
        let cmd: CommandValue = serde_json::from_str(json).unwrap();
        match cmd {
            CommandValue::Multiple(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], "command/changeDarkMode/dark");
                assert_eq!(v[1], "command/changeBrightness/10");
            }
            _ => panic!("Expected Multiple variant"),
        }
    }

    #[test]
    fn test_preferences_roundtrip_serialization() {
        let prefs = Preferences::default();
        let json = serde_json::to_string_pretty(&prefs).unwrap();
        let deserialized: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.show_individual_displays, prefs.show_individual_displays);
        assert_eq!(deserialized.min_brightness, prefs.min_brightness);
        assert_eq!(deserialized.key_bindings.len(), prefs.key_bindings.len());
    }

    #[test]
    fn test_preferences_camel_case_serialization() {
        let prefs = Preferences::default();
        let json = serde_json::to_string(&prefs).unwrap();
        assert!(json.contains("showIndividualDisplays"));
        assert!(json.contains("minBrightness"));
        assert!(json.contains("keyBindings"));
        assert!(json.contains("monitorConfigs"));
        // Should NOT contain snake_case
        assert!(!json.contains("show_individual_displays"));
        assert!(!json.contains("min_brightness"));
        assert!(!json.contains("monitor_configs"));
    }

    #[test]
    fn test_monitor_metadata_serialization() {
        let meta = MonitorMetadata {
            uid: "1::Dell U2723QE".into(),
            api_id: "1".into(),
            api_name: "Dell U2723QE".into(),
            label: "Main Monitor".into(),
            sort_order: 0,
            hidden: false,
            brightness_mode: default_brightness_mode(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("sortOrder"));
        assert!(json.contains("apiId"));
        assert!(json.contains("apiName"));
        assert!(json.contains("brightnessMode"));
        assert!(!json.contains("sort_order"));
        assert!(!json.contains("api_id"));
        assert!(!json.contains("brightness_mode"));
    }

    #[test]
    fn test_monitor_metadata_roundtrip() {
        let meta = MonitorMetadata {
            uid: "builtin::Built-in Display".into(),
            api_id: "builtin".into(),
            api_name: "Built-in Display".into(),
            label: "MacBook Screen".into(),
            sort_order: 0,
            hidden: false,
            brightness_mode: default_brightness_mode(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let restored: MonitorMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, restored);
    }

    #[test]
    fn test_preferences_with_monitor_configs_roundtrip() {
        let mut prefs = Preferences::default();
        prefs.monitor_configs = vec![
            MonitorMetadata {
                uid: "1::Dell".into(),
                api_id: "1".into(),
                api_name: "Dell".into(),
                label: "Left".into(),
                sort_order: 0,
                hidden: false,
                brightness_mode: default_brightness_mode(),
            },
            MonitorMetadata {
                uid: "2::LG".into(),
                api_id: "2".into(),
                api_name: "LG".into(),
                label: "".into(),
                sort_order: 1,
                hidden: false,
                brightness_mode: "overlay".into(),
            },
        ];
        let json = serde_json::to_string_pretty(&prefs).unwrap();
        let restored: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.monitor_configs.len(), 2);
        assert_eq!(restored.monitor_configs[0].uid, "1::Dell");
        assert_eq!(restored.monitor_configs[1].label, "");
    }

    #[test]
    fn test_monitor_metadata_deserialization_from_camel_case() {
        let json = r#"{
            "uid": "1::Dell U2723QE",
            "apiId": "1",
            "apiName": "Dell U2723QE",
            "label": "Main Monitor",
            "sortOrder": 3
        }"#;
        let meta: MonitorMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.uid, "1::Dell U2723QE");
        assert_eq!(meta.api_id, "1");
        assert_eq!(meta.api_name, "Dell U2723QE");
        assert_eq!(meta.label, "Main Monitor");
        assert_eq!(meta.sort_order, 3);
        // Missing brightnessMode in old configs must default to "auto"
        // (the soft-overlay fallback feature is opt-in via Settings, but the
        // auto-discovery path is the safe default for everyone).
        assert_eq!(meta.brightness_mode, "auto");
    }

    /// Verifies all four valid brightness_mode values serde-roundtrip cleanly.
    /// Acts as a guard against accidental enum renames — the four strings are
    /// the contract between the Rust backend, the TS frontend, and the
    /// preferences.json on disk.
    #[test]
    fn test_monitor_metadata_brightness_mode_all_modes_roundtrip() {
        for mode in &["auto", "ddc", "gamma", "overlay"] {
            let meta = MonitorMetadata {
                uid: "1::Test".into(),
                api_id: "1".into(),
                api_name: "Test".into(),
                label: "".into(),
                sort_order: 0,
                hidden: false,
                brightness_mode: (*mode).into(),
            };
            let json = serde_json::to_string(&meta).unwrap();
            assert!(
                json.contains(&format!("\"brightnessMode\":\"{}\"", mode)),
                "expected camelCase brightnessMode={} in JSON: {}",
                mode,
                json,
            );
            let restored: MonitorMetadata = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.brightness_mode, *mode);
        }
    }

    /// Verifies an explicit brightnessMode value deserializes from camelCase JSON.
    #[test]
    fn test_monitor_metadata_brightness_mode_deserializes_explicit() {
        let json = r#"{
            "uid": "1::Samsung",
            "apiId": "1",
            "apiName": "Samsung Smart Monitor",
            "label": "",
            "sortOrder": 0,
            "hidden": false,
            "brightnessMode": "overlay"
        }"#;
        let meta: MonitorMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.brightness_mode, "overlay");
    }

    #[test]
    fn test_default_preferences_has_empty_monitor_configs() {
        let prefs = Preferences::default();
        assert!(prefs.monitor_configs.is_empty());
    }

    #[test]
    fn test_preferences_missing_monitor_configs_defaults_to_empty() {
        let json = r#"{
            "showIndividualDisplays": true,
            "keyBindings": []
        }"#;
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert!(prefs.monitor_configs.is_empty());
    }

    #[test]
    fn test_preferences_file_roundtrip() {
        let dir = std::env::temp_dir().join("display-dj-test-prefs");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("test-preferences.json");

        let prefs = Preferences::default();
        let json = serde_json::to_string_pretty(&prefs).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded: Preferences =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.min_brightness, 10);
        assert_eq!(loaded.key_bindings.len(), 28);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_preferences_with_monitor_configs_file_roundtrip() {
        let dir = std::env::temp_dir().join("display-dj-test-configs-v2");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("test-preferences.json");

        let mut prefs = Preferences::default();
        prefs.monitor_configs = vec![MonitorMetadata {
            uid: "builtin::Built-in Display".into(),
            api_id: "builtin".into(),
            api_name: "Built-in Display".into(),
            label: "MacBook".into(),
            sort_order: 0,
            hidden: false,
            brightness_mode: default_brightness_mode(),
        }];
        let json = serde_json::to_string_pretty(&prefs).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded: Preferences =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.monitor_configs.len(), 1);
        assert_eq!(loaded.monitor_configs[0].label, "MacBook");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_get_app_version() {
        let version = get_app_version();
        assert!(!version.is_empty());
        // Should be a valid semver-like string with arch suffix
        assert!(version.contains('.'));
        assert!(version.contains('('));
        assert!(
            version.contains("arm64") || version.contains("x64"),
            "version should contain arch: {}",
            version
        );
    }

    #[test]
    fn test_malformed_json_returns_default_preferences() {
        let bad_json = "{ not valid json }";
        let result: Preferences = serde_json::from_str(bad_json).unwrap_or_default();
        assert_eq!(result.min_brightness, 10);
    }

    #[test]
    fn test_default_profiles() {
        let prefs = Preferences::default();
        assert_eq!(prefs.profiles.len(), 3);
        assert_eq!(prefs.profiles[0].name, "Presentation");
        assert_eq!(prefs.profiles[1].name, "Focus");
        assert_eq!(prefs.profiles[2].name, "Daylight");
    }

    #[test]
    fn test_profile_serialization() {
        let profile = Profile {
            name: "Test".into(),
            command: CommandValue::Multiple(vec![
                "command/changeBrightness/50".into(),
                "command/changeDarkMode/dark".into(),
            ]),
        };
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"command\""));
        assert!(json.contains("command/changeBrightness/50"));
    }

    #[test]
    fn test_profile_deserialization() {
        let json = r#"{
            "name": "Mute",
            "command": "command/changeVolume/0"
        }"#;
        let profile: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.name, "Mute");
        match profile.command {
            CommandValue::Single(s) => assert_eq!(s, "command/changeVolume/0"),
            _ => panic!("Expected Single variant"),
        }
    }

    #[test]
    fn test_profile_missing_name_uses_default() {
        let json = r#"{
            "command": "command/changeVolume/0"
        }"#;
        let profile: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.name, "");
    }

    #[test]
    fn test_preferences_missing_profiles_uses_defaults() {
        let json = r#"{
            "showIndividualDisplays": false,
            "keyBindings": []
        }"#;
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert_eq!(prefs.profiles.len(), 3);
        assert_eq!(prefs.profiles[0].name, "Presentation");
    }

    #[test]
    fn test_default_expose_grid_is_3x3() {
        let prefs = Preferences::default();
        assert_eq!(prefs.tiling.expose_columns, 3);
        assert_eq!(prefs.tiling.expose_rows, 3);
        assert_eq!(prefs.tiling.expose_max_windows, 0);
    }

    #[test]
    fn test_expose_grid_new_fields_roundtrip() {
        let mut prefs = Preferences::default();
        prefs.tiling.expose_columns = 2;
        prefs.tiling.expose_rows = 4;
        let json = serde_json::to_string_pretty(&prefs).unwrap();
        // expose_max_windows should NOT appear in serialized output
        assert!(!json.contains("exposeMaxWindows"));
        assert!(json.contains("exposeColumns"));
        assert!(json.contains("exposeRows"));
        let restored: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.tiling.expose_columns, 2);
        assert_eq!(restored.tiling.expose_rows, 4);
    }

    #[test]
    fn test_expose_grid_backward_compat_old_config() {
        // Simulates an old preferences.json that has exposeMaxWindows but not
        // exposeColumns/exposeRows. Serde fills the new fields with defaults.
        let json = r#"{
            "tiling": {
                "enabled": true,
                "exposeEnabled": true,
                "exposeMaxWindows": 25
            }
        }"#;
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        // Old field is deserialized for migration
        assert_eq!(prefs.tiling.expose_max_windows, 25);
        // New fields get defaults since they're missing from the JSON
        assert_eq!(prefs.tiling.expose_columns, 3);
        assert_eq!(prefs.tiling.expose_rows, 3);
    }

    #[test]
    fn test_migrate_expose_grid_from_old_config() {
        let mut prefs = Preferences::default();
        prefs.tiling.expose_max_windows = 25; // old 5x5
        prefs.tiling.expose_columns = 3; // still at default
        prefs.tiling.expose_rows = 3; // still at default
        migrate_expose_grid_if_needed(&mut prefs);
        assert_eq!(prefs.tiling.expose_columns, 5);
        assert_eq!(prefs.tiling.expose_rows, 5);
        assert_eq!(prefs.tiling.expose_max_windows, 0);
    }

    #[test]
    fn test_migrate_expose_grid_skips_when_new_fields_set() {
        let mut prefs = Preferences::default();
        prefs.tiling.expose_max_windows = 25;
        prefs.tiling.expose_columns = 2; // user already set new fields
        prefs.tiling.expose_rows = 4;
        migrate_expose_grid_if_needed(&mut prefs);
        // Should NOT overwrite user's new settings
        assert_eq!(prefs.tiling.expose_columns, 2);
        assert_eq!(prefs.tiling.expose_rows, 4);
    }

    #[test]
    fn test_migrate_expose_grid_skips_when_no_old_field() {
        let mut prefs = Preferences::default();
        // expose_max_windows defaults to 0 (not present in new configs)
        migrate_expose_grid_if_needed(&mut prefs);
        assert_eq!(prefs.tiling.expose_columns, 3);
        assert_eq!(prefs.tiling.expose_rows, 3);
    }

    #[test]
    fn test_default_preferences_has_empty_layout_presets() {
        let prefs = Preferences::default();
        assert!(prefs.layout_presets.is_empty());
    }

    #[test]
    fn test_layout_preset_roundtrip_serialization() {
        let mut prefs = Preferences::default();
        prefs.layout_presets.push(LayoutPreset {
            name: "Coding".into(),
            rules: vec![
                LayoutRule { app_match: "Chrome".into(), layout: "leftHalf".into(), display_index: None },
                LayoutRule { app_match: "VS Code".into(), layout: "rightHalf".into(), display_index: Some(0) },
            ],
        });
        let json = serde_json::to_string_pretty(&prefs).unwrap();
        assert!(json.contains("layoutPresets"));
        assert!(json.contains("appMatch"));
        assert!(json.contains("displayIndex"));
        let restored: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.layout_presets.len(), 1);
        assert_eq!(restored.layout_presets[0].name, "Coding");
        assert_eq!(restored.layout_presets[0].rules.len(), 2);
        assert_eq!(restored.layout_presets[0].rules[0].app_match, "Chrome");
        assert_eq!(restored.layout_presets[0].rules[1].display_index, Some(0));
    }

    #[test]
    fn test_preferences_missing_layout_presets_defaults_to_empty() {
        let json = r#"{
            "showIndividualDisplays": false,
            "keyBindings": []
        }"#;
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert!(prefs.layout_presets.is_empty());
    }

    #[test]
    fn test_night_mode_schedule_default_has_empty_commands() {
        let schedule = NightModeSchedule::default();
        assert!(schedule.night_commands.is_empty());
        assert!(schedule.day_commands.is_empty());
    }

    #[test]
    fn test_night_mode_schedule_with_commands_roundtrip() {
        let mut prefs = Preferences::default();
        prefs.night_mode_schedule.night_commands = vec![
            "command/changeBrightness/20".into(),
            "command/changeDarkMode/dark".into(),
        ];
        prefs.night_mode_schedule.day_commands = vec![
            "command/changeBrightness/100".into(),
            "command/changeDarkMode/light".into(),
            "command/changeVolume/50".into(),
        ];
        let json = serde_json::to_string_pretty(&prefs).unwrap();
        assert!(json.contains("nightCommands"));
        assert!(json.contains("dayCommands"));
        let restored: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.night_mode_schedule.night_commands.len(), 2);
        assert_eq!(restored.night_mode_schedule.day_commands.len(), 3);
        assert_eq!(restored.night_mode_schedule.day_commands[2], "command/changeVolume/50");
    }

    #[test]
    fn test_night_mode_schedule_backward_compat_missing_commands() {
        // Old config without nightCommands/dayCommands should default to empty
        let json = r#"{
            "nightModeSchedule": {
                "enabled": true,
                "nightStart": "21:00",
                "nightBrightness": 20,
                "dayStart": "07:00",
                "dayBrightness": 100
            }
        }"#;
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert!(prefs.night_mode_schedule.enabled);
        assert!(prefs.night_mode_schedule.night_commands.is_empty());
        assert!(prefs.night_mode_schedule.day_commands.is_empty());
    }

    /// Verifies config_dir() returns a path ending with "display-dj" and that
    /// the directory is created on disk (used by open_app_folder).
    #[test]
    fn test_config_dir_exists_and_named_correctly() {
        let dir = super::config_dir();
        assert!(dir.ends_with("display-dj"));
        assert!(dir.exists(), "config_dir() should create the directory");
    }

    /// Verifies WallpaperPreferences defaults to fit="fill" and no current path.
    #[test]
    fn test_wallpaper_preferences_default() {
        let wp = WallpaperPreferences::default();
        assert_eq!(wp.fit, "fill");
        assert!(wp.current_wallpaper_path.is_none());
    }

    /// Verifies WallpaperPreferences serializes/deserializes with camelCase.
    #[test]
    fn test_wallpaper_preferences_roundtrip() {
        let wp = WallpaperPreferences {
            fit: "center".into(),
            current_wallpaper_path: Some("/tmp/wallpaper.jpg".into()),
            per_monitor_wallpapers: Vec::new(),
            slideshow_enabled: false,
            slideshow_folder: None,
            slideshow_interval_minutes: 30,
            slideshow_order: "forward".into(),
        };
        let json = serde_json::to_string(&wp).unwrap();
        assert!(json.contains("currentWallpaperPath"));
        let restored: WallpaperPreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.fit, "center");
        assert_eq!(restored.current_wallpaper_path.as_deref(), Some("/tmp/wallpaper.jpg"));
    }

    /// Verifies old configs missing the wallpaper field get defaults.
    #[test]
    fn test_preferences_missing_wallpaper_defaults() {
        let json = r#"{
            "showIndividualDisplays": false,
            "keyBindings": []
        }"#;
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert_eq!(prefs.wallpaper.fit, "fill");
        assert!(prefs.wallpaper.current_wallpaper_path.is_none());
    }
}
