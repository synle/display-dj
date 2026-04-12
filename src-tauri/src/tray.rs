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
    let open_debug_log =
        MenuItemBuilder::with_id("open_debug_log", "Open Debug Log").build(app)?;
    let reset_defaults =
        MenuItemBuilder::with_id("reset_defaults", "Reset to Default").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&bridge)
        .separator()
        .items(&[&dark_mode, &light_mode])
        .separator()
        .items(&[&open_configs, &open_prefs, &open_debug_log])
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
            "open_debug_log" => {
                let _ = crate::config::open_debug_log();
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
                    if let Some(state) = app.try_state::<crate::AppState>() {
                        crate::config::write_debug_log(
                            &state,
                            &format!("tray_click: visible={}", visible),
                        );
                    }
                    if visible {
                        let _ = window.hide();
                    } else {
                        if let Ok(Some(tray_rect)) = tray.rect() {
                            if let Some(state) = app.try_state::<crate::AppState>() {
                                crate::config::write_debug_log(
                                    &state,
                                    &format!(
                                        "tray_rect: pos={:?} size={:?}",
                                        tray_rect.position, tray_rect.size
                                    ),
                                );
                            }
                            let result = position_window_near_tray(&window, tray_rect, app.try_state::<crate::AppState>());
                            if let Some(state) = app.try_state::<crate::AppState>() {
                                crate::config::write_debug_log(
                                    &state,
                                    &format!("position_result: {:?}", result.as_ref().map(|_| "ok")),
                                );
                            }
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

/// Position the popup window directly below (or above) the system tray icon.
///
/// # Multi-monitor DPI pitfall (the hard-won lesson)
///
/// On macOS, Tauri's coordinate APIs behave as follows:
///
///   - `tray.rect()`, `monitor.position()`, `monitor.size()` all return values
///     in a **global physical-pixel coordinate space**. Internally macOS works
///     in points (logical), but Tauri multiplies by each display's scale factor
///     when reporting positions as `PhysicalPosition`/`PhysicalSize`.
///
///   - `window.set_position(PhysicalPosition(x, y))` does **not** place the
///     window at physical pixel (x, y). Instead Tauri converts to platform
///     coordinates (macOS points) by dividing: `point = x / window.scale_factor()`.
///
///   - `window.scale_factor()` returns the scale of **the monitor the window is
///     currently on**, and it does NOT update synchronously after `set_position`.
///
/// This means that if the window is on a 1× external monitor (scale=1) and you
/// click the tray on the 2× built-in Retina display (scale=2):
///
///   - The tray position comes back in 2× physical coords, e.g. x=12380
///   - We compute the desired position in the same physical space, e.g. x=12054
///   - `set_position(12054)` → Tauri divides by window_scale (1) → macOS point 12054
///   - But the correct macOS point is 12054 / 2 = 6027 → **window goes off-screen!**
///
/// Attempted fix that **does not work**: moving the hidden window to the target
/// monitor first and then calling `scale_factor()`. The scale factor does not
/// update synchronously after `set_position`, so the second call still returns
/// the old monitor's scale.
///
/// # The fix: scale compensation
///
/// We compute everything in the global physical space using `target_scale`
/// (the scale of the monitor where the tray icon is). Then, right before
/// calling `set_position`, we apply a compensation factor:
///
/// ```text
///   Tauri does:       point = physical_arg / window_scale
///   We need:          point = physical     / target_scale
///   Therefore pass:   physical_arg = physical * window_scale / target_scale
/// ```
///
/// When window and target are on the same monitor, `window_scale == target_scale`
/// and the compensation is 1 (no-op). When they differ, it corrects the mismatch.
///
/// # Debug logging
///
/// When "Debug Logging" is enabled in preferences, every tray click writes
/// detailed positioning data to `debug.log` in the config directory (capped
/// at 512 KB). Open it via the tray menu → "Open Debug Log".
fn position_window_near_tray(
    window: &tauri::WebviewWindow,
    tray_rect: tauri::Rect,
    state: Option<tauri::State<'_, crate::AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::PhysicalPosition;

    let dbg = |msg: &str| {
        if let Some(ref s) = state {
            crate::config::write_debug_log(s, msg);
        }
    };

    // --- Phase 1: Find the target monitor using a rough scale estimate ---
    // We use window.scale_factor() only to convert Logical→Physical for the
    // hit-test. The exact value doesn't matter much here — it just needs to be
    // close enough to land in the right monitor's bounding box.
    let rough_scale = window.scale_factor().unwrap_or(2.0);
    dbg(&format!("phase1: rough_scale={}", rough_scale));

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
    dbg(&format!(
        "phase1: tray_rough x={} y={} w={} h={} cx={} cy={}",
        tray_x_rough, tray_y_rough, tray_w_rough, tray_h_rough, tray_cx, tray_cy
    ));

    let monitors = window.available_monitors()?;
    let mut target_idx: Option<usize> = None;
    for (i, m) in monitors.iter().enumerate() {
        let pos = m.position();
        let size = m.size();
        let scale = m.scale_factor();
        dbg(&format!(
            "  monitor[{}]: pos=({},{}) size={}x{} scale={}",
            i, pos.x, pos.y, size.width, size.height, scale
        ));
        if target_idx.is_none()
            && tray_cx >= pos.x as f64
            && tray_cx < pos.x as f64 + size.width as f64
            && tray_cy >= pos.y as f64
            && tray_cy < pos.y as f64 + size.height as f64
        {
            target_idx = Some(i);
        }
    }

    let target = target_idx.map(|i| &monitors[i]);
    let target_scale = target.map(|m| m.scale_factor()).unwrap_or(rough_scale);
    let window_scale = window.scale_factor().unwrap_or(1.0);
    let win_pos = window.outer_position().ok();
    if let Some(ref t) = target {
        dbg(&format!(
            "target: monitor[{}] pos=({},{}) size={}x{} scale={} | window_scale={} window_pos={:?}",
            target_idx.unwrap_or(0),
            t.position().x, t.position().y,
            t.size().width, t.size().height,
            target_scale, window_scale, win_pos
        ));
    } else {
        dbg(&format!("target: NONE | window_scale={} window_pos={:?}", window_scale, win_pos));
    }

    // --- Phase 2: Calculate position using target monitor's scale ---
    // All tray/monitor coordinates are in the global physical space.
    // We use target_scale (not window_scale) for sizing, then compensate
    // in set_position because Tauri divides by window_scale internally.
    let (tray_x, tray_y) = match tray_rect.position {
        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
        tauri::Position::Logical(p) => (p.x * target_scale, p.y * target_scale),
    };
    let (tray_w, tray_h) = match tray_rect.size {
        tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
        tauri::Size::Logical(s) => (s.width * target_scale, s.height * target_scale),
    };

    // Window size in target monitor's physical pixels
    let win_w = 360.0 * target_scale;
    let win_h = window.outer_size()?.height as f64 * target_scale / window_scale;
    dbg(&format!(
        "tray x={} y={} w={} h={} | win_w={} win_h={}",
        tray_x, tray_y, tray_w, tray_h, win_w, win_h
    ));

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
    let gap = 4.0 * target_scale;
    let mut y = if tray_in_top_half {
        tray_y + tray_h + gap
    } else {
        tray_y - win_h - gap
    };
    dbg(&format!(
        "before_clamp x={} y={} | mon ({},{}) {}x{} | top_half={}",
        x, y, mon_x, mon_y, mon_w, mon_h, tray_in_top_half
    ));

    // Clamp to monitor bounds
    x = x.max(mon_x).min(mon_x + mon_w - win_w);
    y = y.max(mon_y).min(mon_y + mon_h - win_h);

    // Compensate for Tauri's set_position dividing by window_scale.
    // We computed (x, y) in the global physical space. Tauri will do:
    //   platform_pos = physical / window_scale
    // But we need:
    //   platform_pos = physical / target_scale
    // So we pass: physical * window_scale / target_scale
    let comp = window_scale / target_scale;
    let final_x = (x * comp) as i32;
    let final_y = (y * comp) as i32;
    dbg(&format!(
        "final: x={} y={} comp={} set_pos=({},{})",
        x, y, comp, final_x, final_y
    ));

    window.set_position(PhysicalPosition::new(final_x, final_y))?;
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
