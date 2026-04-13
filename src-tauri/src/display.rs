use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    /// Raw API id from display-dj sidecar (e.g. "1", "builtin"). Used for brightness commands.
    pub id: String,
    /// Composite unique key: "{api_id}::{api_name}". Used for config lookups.
    pub uid: String,
    /// Display label (custom label from config, or api_name if no custom label).
    pub name: String,
    /// Original model name from the API (never changes).
    pub original_name: String,
    pub brightness: u32,
    /// Current contrast level (None for displays that don't support DDC contrast).
    pub contrast: Option<u32>,
    pub supports_brightness: bool,
    pub is_built_in: bool,
    #[serde(default)]
    pub hidden: bool,
}

/// Response from the display-dj HTTP server's /get_all and /list endpoints.
#[derive(Deserialize, Debug)]
struct DjDisplay {
    id: String,
    name: String,
    display_type: String,
    brightness: Option<u32>,
    contrast: Option<u32>,
}

impl DjDisplay {
    /// Converts a sidecar API response into the app's Monitor struct,
    /// computing the composite UID and defaulting brightness to 50 if unknown.
    fn into_monitor(self) -> Monitor {
        let is_built_in = self.display_type == "builtin";
        let uid = format!("{}::{}", self.id, self.name);
        Monitor {
            id: self.id,
            uid,
            name: self.name.clone(),
            original_name: self.name,
            brightness: self.brightness.unwrap_or(50),
            contrast: self.contrast,
            supports_brightness: true,
            is_built_in,
            hidden: false,
        }
    }
}

/// Returns the base URL of the display-dj sidecar HTTP server.
fn base_url() -> String {
    let port = crate::server_port();
    format!("http://127.0.0.1:{}", port)
}

/// Fetches all connected displays from the sidecar and converts them to Monitors.
async fn detect_monitors() -> Vec<Monitor> {
    let url = format!("{}/get_all", base_url());
    log::info!("detect_monitors: GET {}", url);
    let displays: Vec<DjDisplay> = match reqwest::get(&url).await {
        Ok(resp) => resp.json().await.unwrap_or_default(),
        Err(e) => {
            log::warn!("display-dj server request failed: {}", e);
            Vec::new()
        }
    };
    log::info!("detect_monitors: found {} displays", displays.len());
    displays.into_iter().map(|d| d.into_monitor()).collect()
}

/// Sets brightness for a single monitor via the sidecar, clamped to [min_brightness, 100].
async fn set_monitor_brightness(monitor_id: &str, value: u32, min_brightness: u32) -> Result<(), String> {
    let clamped = value.clamp(min_brightness, 100);
    let url = format!("{}/set_one/{}/{}", base_url(), monitor_id, clamped);
    log::info!("set_monitor_brightness: monitor_id={} value={} clamped={} GET {}", monitor_id, value, clamped, url);
    reqwest::get(&url).await
        .map_err(|e| format!("Failed to set brightness: {}", e))?;
    log::info!("set_monitor_brightness: done");
    Ok(())
}

/// Sets brightness for all monitors via the sidecar, clamped to [min_brightness, 100].
async fn set_all_monitors_brightness(value: u32, min_brightness: u32) -> Result<(), String> {
    let clamped = value.clamp(min_brightness, 100);
    let url = format!("{}/set_all/{}", base_url(), clamped);
    log::info!("set_all_monitors_brightness: value={} clamped={} GET {}", value, clamped, url);
    reqwest::get(&url).await
        .map_err(|e| format!("Failed to set all brightness: {}", e))?;
    log::info!("set_all_monitors_brightness: done");
    Ok(())
}

