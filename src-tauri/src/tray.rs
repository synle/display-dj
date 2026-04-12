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
    let reset_defaults =
        MenuItemBuilder::with_id("reset_defaults", "Reset to Default").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&bridge)
        .separator()
        .items(&[&dark_mode, &light_mode])
        .separator()
        .items(&[&open_configs, &open_prefs])
        .separator()
        .item(&reset_defaults)
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
            "reset_defaults" => {
                crate::config::reset_to_defaults();
                // Reload in-memory state
                if let Some(state) = app.try_state::<crate::AppState>() {
                    if let Ok(mut prefs) = state.preferences.lock() {
                        *prefs = crate::config::load_preferences();
                    }
                    if let Ok(mut configs) = state.monitor_configs.lock() {
                        *configs = crate::config::load_monitor_configs();
                    }
                }
                // Re-register shortcuts with default keybindings
                let prefs = crate::config::Preferences::default();
                register_shortcuts(app, &prefs.key_bindings);
                // Notify frontend to refresh
                let _ = app.emit("monitors-changed", ());
                let _ = app.emit("dark-mode-changed", ());
                let _ = app.emit("volume-changed", ());
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

    // --- Phase 1: Find the target monitor using a rough scale estimate ---
    let rough_scale = window.scale_factor().unwrap_or(2.0);

    let tray_x_rough = match tray_rect.position {
        tauri::Position::Physical(p) => p.x as f64,
        tauri::Position::Logical(p) => p.x * rough_scale,
    };
    let tray_y_rough = match tray_rect.position {
        tauri::Position::Physical(p) => p.y as f64,
        tauri::Position::Logical(p) => p.y * rough_scale,
    };
    let tray_w_rough = match tray_rect.size {
        tauri::Size::Physical(s) => s.width as f64,
        tauri::Size::Logical(s) => s.width * rough_scale,
    };
    let tray_h_rough = match tray_rect.size {
        tauri::Size::Physical(s) => s.height as f64,
        tauri::Size::Logical(s) => s.height * rough_scale,
    };

    let tray_cx = tray_x_rough + tray_w_rough / 2.0;
    let tray_cy = tray_y_rough + tray_h_rough / 2.0;

    let monitors = window.available_monitors()?;
    let target = monitors.iter().find(|m| {
        let pos = m.position();
        let size = m.size();
        tray_cx >= pos.x as f64
            && tray_cx < pos.x as f64 + size.width as f64
            && tray_cy >= pos.y as f64
            && tray_cy < pos.y as f64 + size.height as f64
    });

    // --- Phase 2: Move window to target monitor so scale_factor() is correct ---
    // The window is still hidden at this point, so there's no visual flicker.
    // This is necessary because set_position(PhysicalPosition) internally
    // divides by window.scale_factor() to get platform coordinates. If the
    // window is on a different monitor (different DPI), the division is wrong.
    if let Some(m) = target {
        window.set_position(PhysicalPosition::new(
            m.position().x + 10,
            m.position().y + 10,
        ))?;
    }

    // --- Phase 3: Now position precisely (scale_factor matches target monitor) ---
    let scale = window.scale_factor()?;

    let (tray_x, tray_y) = match tray_rect.position {
        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
        tauri::Position::Logical(p) => (p.x * scale, p.y * scale),
    };
    let (tray_w, tray_h) = match tray_rect.size {
        tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
        tauri::Size::Logical(s) => (s.width * scale, s.height * scale),
    };

    let win_w = 360.0 * scale;
    let win_h = window.outer_size()?.height as f64;

    // Center window horizontally under tray icon
    let mut x = tray_x + (tray_w / 2.0) - (win_w / 2.0);

    // Position below tray if in top half of monitor, above otherwise
    let (mon_x, mon_y, mon_w, mon_h) = if let Some(m) = target {
        (
            m.position().x as f64,
            m.position().y as f64,
            m.size().width as f64,
            m.size().height as f64,
        )
    } else {
        (0.0, 0.0, 1920.0, 1080.0)
    };

    let tray_in_top_half = (tray_y - mon_y) < mon_h / 2.0;
    let gap = 4.0 * scale;
    let mut y = if tray_in_top_half {
        tray_y + tray_h + gap
    } else {
        tray_y - win_h - gap
    };

    // Clamp to monitor bounds
    x = x.max(mon_x).min(mon_x + mon_w - win_w);
    y = y.max(mon_y).min(mon_y + mon_h - win_h);

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
                let min = app
                    .state::<crate::AppState>()
                    .preferences
                    .lock()
                    .map(|p| p.effective_min_brightness())
                    .unwrap_or(crate::config::ABSOLUTE_MIN_BRIGHTNESS);
                let url = format!("{}/set_all/{}", base, val.clamp(min, 100));
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
