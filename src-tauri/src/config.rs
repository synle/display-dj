use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Absolute floor for brightness — never allow less than this regardless of user config.
pub const ABSOLUTE_MIN_BRIGHTNESS: u32 = 5;

pub type MonitorConfigs = HashMap<String, MonitorConfig>;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MonitorConfig {
    pub id: String,
    pub name: String,
    pub sort_order: i32,
    pub disabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub show_individual_displays: bool,
    pub brightness_delta: u32,
    pub contrast_delta: u32,
    pub min_brightness: u32,
    pub key_bindings: Vec<KeyBinding>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KeyBinding {
    pub key: String,
    pub command: CommandValue,
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
            brightness_delta: 50,
            contrast_delta: 25,
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
        }
    }
}

fn config_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("display-dj");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn preferences_path() -> PathBuf {
    config_dir().join("preferences.json")
}

fn monitor_configs_path() -> PathBuf {
    config_dir().join("monitor-configs.json")
}

pub fn load_preferences() -> Preferences {
    let path = preferences_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => {
            let prefs = Preferences::default();
            save_preferences_to_disk(&prefs);
            prefs
        }
    }
}

pub fn save_preferences_to_disk(prefs: &Preferences) {
    let path = preferences_path();
    if let Ok(json) = serde_json::to_string_pretty(prefs) {
        std::fs::write(path, json).ok();
    }
}

pub fn load_monitor_configs() -> MonitorConfigs {
    let path = monitor_configs_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

pub fn save_monitor_configs_to_disk(configs: &MonitorConfigs) {
    let path = monitor_configs_path();
    if let Ok(json) = serde_json::to_string_pretty(configs) {
        std::fs::write(path, json).ok();
    }
}

pub fn reset_to_defaults() {
    let now = chrono::Local::now().format("%Y%m%d_%H%M%S");

    // Backup and reset preferences
    let prefs_path = preferences_path();
    if prefs_path.exists() {
        let backup = config_dir().join(format!("preferences.bak_{}.json", now));
        std::fs::copy(&prefs_path, &backup).ok();
    }
    save_preferences_to_disk(&Preferences::default());

    // Backup and reset monitor configs
    let configs_path = monitor_configs_path();
    if configs_path.exists() {
        let backup = config_dir().join(format!("monitor-configs.bak_{}.json", now));
        std::fs::copy(&configs_path, &backup).ok();
    }
    save_monitor_configs_to_disk(&HashMap::new());
}

// -- Tauri commands --

#[tauri::command]
pub fn get_preferences(state: tauri::State<'_, crate::AppState>) -> Result<Preferences, String> {
    let prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    Ok(prefs.clone())
}

#[tauri::command]
pub fn save_preferences(
    state: tauri::State<'_, crate::AppState>,
    preferences: Preferences,
) -> Result<(), String> {
    save_preferences_to_disk(&preferences);
    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    *prefs = preferences;
    Ok(())
}

#[tauri::command]
pub fn get_monitor_configs(
    state: tauri::State<'_, crate::AppState>,
) -> Result<MonitorConfigs, String> {
    let configs = state.monitor_configs.lock().map_err(|e| e.to_string())?;
    Ok(configs.clone())
}

#[tauri::command]
pub fn save_monitor_config(
    state: tauri::State<'_, crate::AppState>,
    config: MonitorConfig,
) -> Result<(), String> {
    let mut configs = state.monitor_configs.lock().map_err(|e| e.to_string())?;
    configs.insert(config.id.clone(), config);
    save_monitor_configs_to_disk(&configs);
    Ok(())
}

#[tauri::command]
pub fn open_config_file() -> Result<(), String> {
    let path = monitor_configs_path();
    // Ensure the file exists before trying to open it
    if !path.exists() {
        save_monitor_configs_to_disk(&HashMap::new());
    }
    open::that(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_preferences_file() -> Result<(), String> {
    let path = preferences_path();
    // Ensure the file exists before trying to open it
    if !path.exists() {
        save_preferences_to_disk(&Preferences::default());
    }
    open::that(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_preferences() {
        let prefs = Preferences::default();
        assert!(!prefs.show_individual_displays);
        assert_eq!(prefs.brightness_delta, 50);
        assert_eq!(prefs.contrast_delta, 25);
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
            "brightnessDelta": 30,
            "keyBindings": []
        }"#;
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert!(prefs.show_individual_displays);
        assert_eq!(prefs.brightness_delta, 30);
        assert_eq!(prefs.contrast_delta, 25);
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
        assert_eq!(deserialized.brightness_delta, prefs.brightness_delta);
        assert_eq!(deserialized.key_bindings.len(), prefs.key_bindings.len());
    }

    #[test]
    fn test_preferences_camel_case_serialization() {
        let prefs = Preferences::default();
        let json = serde_json::to_string(&prefs).unwrap();
        assert!(json.contains("showIndividualDisplays"));
        assert!(json.contains("brightnessDelta"));
        assert!(json.contains("contrastDelta"));
        assert!(json.contains("minBrightness"));
        assert!(json.contains("keyBindings"));
        // Should NOT contain snake_case
        assert!(!json.contains("show_individual_displays"));
        assert!(!json.contains("brightness_delta"));
        assert!(!json.contains("contrast_delta"));
        assert!(!json.contains("min_brightness"));
    }

    #[test]
    fn test_monitor_config_serialization() {
        let config = MonitorConfig {
            id: "external-1".into(),
            name: "My Monitor".into(),
            sort_order: 1,
            disabled: false,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("sortOrder"));
        assert!(!json.contains("sort_order"));
    }

    #[test]
    fn test_monitor_configs_hashmap() {
        let mut configs: MonitorConfigs = HashMap::new();
        configs.insert(
            "external-1".into(),
            MonitorConfig {
                id: "external-1".into(),
                name: "Dell".into(),
                sort_order: 0,
                disabled: false,
            },
        );
        let json = serde_json::to_string(&configs).unwrap();
        let deserialized: MonitorConfigs = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 1);
        assert_eq!(deserialized["external-1"].name, "Dell");
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
        assert_eq!(loaded.brightness_delta, 50);
        assert_eq!(loaded.key_bindings.len(), 9);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_monitor_configs_file_roundtrip() {
        let dir = std::env::temp_dir().join("display-dj-test-configs");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("test-monitor-configs.json");

        let mut configs: MonitorConfigs = HashMap::new();
        configs.insert(
            "builtin-0".into(),
            MonitorConfig {
                id: "builtin-0".into(),
                name: "Built-in".into(),
                sort_order: 0,
                disabled: false,
            },
        );
        let json = serde_json::to_string_pretty(&configs).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded: MonitorConfigs =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded["builtin-0"].name, "Built-in");

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
        assert_eq!(result.brightness_delta, 50);
    }
}
