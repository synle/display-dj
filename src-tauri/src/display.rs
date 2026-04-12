use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub brightness: u32,
    pub contrast: u32,
    pub supports_brightness: bool,
    pub supports_contrast: bool,
    pub is_built_in: bool,
}

/// Response from the display-dj HTTP server's /get_all and /list endpoints.
#[derive(Deserialize, Debug)]
struct DjDisplay {
    id: String,
    name: String,
    display_type: String,
    brightness: Option<u32>,
    contrast: Option<u32>,
    ddc_supported: bool,
}

impl DjDisplay {
    fn into_monitor(self) -> Monitor {
        let is_built_in = self.display_type == "builtin";
        Monitor {
            id: self.id,
            name: self.name,
            brightness: self.brightness.unwrap_or(50),
            contrast: self.contrast.unwrap_or(50),
            supports_brightness: true,
            supports_contrast: !is_built_in && self.ddc_supported && self.contrast.is_some(),
            is_built_in,
        }
    }
}

fn base_url() -> String {
    let port = crate::server_port();
    format!("http://127.0.0.1:{}", port)
}

async fn detect_monitors() -> Vec<Monitor> {
    let url = format!("{}/get_all", base_url());
    let displays: Vec<DjDisplay> = match reqwest::get(&url).await {
        Ok(resp) => resp.json().await.unwrap_or_default(),
        Err(e) => {
            log::warn!("display-dj server request failed: {}", e);
            Vec::new()
        }
    };
    displays.into_iter().map(|d| d.into_monitor()).collect()
}

async fn set_monitor_brightness(monitor_id: &str, value: u32) -> Result<(), String> {
    let url = format!("{}/set_one/{}/{}", base_url(), monitor_id, value.clamp(10, 100));
    reqwest::get(&url).await
        .map_err(|e| format!("Failed to set brightness: {}", e))?;
    Ok(())
}

async fn set_monitor_contrast(monitor_id: &str, value: u32) -> Result<(), String> {
    let _ = (monitor_id, value);
    Err("Contrast control via display-dj server is not yet supported".into())
}

async fn set_all_monitors_brightness(value: u32) -> Result<(), String> {
    let url = format!("{}/set_all/{}", base_url(), value.clamp(10, 100));
    reqwest::get(&url).await
        .map_err(|e| format!("Failed to set all brightness: {}", e))?;
    Ok(())
}

async fn set_all_monitors_contrast(value: u32) -> Result<(), String> {
    let _ = value;
    Err("Contrast control via display-dj server is not yet supported".into())
}

// ===========================================================================
// Common helpers
// ===========================================================================

fn merge_with_configs(
    monitors: Vec<Monitor>,
    configs: &crate::config::MonitorConfigs,
) -> Vec<Monitor> {
    let mut result: Vec<Monitor> = Vec::new();

    for mut monitor in monitors {
        if let Some(config) = configs.get(&monitor.id) {
            if !config.name.is_empty() {
                monitor.name = config.name.clone();
            }
            if config.disabled {
                continue;
            }
        }
        result.push(monitor);
    }

    result.sort_by(|a, b| {
        let order_a = configs.get(&a.id).map(|c| c.sort_order).unwrap_or(i32::MAX);
        let order_b = configs.get(&b.id).map(|c| c.sort_order).unwrap_or(i32::MAX);
        order_a.cmp(&order_b).then(a.id.cmp(&b.id))
    });

    result
}

// ===========================================================================
// Tauri commands
// ===========================================================================

