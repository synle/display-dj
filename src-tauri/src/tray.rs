use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::config::{CommandValue, KeyBinding};

fn base_url() -> String {
    let port = crate::server_port();
    format!("http://127.0.0.1:{}", port)
}

/// Fire an HTTP GET in a background thread (non-blocking, fire-and-forget).
fn http_get(url: String) {
    std::thread::spawn(move || {
        let _ = reqwest::blocking::get(&url);
    });
}

/// Fire an HTTP GET in a background thread and emit an event when done.
fn http_get_then_emit(url: String, app: AppHandle, event: &'static str) {
    std::thread::spawn(move || {
        let _ = reqwest::blocking::get(&url);
        let _ = app.emit(event, ());
    });
}

pub fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let port = crate::server_port();
    let bridge_label = format!("Bridge: 127.0.0.1:{}", port);
    let bridge = MenuItemBuilder::with_id("bridge", &bridge_label)
        .enabled(false)
        .build(app)?;
    let dark_mode = MenuItemBuilder::with_id("dark_mode", "Dark Mode").build(app)?;
    let light_mode = MenuItemBuilder::with_id("light_mode", "Light Mode").build(app)?;
    let open_configs =
        MenuItemBuilder::with_id("open_configs", "Open Monitor Configs").build(app)?;
    let open_prefs =
        MenuItemBuilder::with_id("open_prefs", "Open App Preferences").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&bridge)
        .separator()
        .items(&[&dark_mode, &light_mode])
        .separator()
        .items(&[&open_configs, &open_prefs])
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Display DJ")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "dark_mode" => {
                let url = format!("{}/dark", base_url());
                http_get(url);
            }
            "light_mode" => {
                let url = format!("{}/light", base_url());
                http_get(url);
            }
            "open_configs" => {
                let _ = crate::config::open_config_file();
            }
            "open_prefs" => {
                let _ = crate::config::open_preferences_file();
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    if visible {
                        let _ = window.hide();
                    } else {
                        if let Ok(Some(tray_rect)) = tray.rect() {
                            let _ = position_window_near_tray(&window, tray_rect);
                        }
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn position_window_near_tray(
    window: &tauri::WebviewWindow,
    tray_rect: tauri::Rect,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::PhysicalPosition;

    let current_scale = window.scale_factor()?;

    // Get tray position/size in physical pixels
    let (tray_x, tray_y) = match tray_rect.position {
        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
        tauri::Position::Logical(p) => (p.x * current_scale, p.y * current_scale),
    };
    let (tray_w, tray_h) = match tray_rect.size {
        tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
        tauri::Size::Logical(s) => (s.width * current_scale, s.height * current_scale),
    };

    // Find the monitor containing the tray icon (physical coords)
    let monitors = window.available_monitors()?;
    let tray_cx = tray_x + tray_w / 2.0;
    let tray_cy = tray_y + tray_h / 2.0;
    let target = monitors.iter().find(|m| {
        let pos = m.position();
        let size = m.size();
        tray_cx >= pos.x as f64
            && tray_cx < pos.x as f64 + size.width as f64
            && tray_cy >= pos.y as f64
            && tray_cy < pos.y as f64 + size.height as f64
    });

    // Use the target monitor's scale factor for window size calculation
    let target_scale = target
        .map(|m| m.scale_factor())
        .unwrap_or(current_scale);
    let logical_w = window.outer_size()?.width as f64 / current_scale;
    let logical_h = window.outer_size()?.height as f64 / current_scale;
    let win_w = logical_w * target_scale;
    let win_h = logical_h * target_scale;

    // Center window horizontally under tray icon
    let mut x = tray_x + (tray_w / 2.0) - (win_w / 2.0);

    // Position below tray if near the top of its monitor, above otherwise
    let mon_top = target.map(|m| m.position().y as f64).unwrap_or(0.0);
    let mut y = if tray_y - mon_top < 100.0 * target_scale {
        tray_y + tray_h
    } else {
        tray_y - win_h
    };

    // Clamp to the target monitor's bounds
    if let Some(m) = target {
        let mx = m.position().x as f64;
        let my = m.position().y as f64;
        let mw = m.size().width as f64;
        let mh = m.size().height as f64;

        if x + win_w > mx + mw {
            x = mx + mw - win_w;
        }
        if x < mx {
            x = mx;
        }
        if y + win_h > my + mh {
            y = my + mh - win_h;
        }
        if y < my {
            y = my;
        }
    }

    window.set_position(PhysicalPosition::new(x as i32, y as i32))?;
    Ok(())
}

pub fn register_shortcuts(app: &AppHandle, key_bindings: &[KeyBinding]) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let _ = app.global_shortcut().unregister_all();

    for binding in key_bindings {
        let commands: Vec<String> = match &binding.command {
            CommandValue::Single(cmd) => vec![cmd.clone()],
            CommandValue::Multiple(cmds) => cmds.clone(),
        };

        let handle = app.clone();
        let key = binding.key.clone();

        if let Ok(shortcut) = key.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = app.global_shortcut().on_shortcut(
                shortcut,
                move |_app, _shortcut, _event| {
                    for cmd in &commands {
                        execute_command(&handle, cmd);
                    }
                },
            );
        } else {
            log::warn!("Failed to parse shortcut: {}", key);
        }
    }
}

fn execute_command(app: &AppHandle, command: &str) {
    let parts: Vec<&str> = command.split('/').collect();
    let base = base_url();
    match parts.as_slice() {
        ["command", "changeBrightness", value] => {
            if let Ok(val) = value.parse::<u32>() {
                let url = format!("{}/set_all/{}", base, val.min(100));
                http_get_then_emit(url, app.clone(), "monitors-changed");
            }
        }
        ["command", "changeDarkMode", mode] => {
            let url = match *mode {
                "toggle" => {
                    // For toggle, we need to read current state first — fire and forget
                    let base_clone = base.clone();
                    let app_clone = app.clone();
                    std::thread::spawn(move || {
                        if let Ok(resp) = reqwest::blocking::get(format!("{}/theme", base_clone)) {
                            if let Ok(text) = resp.text() {
                                let route = if text.contains("dark") { "light" } else { "dark" };
                                let _ = reqwest::blocking::get(format!("{}/{}", base_clone, route));
                            }
                        }
                        let _ = app_clone.emit("dark-mode-changed", ());
                    });
                    return;
                }
                "dark" => format!("{}/dark", base),
                "light" => format!("{}/light", base),
                _ => return,
            };
            http_get_then_emit(url, app.clone(), "dark-mode-changed");
        }
        ["command", "changeVolume", value] => {
            if let Ok(val) = value.parse::<u32>() {
                let url = format!("{}/set_volume/{}", base, val.min(100));
                http_get_then_emit(url, app.clone(), "volume-changed");
            }
        }
        _ => {
            log::warn!("Unknown command: {}", command);
        }
    }
}
