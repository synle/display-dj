use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Absolute floor for brightness — never allow less than this regardless of user config.
pub const ABSOLUTE_MIN_BRIGHTNESS: u32 = 5;

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
}

impl Default for NightModeSchedule {
    fn default() -> Self {
        Self {
            enabled: false,
            night_start: "21:00".into(),
            night_brightness: 20,
            day_start: "07:00".into(),
            day_brightness: 100,
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
                    command: CommandValue::Multiple(vec![
                        "command/changeDarkMode/dark".into(),
                        "command/changeBrightness/10".into(),
                    ]),
                },
                KeyBinding {
                    key: "Shift+F2".into(),
                    command: CommandValue::Multiple(vec![
                        "command/changeDarkMode/light".into(),
                        "command/changeBrightness/100".into(),
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
                        "command/changeBrightness/80".into(),
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
        }
    }
}

const MAX_DEBUG_LOG_SIZE: u64 = 1024 * 1024; // 1 MB

/// Returns the path to the debug log file in the app's config directory.
pub fn debug_log_path() -> PathBuf {
    config_dir().join("debug.log")
}

/// Appends a timestamped message to the debug log (no-op if debug logging is disabled).
/// Auto-truncates the log file when it exceeds 1 MB, keeping the last 80%.
pub fn write_debug_log(state: &crate::AppState, message: &str) {
    let enabled = state
        .preferences
        .lock()
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

/// Returns the app's config directory (creates it if it doesn't exist).
fn config_dir() -> PathBuf {
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
        });
    }

    save_preferences_to_disk(prefs);

    // Rename old file so migration doesn't re-run
    let migrated_path = config_dir().join("monitor-configs.migrated.json");
    std::fs::rename(&old_path, &migrated_path).ok();
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
#[tauri::command]
pub fn get_preferences(state: tauri::State<'_, crate::AppState>) -> Result<Preferences, String> {
    let prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    write_debug_log(
        &state,
        &format!(
            "get_preferences: show_contrast={} min_brightness={} monitors={} profiles={}",
            prefs.show_contrast, prefs.min_brightness, prefs.monitor_configs.len(), prefs.profiles.len()
        ),
    );
    Ok(prefs.clone())
}

/// Saves updated preferences from the frontend, syncs autostart with the OS, and persists to disk.
#[tauri::command]
pub fn save_preferences(
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

    // Save to disk and update in-memory state first so the UI isn't blocked
    save_preferences_to_disk(&preferences);
    write_debug_log(&state, "save_preferences: written to disk");

    // Check if launch_at_login actually changed before calling autostart
    // (autostart.enable/disable can hang on some platforms)
    let old_launch_at_login = state
        .preferences
        .lock()
        .map(|p| p.launch_at_login)
        .unwrap_or(false);

    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    *prefs = preferences.clone();
    drop(prefs); // release lock before autostart call
    write_debug_log(&state, "save_preferences: in-memory state updated");

    // Only sync autostart when the value actually changed
    if preferences.launch_at_login != old_launch_at_login {
        write_debug_log(
            &state,
            &format!(
                "save_preferences: autostart changed {} -> {}, syncing",
                old_launch_at_login, preferences.launch_at_login
            ),
        );
        use tauri_plugin_autostart::ManagerExt;
        let autostart = app.autolaunch();
        let result = if preferences.launch_at_login {
            autostart.enable()
        } else {
            autostart.disable()
        };
        if let Err(e) = result {
            write_debug_log(&state, &format!("save_preferences: autostart error: {}", e));
            log::error!("save_preferences: autostart failed: {}", e);
            // Non-fatal: preferences are already saved, don't fail the whole operation
        }
        write_debug_log(&state, "save_preferences: autostart synced");
    } else {
        write_debug_log(&state, "save_preferences: launch_at_login unchanged, skipping autostart");
    }

    write_debug_log(&state, "save_preferences: done");
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

/// Set debug_logging in preferences and persist to disk. Used by tray Debug submenu.
pub fn set_debug_logging(state: &crate::AppState, enabled: bool) {
    if let Ok(mut prefs) = state.preferences.lock() {
        prefs.debug_logging = enabled;
        save_preferences_to_disk(&prefs);
    }
}

/// Returns the app version string (set at compile time from package.json).
#[tauri::command]
pub fn get_app_version() -> String {
    env!("APP_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_preferences() {
        let prefs = Preferences::default();
        assert!(!prefs.show_individual_displays);
        assert_eq!(prefs.min_brightness, 10);
        assert_eq!(prefs.key_bindings.len(), 9);
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
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("sortOrder"));
        assert!(json.contains("apiId"));
        assert!(json.contains("apiName"));
        assert!(!json.contains("sort_order"));
        assert!(!json.contains("api_id"));
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
            },
            MonitorMetadata {
                uid: "2::LG".into(),
                api_id: "2".into(),
                api_name: "LG".into(),
                label: "".into(),
                sort_order: 1,
                hidden: false,
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
        assert_eq!(loaded.key_bindings.len(), 9);

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
        // Should be a valid semver-like string
        assert!(version.contains('.'));
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
}