/// Sets contrast for a single monitor via the sidecar (0-100).
async fn set_monitor_contrast(monitor_id: &str, value: u32) -> Result<(), String> {
    let clamped = value.min(100);
    let url = format!("{}/set_contrast_one/{}/{}", base_url(), monitor_id, clamped);
    log::info!("set_monitor_contrast: monitor_id={} value={} GET {}", monitor_id, clamped, url);
    reqwest::get(&url).await
        .map_err(|e| format!("Failed to set contrast: {}", e))?;
    log::info!("set_monitor_contrast: done");
    Ok(())
}

/// Sets contrast for all monitors via the sidecar (0-100).
async fn set_all_monitors_contrast(value: u32) -> Result<(), String> {
    let clamped = value.min(100);
    let url = format!("{}/set_contrast_all/{}", base_url(), clamped);
    log::info!("set_all_monitors_contrast: value={} GET {}", clamped, url);
    reqwest::get(&url).await
        .map_err(|e| format!("Failed to set all contrast: {}", e))?;
    log::info!("set_all_monitors_contrast: done");
    Ok(())
}

// ===========================================================================
// Common helpers
// ===========================================================================

/// Applies saved metadata (custom labels, hidden state) to detected monitors
/// and sorts them by the user's configured sort order.
fn merge_with_configs(
    monitors: Vec<Monitor>,
    configs: &[crate::config::MonitorMetadata],
) -> Vec<Monitor> {
    let mut result: Vec<Monitor> = Vec::new();

    for mut monitor in monitors {
        if let Some(meta) = configs.iter().find(|m| m.uid == monitor.uid) {
            if !meta.label.is_empty() {
                monitor.name = meta.label.clone();
            }
            monitor.hidden = meta.hidden;
        }
        result.push(monitor);
    }

    result.sort_by(|a, b| {
        let order_a = configs.iter().find(|c| c.uid == a.uid).map(|c| c.sort_order).unwrap_or(i32::MAX);
        let order_b = configs.iter().find(|c| c.uid == b.uid).map(|c| c.sort_order).unwrap_or(i32::MAX);
        order_a.cmp(&order_b).then(a.uid.cmp(&b.uid))
    });

    result
}

/// Fix up migrated entries whose api_name is "unknown" — once we detect the real
/// monitor, we can fill in the correct uid and api_name.
fn reconcile_migrated_configs(
    monitors: &[Monitor],
    configs: &mut Vec<crate::config::MonitorMetadata>,
) -> bool {
    let mut changed = false;
    for monitor in monitors {
        if configs.iter().any(|c| c.uid == monitor.uid) {
            continue;
        }
        if let Some(meta) = configs.iter_mut().find(|c| c.api_id == monitor.id && c.api_name == "unknown") {
            meta.uid = monitor.uid.clone();
            meta.api_name = monitor.original_name.clone();
            changed = true;
        }
    }
    changed
}

/// Ensure every detected monitor has a metadata entry in preferences.
/// New monitors get an entry with empty label (will display api_name).
fn ensure_metadata_for_monitors(
    monitors: &[Monitor],
    configs: &mut Vec<crate::config::MonitorMetadata>,
) -> bool {
    let mut changed = false;
    let next_order = configs.iter().map(|c| c.sort_order).max().unwrap_or(-1) + 1;

    for (i, monitor) in monitors.iter().enumerate() {
        if !configs.iter().any(|c| c.uid == monitor.uid) {
            configs.push(crate::config::MonitorMetadata {
                uid: monitor.uid.clone(),
                api_id: monitor.id.clone(),
                api_name: monitor.original_name.clone(),
                label: String::new(),
                sort_order: next_order + i as i32,
                hidden: false,
            });
            changed = true;
        }
    }
    changed
}

// ===========================================================================
// Tauri commands
// ===========================================================================

