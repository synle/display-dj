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
        .menu_on_left_click(false)
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
    use tauri::LogicalPosition;

    let window_size = window.outer_size()?;
    let scale = window.scale_factor()?;
    let win_w = window_size.width as f64 / scale;
    let win_h = window_size.height as f64 / scale;

    let (tray_x, tray_y) = match tray_rect.position {
        tauri::Position::Physical(p) => (p.x as f64 / scale, p.y as f64 / scale),
        tauri::Position::Logical(p) => (p.x, p.y),
    };

    let (tray_w, tray_h) = match tray_rect.size {
        tauri::Size::Physical(s) => (s.width as f64 / scale, s.height as f64 / scale),
        tauri::Size::Logical(s) => (s.width, s.height),
    };

    let x = tray_x + (tray_w / 2.0) - (win_w / 2.0);

    let y = if tray_y < 100.0 {
        tray_y + tray_h
    } else {
        tray_y - win_h
    };

    window.set_position(LogicalPosition::new(x, y))?;
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
        ["command", "changeContrast", _value] => {
            let _ = app.emit("monitors-changed", ());
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