#[tauri::command]
pub async fn get_monitors(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<Monitor>, String> {
    let monitors = detect_monitors().await;
    let configs = state.monitor_configs.lock().map_err(|e| e.to_string())?;
    Ok(merge_with_configs(monitors, &configs))
}

#[tauri::command]
pub async fn set_brightness(monitor_id: String, value: u32) -> Result<(), String> {
    set_monitor_brightness(&monitor_id, value).await
}

#[tauri::command]
pub async fn set_contrast(monitor_id: String, value: u32) -> Result<(), String> {
    set_monitor_contrast(&monitor_id, value).await
}

#[tauri::command]
pub async fn set_all_brightness(value: u32) -> Result<(), String> {
    set_all_monitors_brightness(value).await
}

#[tauri::command]
pub async fn set_all_contrast(value: u32) -> Result<(), String> {
    set_all_monitors_contrast(value).await
}

#[tauri::command]
pub fn rename_monitor(
    state: tauri::State<'_, crate::AppState>,
    monitor_id: String,
    name: String,
) -> Result<(), String> {
    let mut configs = state.monitor_configs.lock().map_err(|e| e.to_string())?;
    let config = configs
        .entry(monitor_id.clone())
        .or_insert_with(|| crate::config::MonitorConfig {
            id: monitor_id,
            name: String::new(),
            sort_order: 0,
            disabled: false,
        });
    config.name = name;
    crate::config::save_monitor_configs_to_disk(&configs);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_monitor(id: &str, name: &str, is_built_in: bool) -> Monitor {
        Monitor {
            id: id.into(),
            name: name.into(),
            brightness: 50,
            contrast: 50,
            supports_brightness: true,
            supports_contrast: !is_built_in,
            is_built_in,
        }
    }

    #[test]
    fn test_monitor_serialization_camel_case() {
        let monitor = make_monitor("builtin-0", "Built-in", true);
        let json = serde_json::to_string(&monitor).unwrap();
        assert!(json.contains("\"supportsBrightness\""));
        assert!(json.contains("\"supportsContrast\""));
        assert!(json.contains("\"isBuiltIn\""));
        assert!(!json.contains("supports_brightness"));
        assert!(!json.contains("is_built_in"));
    }

    #[test]
    fn test_monitor_deserialization() {
        let json = r#"{
            "id": "external-1",
            "name": "Dell U2723QE",
            "brightness": 80,
            "contrast": 50,
            "supportsBrightness": true,
            "supportsContrast": true,
            "isBuiltIn": false
        }"#;
        let monitor: Monitor = serde_json::from_str(json).unwrap();
        assert_eq!(monitor.id, "external-1");
        assert_eq!(monitor.name, "Dell U2723QE");
        assert_eq!(monitor.brightness, 80);
        assert!(monitor.supports_contrast);
        assert!(!monitor.is_built_in);
    }

    #[test]
    fn test_monitor_roundtrip_serialization() {
        let original = make_monitor("external-2", "LG 27UK850", false);
        let json = serde_json::to_string(&original).unwrap();
        let restored: Monitor = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, original.id);
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.brightness, original.brightness);
        assert_eq!(restored.contrast, original.contrast);
        assert_eq!(restored.supports_brightness, original.supports_brightness);
        assert_eq!(restored.supports_contrast, original.supports_contrast);
        assert_eq!(restored.is_built_in, original.is_built_in);
    }

    #[test]
    fn test_merge_with_configs_renames_monitor() {
        let monitors = vec![make_monitor("external-1", "External Display 1", false)];
        let mut configs: crate::config::MonitorConfigs = HashMap::new();
        configs.insert(
            "external-1".into(),
            crate::config::MonitorConfig {
                id: "external-1".into(),
                name: "My Dell".into(),
                sort_order: 0,
                disabled: false,
            },
        );
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "My Dell");
    }

    #[test]
    fn test_merge_with_configs_filters_disabled() {
        let monitors = vec![
            make_monitor("external-1", "Monitor 1", false),
            make_monitor("external-2", "Monitor 2", false),
        ];
        let mut configs: crate::config::MonitorConfigs = HashMap::new();
        configs.insert(
            "external-2".into(),
            crate::config::MonitorConfig {
                id: "external-2".into(),
                name: "Monitor 2".into(),
                sort_order: 0,
                disabled: true,
            },
        );
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "external-1");
    }

    #[test]
    fn test_merge_with_configs_sorts_by_sort_order() {
        let monitors = vec![
            make_monitor("external-1", "Monitor A", false),
            make_monitor("external-2", "Monitor B", false),
            make_monitor("builtin-0", "Built-in", true),
        ];
        let mut configs: crate::config::MonitorConfigs = HashMap::new();
        configs.insert(
            "external-2".into(),
            crate::config::MonitorConfig {
                id: "external-2".into(),
                name: "Monitor B".into(),
                sort_order: 1,
                disabled: false,
            },
        );
        configs.insert(
            "builtin-0".into(),
            crate::config::MonitorConfig {
                id: "builtin-0".into(),
                name: "Built-in".into(),
                sort_order: 0,
                disabled: false,
            },
        );
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, "builtin-0");
        assert_eq!(result[1].id, "external-2");
        assert_eq!(result[2].id, "external-1");
    }

    #[test]
    fn test_merge_with_configs_empty_name_keeps_original() {
        let monitors = vec![make_monitor("external-1", "Original Name", false)];
        let mut configs: crate::config::MonitorConfigs = HashMap::new();
        configs.insert(
            "external-1".into(),
            crate::config::MonitorConfig {
                id: "external-1".into(),
                name: "".into(),
                sort_order: 0,
                disabled: false,
            },
        );
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result[0].name, "Original Name");
    }

    #[test]
    fn test_merge_with_configs_no_configs() {
        let monitors = vec![
            make_monitor("builtin-0", "Built-in", true),
            make_monitor("external-1", "External", false),
        ];
        let configs: crate::config::MonitorConfigs = HashMap::new();
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_dj_display_into_monitor_builtin() {
        let dj = DjDisplay {
            id: "builtin".into(),
            name: "Built-in Display".into(),
            display_type: "builtin".into(),
            brightness: Some(80),
            contrast: None,
            ddc_supported: false,
        };
        let m = dj.into_monitor();
        assert!(m.is_built_in);
        assert_eq!(m.brightness, 80);
        assert!(!m.supports_contrast);
    }

    #[test]
    fn test_dj_display_into_monitor_external_ddc() {
        let dj = DjDisplay {
            id: "1".into(),
            name: "Dell U2723QE".into(),
            display_type: "external".into(),
            brightness: Some(50),
            contrast: Some(70),
            ddc_supported: true,
        };
        let m = dj.into_monitor();
        assert!(!m.is_built_in);
        assert_eq!(m.brightness, 50);
        assert_eq!(m.contrast, 70);
        assert!(m.supports_contrast);
    }

    #[test]
    fn test_dj_display_into_monitor_null_brightness() {
        let dj = DjDisplay {
            id: "2".into(),
            name: "Unknown".into(),
            display_type: "external".into(),
            brightness: None,
            contrast: None,
            ddc_supported: false,
        };
        let m = dj.into_monitor();
        assert_eq!(m.brightness, 50);
        assert_eq!(m.contrast, 50);
        assert!(!m.supports_contrast);
    }
}