/// Returns all connected monitors with saved metadata applied.
/// Reconciles migrated configs and ensures new monitors get metadata entries.
#[tauri::command]
pub async fn get_monitors(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<Monitor>, String> {
    let monitors = detect_monitors().await;
    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;

    let mut dirty = reconcile_migrated_configs(&monitors, &mut prefs.monitor_configs);
    dirty |= ensure_metadata_for_monitors(&monitors, &mut prefs.monitor_configs);
    if dirty {
        crate::config::save_preferences_to_disk(&prefs);
    }

    Ok(merge_with_configs(monitors, &prefs.monitor_configs))
}

/// Sets brightness for a single monitor, enforcing the minimum brightness floor.
#[tauri::command]
pub async fn set_brightness(
    state: tauri::State<'_, crate::AppState>,
    monitor_id: String,
    value: u32,
) -> Result<(), String> {
    let min = state.preferences.lock().map_err(|e| e.to_string())?.effective_min_brightness();
    set_monitor_brightness(&monitor_id, value, min).await
}

/// Sets brightness for all monitors, enforcing the minimum brightness floor.
#[tauri::command]
pub async fn set_all_brightness(
    state: tauri::State<'_, crate::AppState>,
    value: u32,
) -> Result<(), String> {
    let min = state.preferences.lock().map_err(|e| e.to_string())?.effective_min_brightness();
    set_all_monitors_brightness(value, min).await
}

/// Sets contrast for a single monitor (0-100, DDC-only).
#[tauri::command]
pub async fn set_contrast(
    monitor_id: String,
    value: u32,
) -> Result<(), String> {
    set_monitor_contrast(&monitor_id, value).await
}

/// Sets contrast for all monitors (0-100, DDC-only).
#[tauri::command]
pub async fn set_all_contrast(
    value: u32,
) -> Result<(), String> {
    set_all_monitors_contrast(value).await
}

/// Updates a monitor's custom label in preferences. Creates a new metadata entry
/// if the monitor isn't tracked yet.
#[tauri::command]
pub fn rename_monitor(
    state: tauri::State<'_, crate::AppState>,
    uid: String,
    name: String,
) -> Result<(), String> {
    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    if let Some(meta) = prefs.monitor_configs.iter_mut().find(|m| m.uid == uid) {
        meta.label = name;
    } else {
        let parts: Vec<&str> = uid.splitn(2, "::").collect();
        let (api_id, api_name) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (uid.clone(), String::new())
        };
        prefs.monitor_configs.push(crate::config::MonitorMetadata {
            uid: uid.clone(),
            api_id,
            api_name,
            label: name,
            sort_order: 0,
            hidden: false,
        });
    }
    crate::config::save_preferences_to_disk(&prefs);
    Ok(())
}

/// Persists the user's custom monitor sort order to preferences.
#[tauri::command]
pub fn save_monitor_order(
    state: tauri::State<'_, crate::AppState>,
    orders: Vec<(String, i32)>,
) -> Result<(), String> {
    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    for (uid, sort_order) in orders {
        if let Some(meta) = prefs.monitor_configs.iter_mut().find(|m| m.uid == uid) {
            meta.sort_order = sort_order;
        } else {
            let parts: Vec<&str> = uid.splitn(2, "::").collect();
            let (api_id, api_name) = if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (uid.clone(), String::new())
            };
            prefs.monitor_configs.push(crate::config::MonitorMetadata {
                uid,
                api_id,
                api_name,
                label: String::new(),
                sort_order,
                hidden: false,
            });
        }
    }
    crate::config::save_preferences_to_disk(&prefs);
    Ok(())
}

/// Toggles a monitor's hidden state in preferences (hidden monitors are excluded from the main UI).
#[tauri::command]
pub fn set_monitor_visibility(
    state: tauri::State<'_, crate::AppState>,
    uid: String,
    hidden: bool,
) -> Result<(), String> {
    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    if let Some(meta) = prefs.monitor_configs.iter_mut().find(|m| m.uid == uid) {
        meta.hidden = hidden;
    }
    crate::config::save_preferences_to_disk(&prefs);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_monitor(id: &str, name: &str, is_built_in: bool) -> Monitor {
        Monitor {
            id: id.into(),
            uid: format!("{}::{}", id, name),
            name: name.into(),
            original_name: name.into(),
            brightness: 50,
            contrast: None,
            supports_brightness: true,
            is_built_in,
            hidden: false,
        }
    }

    fn make_meta(uid: &str, label: &str, sort_order: i32) -> crate::config::MonitorMetadata {
        let parts: Vec<&str> = uid.splitn(2, "::").collect();
        crate::config::MonitorMetadata {
            uid: uid.into(),
            api_id: parts.first().unwrap_or(&"").to_string(),
            api_name: parts.get(1).unwrap_or(&"").to_string(),
            label: label.into(),
            sort_order,
            hidden: false,
        }
    }

    #[test]
    fn test_monitor_serialization_camel_case() {
        let monitor = make_monitor("builtin", "Built-in", true);
        let json = serde_json::to_string(&monitor).unwrap();
        assert!(json.contains("\"supportsBrightness\""));
        assert!(json.contains("\"isBuiltIn\""));
        assert!(json.contains("\"uid\""));
        assert!(!json.contains("supports_brightness"));
        assert!(!json.contains("is_built_in"));
    }

    #[test]
    fn test_monitor_deserialization() {
        let json = r#"{
            "id": "1",
            "uid": "1::Dell U2723QE",
            "name": "Dell U2723QE",
            "originalName": "Dell U2723QE",
            "brightness": 80,
            "contrast": 60,
            "supportsBrightness": true,
            "isBuiltIn": false
        }"#;
        let monitor: Monitor = serde_json::from_str(json).unwrap();
        assert_eq!(monitor.id, "1");
        assert_eq!(monitor.uid, "1::Dell U2723QE");
        assert_eq!(monitor.name, "Dell U2723QE");
        assert_eq!(monitor.brightness, 80);
        assert!(!monitor.is_built_in);
    }

    #[test]
    fn test_monitor_roundtrip_serialization() {
        let original = make_monitor("2", "LG 27UK850", false);
        let json = serde_json::to_string(&original).unwrap();
        let restored: Monitor = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, original.id);
        assert_eq!(restored.uid, original.uid);
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.brightness, original.brightness);
        assert_eq!(restored.supports_brightness, original.supports_brightness);
        assert_eq!(restored.is_built_in, original.is_built_in);
    }

    #[test]
    fn test_merge_with_configs_renames_monitor() {
        let monitors = vec![make_monitor("1", "External Display 1", false)];
        let configs = vec![make_meta("1::External Display 1", "My Dell", 0)];
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "My Dell");
    }

    #[test]
    fn test_merge_with_configs_sorts_by_sort_order() {
        let monitors = vec![
            make_monitor("1", "Monitor A", false),
            make_monitor("2", "Monitor B", false),
            make_monitor("builtin", "Built-in", true),
        ];
        let configs = vec![
            make_meta("2::Monitor B", "Monitor B", 1),
            make_meta("builtin::Built-in", "Built-in", 0),
        ];
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].uid, "builtin::Built-in");
        assert_eq!(result[1].uid, "2::Monitor B");
        assert_eq!(result[2].uid, "1::Monitor A");
    }

    #[test]
    fn test_merge_with_configs_empty_label_keeps_original() {
        let monitors = vec![make_monitor("1", "Original Name", false)];
        let configs = vec![make_meta("1::Original Name", "", 0)];
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result[0].name, "Original Name");
    }

    #[test]
    fn test_merge_with_configs_no_configs() {
        let monitors = vec![
            make_monitor("builtin", "Built-in", true),
            make_monitor("1", "External", false),
        ];
        let configs: Vec<crate::config::MonitorMetadata> = Vec::new();
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_merge_with_configs_preserves_original_name() {
        let monitors = vec![make_monitor("1", "Dell U2723QE", false)];
        let configs = vec![make_meta("1::Dell U2723QE", "My Custom Label", 0)];
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result[0].name, "My Custom Label");
        assert_eq!(result[0].original_name, "Dell U2723QE"); // must NOT change
    }

    #[test]
    fn test_merge_with_configs_same_sort_order_tiebreaks_by_uid() {
        let monitors = vec![
            make_monitor("2", "Monitor B", false),
            make_monitor("1", "Monitor A", false),
        ];
        let configs = vec![
            make_meta("2::Monitor B", "", 0),
            make_meta("1::Monitor A", "", 0), // same sort_order
        ];
        let result = merge_with_configs(monitors, &configs);
        // Tiebreaker is uid ascending: "1::Monitor A" < "2::Monitor B"
        assert_eq!(result[0].uid, "1::Monitor A");
        assert_eq!(result[1].uid, "2::Monitor B");
    }

    #[test]
    fn test_merge_with_configs_unplugged_monitors_not_in_result() {
        // Config has entries for 3 monitors, but only 1 is currently connected
        let monitors = vec![make_monitor("1", "Dell", false)];
        let configs = vec![
            make_meta("1::Dell", "My Dell", 0),
            make_meta("2::LG", "Office Left", 1),       // unplugged
            make_meta("builtin::Built-in", "MacBook", 2), // unplugged
        ];
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 1); // only the connected monitor
        assert_eq!(result[0].name, "My Dell");
    }

    #[test]
    fn test_dj_display_into_monitor_preserves_original_name() {
        let dj = DjDisplay {
            id: "1".into(),
            name: "Dell U2723QE".into(),
            display_type: "external".into(),
            brightness: Some(70),
            contrast: Some(60),
        };
        let m = dj.into_monitor();
        assert_eq!(m.name, "Dell U2723QE");
        assert_eq!(m.original_name, "Dell U2723QE");
        assert_eq!(m.name, m.original_name); // name and original_name start identical
    }

    #[test]
    fn test_dj_display_into_monitor_builtin() {
        let dj = DjDisplay {
            id: "builtin".into(),
            name: "Built-in Display".into(),
            display_type: "builtin".into(),
            brightness: Some(80),
            contrast: None,
        };
        let m = dj.into_monitor();
        assert!(m.is_built_in);
        assert_eq!(m.brightness, 80);
        assert_eq!(m.uid, "builtin::Built-in Display");
    }

    #[test]
    fn test_dj_display_into_monitor_external_ddc() {
        let dj = DjDisplay {
            id: "1".into(),
            name: "Dell U2723QE".into(),
            display_type: "external".into(),
            brightness: Some(50),
            contrast: Some(75),
        };
        let m = dj.into_monitor();
        assert!(!m.is_built_in);
        assert_eq!(m.brightness, 50);
        assert_eq!(m.uid, "1::Dell U2723QE");
    }

    #[test]
    fn test_dj_display_into_monitor_null_brightness() {
        let dj = DjDisplay {
            id: "2".into(),
            name: "Unknown".into(),
            display_type: "external".into(),
            brightness: None,
            contrast: None,
        };
        let m = dj.into_monitor();
        assert_eq!(m.brightness, 50);
        assert_eq!(m.uid, "2::Unknown");
    }

    #[test]
    fn test_reconcile_migrated_configs() {
        let monitors = vec![make_monitor("1", "Dell U2723QE", false)];
        let mut configs = vec![crate::config::MonitorMetadata {
            uid: "1::unknown".into(),
            api_id: "1".into(),
            api_name: "unknown".into(),
            label: "My Dell".into(),
            sort_order: 0,
            hidden: false,
        }];
        let changed = reconcile_migrated_configs(&monitors, &mut configs);
        assert!(changed);
        assert_eq!(configs[0].uid, "1::Dell U2723QE");
        assert_eq!(configs[0].api_name, "Dell U2723QE");
        assert_eq!(configs[0].label, "My Dell"); // label preserved
    }

    #[test]
    fn test_ensure_metadata_for_monitors() {
        let monitors = vec![
            make_monitor("builtin", "Built-in", true),
            make_monitor("1", "Dell", false),
        ];
        let mut configs: Vec<crate::config::MonitorMetadata> = Vec::new();
        let changed = ensure_metadata_for_monitors(&monitors, &mut configs);
        assert!(changed);
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].uid, "builtin::Built-in");
        assert_eq!(configs[1].uid, "1::Dell");
        assert_eq!(configs[0].label, ""); // default empty label

        // Running again should not add duplicates
        let changed2 = ensure_metadata_for_monitors(&monitors, &mut configs);
        assert!(!changed2);
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_ensure_metadata_sort_order_continues_from_existing() {
        let monitors = vec![
            make_monitor("builtin", "Built-in", true),
            make_monitor("3", "New Monitor", false),
        ];
        // Pre-existing configs with sort orders 0, 5, 10
        let mut configs = vec![
            make_meta("builtin::Built-in", "MacBook", 0),
            make_meta("1::Dell", "Left", 5),   // unplugged but persisted
            make_meta("2::LG", "Right", 10),    // unplugged but persisted
        ];
        let changed = ensure_metadata_for_monitors(&monitors, &mut configs);
        assert!(changed); // "3::New Monitor" is new
        assert_eq!(configs.len(), 4);
        // New monitor should get sort_order = max(0,5,10) + 1 + index_in_monitors_list
        // "3::New Monitor" is at index 1 in monitors vec, so sort_order = 11 + 1 = 12
        let new = configs.iter().find(|c| c.uid == "3::New Monitor").unwrap();
        assert_eq!(new.sort_order, 12);
    }

    #[test]
    fn test_reconcile_migrated_configs_multiple_monitors() {
        let monitors = vec![
            make_monitor("1", "Dell U2723QE", false),
            make_monitor("2", "LG 27UK850", false),
            make_monitor("builtin", "Built-in Display", true),
        ];
        let mut configs = vec![
            // Two migrated entries with "unknown"
            crate::config::MonitorMetadata {
                uid: "1::unknown".into(),
                api_id: "1".into(),
                api_name: "unknown".into(),
                label: "Left Monitor".into(),
                sort_order: 0,
                hidden: false,
            },
            crate::config::MonitorMetadata {
                uid: "2::unknown".into(),
                api_id: "2".into(),
                api_name: "unknown".into(),
                label: "Right Monitor".into(),
                sort_order: 1,
                hidden: false,
            },
            // One already-known entry (not migrated)
            crate::config::MonitorMetadata {
                uid: "builtin::Built-in Display".into(),
                api_id: "builtin".into(),
                api_name: "Built-in Display".into(),
                label: "MacBook".into(),
                sort_order: 2,
                hidden: false,
            },
        ];
        let changed = reconcile_migrated_configs(&monitors, &mut configs);
        assert!(changed);
        // Migrated entries should be reconciled
        assert_eq!(configs[0].uid, "1::Dell U2723QE");
        assert_eq!(configs[0].api_name, "Dell U2723QE");
        assert_eq!(configs[0].label, "Left Monitor"); // preserved
        assert_eq!(configs[1].uid, "2::LG 27UK850");
        assert_eq!(configs[1].api_name, "LG 27UK850");
        assert_eq!(configs[1].label, "Right Monitor"); // preserved
        // Already-known entry unchanged
        assert_eq!(configs[2].uid, "builtin::Built-in Display");
        assert_eq!(configs[2].label, "MacBook");
    }

    #[test]
    fn test_reconcile_skips_when_uid_already_matches() {
        let monitors = vec![make_monitor("1", "Dell", false)];
        let mut configs = vec![make_meta("1::Dell", "My Dell", 0)];
        let changed = reconcile_migrated_configs(&monitors, &mut configs);
        assert!(!changed); // uid already matches, nothing to do
    }
}
