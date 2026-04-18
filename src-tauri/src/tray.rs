use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::config::{CommandValue, KeyBinding};

/// Returns the base URL of the display-dj sidecar HTTP server.
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

/// Shows the popup window, emits refresh events, and sets focus.
/// Used by both the tray left-click handler and the "Show Window" menu item.
/// Sets `expect_focus_gain` so the focus-loss handler won't hide us until
/// the window actually receives `Focused(true)`.
fn show_popup_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // Set flag so the focus-loss handler won't hide us until the window actually
        // receives Focused(true). This suppresses the spurious Focused(false) that
        // fires on Linux/X11 (and occasionally Windows) before focus arrives.
        if let Some(state) = app.try_state::<crate::AppState>() {
            if let Ok(mut e) = state.expect_focus_gain.lock() {
                *e = true;
            }
        }
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("monitors-changed", ());
        let _ = app.emit("dark-mode-changed", ());
        let _ = app.emit("volume-changed", ());
    }
}

/// Builds the tray context menu from current preferences.
/// Called on initial setup and after any action that changes the menu (debug toggle, reset).
fn build_tray_menu(app: &AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let debug_on = {
        if let Some(state) = app.try_state::<crate::AppState>() {
            state.preferences.lock().map(|p| p.debug_logging).unwrap_or(false)
        } else {
            false
        }
    };

    let show_window = MenuItemBuilder::with_id("show_window", "Show Window").build(app)?;
    let dark_mode = MenuItemBuilder::with_id("dark_mode", "Dark Mode").build(app)?;
    let light_mode = MenuItemBuilder::with_id("light_mode", "Light Mode").build(app)?;

    // Build profiles submenu from saved preferences
    let profiles = {
        if let Some(state) = app.try_state::<crate::AppState>() {
            state.preferences.lock().map(|p| p.profiles.clone()).unwrap_or_default()
        } else {
            crate::config::Preferences::default().profiles
        }
    };
    let mut profiles_submenu = SubmenuBuilder::new(app, "Profiles");
    for (i, profile) in profiles.iter().enumerate() {
        let label = if profile.name.is_empty() {
            format!("Unnamed Profile #{}", i + 1)
        } else {
            profile.name.clone()
        };
        let item_id = format!("profile_{}", i);
        let item = MenuItemBuilder::with_id(&item_id, &label).build(app)?;
        profiles_submenu = profiles_submenu.item(&item);
    }
    let profiles_submenu = profiles_submenu.build()?;

    // Debug submenu — items inside vary based on whether debug logging is on
    let debug_enable = MenuItemBuilder::with_id("debug_enable", "Enable Logging").build(app)?;
    let debug_disable = MenuItemBuilder::with_id("debug_disable", "Disable Logging").build(app)?;
    let debug_open = MenuItemBuilder::with_id("debug_open", "Open Debug Log").build(app)?;
    let debug_dump = MenuItemBuilder::with_id("debug_dump", "Dump Debug Info").build(app)?;
    let open_prefs =
        MenuItemBuilder::with_id("open_prefs", "Open App Preferences").build(app)?;
    let open_folder =
        MenuItemBuilder::with_id("open_folder", "Open App Folder").build(app)?;
    let port = crate::server_port();
    let bridge_label = format!("Bridge: 127.0.0.1:{}", port);
    let bridge = MenuItemBuilder::with_id("bridge", &bridge_label).build(app)?;
    let force_refresh =
        MenuItemBuilder::with_id("force_refresh", "Force Refresh").build(app)?;

    let reset_defaults =
        MenuItemBuilder::with_id("reset_defaults", "Reset to Default").build(app)?;

    let debug_submenu = if debug_on {
        SubmenuBuilder::new(app, "Debug")
            .item(&debug_disable)
            .item(&debug_open)
            .item(&debug_dump)
            .separator()
            .item(&open_prefs)
            .item(&open_folder)
            .item(&bridge)
            .separator()
            .item(&force_refresh)
            .separator()
            .item(&reset_defaults)
            .build()?
    } else {
        SubmenuBuilder::new(app, "Debug")
            .item(&debug_enable)
            .item(&debug_dump)
            .separator()
            .item(&open_prefs)
            .item(&open_folder)
            .item(&bridge)
            .separator()
            .item(&force_refresh)
            .separator()
            .item(&reset_defaults)
            .build()?
    };

    // Tiling submenu — macOS + Windows + Linux, toggle + layouts (only shown when enabled)
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    let (tiling_on, expose_on) = {
        if let Some(state) = app.try_state::<crate::AppState>() {
            state
                .preferences
                .lock()
                .map(|p| (p.tiling.enabled, p.tiling.expose_enabled))
                .unwrap_or((true, true))
        } else {
            (true, true)
        }
    };

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    let tiling_submenu = if tiling_on {
        SubmenuBuilder::new(app, "Tiling")
            .item(&MenuItemBuilder::with_id("tiling_disable", "Disable Tiling").build(app)?)
            .separator()
            .item(&MenuItemBuilder::with_id("tile_leftHalf", "Left Half").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_rightHalf", "Right Half").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_topHalf", "Top Half").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_bottomHalf", "Bottom Half").build(app)?)
            .separator()
            .item(&MenuItemBuilder::with_id("tile_topLeftQuarter", "Top Left").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_topRightQuarter", "Top Right").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_bottomLeftQuarter", "Bottom Left").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_bottomRightQuarter", "Bottom Right").build(app)?)
            .separator()
            .item(&MenuItemBuilder::with_id("tile_leftThird", "Left Third").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_centerThird", "Center Third").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_rightThird", "Right Third").build(app)?)
            .separator()
            .item(&MenuItemBuilder::with_id("tile_leftTwoThirds", "Left Two-Thirds").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_rightTwoThirds", "Right Two-Thirds").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_topTwoThirds", "Top Two-Thirds").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_bottomTwoThirds", "Bottom Two-Thirds").build(app)?)
            .separator()
            .item(&MenuItemBuilder::with_id("tile_maximize", "Maximize").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_restore", "Restore").build(app)?)
            .build()?
    } else {
        SubmenuBuilder::new(app, "Tiling")
            .item(&MenuItemBuilder::with_id("tiling_enable", "Enable Tiling").build(app)?)
            .build()?
    };

    // Exposé submenu — only visible when tiling is supported (same platform gate)
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    let expose_submenu = if expose_on {
        let mut builder = SubmenuBuilder::new(app, "Exposé")
            .item(&MenuItemBuilder::with_id("expose_disable", "Disable Exposé").build(app)?)
            .separator()
            .item(&MenuItemBuilder::with_id("tile_expose", "Exposé").build(app)?)
            .item(&MenuItemBuilder::with_id("tile_exposeApp", "App Exposé").build(app)?)
            .separator();
        // Grid size options (columns × rows presets)
        let (cur_cols, cur_rows) = if let Some(state) = app.try_state::<crate::AppState>() {
            state
                .preferences
                .lock()
                .map(|p| (p.tiling.expose_columns, p.tiling.expose_rows))
                .unwrap_or((3, 3))
        } else {
            (3, 3)
        };
        for &(c, r) in &[(2u32, 2), (2, 3), (3, 3), (3, 4), (4, 4), (5, 5)] {
            let check = if c == cur_cols && r == cur_rows { "● " } else { "   " };
            let label = format!("{}{} \u{00d7} {} = {} windows", check, c, r, c * r);
            let id = format!("expose_grid_{}x{}", c, r);
            builder =
                builder.item(&MenuItemBuilder::with_id(&id, &label).build(app)?);
        }
        builder.build()?
    } else {
        SubmenuBuilder::new(app, "Exposé")
            .item(&MenuItemBuilder::with_id("expose_enable", "Enable Exposé").build(app)?)
            .build()?
    };

    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let mut menu = MenuBuilder::new(app)
        .item(&show_window)
        .separator()
        .items(&[&dark_mode, &light_mode])
        .separator()
        .item(&profiles_submenu);

    // Tiling + Exposé submenus on macOS + Windows + Linux (X11)
    // Layout Presets submenu — only shown when presets exist and tiling is supported
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    let layout_presets = {
        if let Some(state) = app.try_state::<crate::AppState>() {
            state.preferences.lock().map(|p| p.layout_presets.clone()).unwrap_or_default()
        } else {
            Vec::new()
        }
    };

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        menu = menu.separator().item(&tiling_submenu).item(&expose_submenu);
        if !layout_presets.is_empty() {
            let mut presets_submenu = SubmenuBuilder::new(app, "Layout Presets");
            for (i, preset) in layout_presets.iter().enumerate() {
                let label = if preset.name.is_empty() {
                    format!("Preset #{}", i + 1)
                } else {
                    preset.name.clone()
                };
                let id = format!("layout_preset_{}", i);
                presets_submenu = presets_submenu
                    .item(&MenuItemBuilder::with_id(&id, &label).build(app)?);
            }
            let presets_submenu = presets_submenu.build()?;
            menu = menu.item(&presets_submenu);
        }
    }

    let menu = menu
        .separator()
        .item(&debug_submenu)
        .separator()
        .item(&quit)
        .build()?;

    Ok(menu)
}

/// Rebuilds the tray context menu from current preferences and applies it.
fn rebuild_tray_menu(app: &AppHandle) {
    if let Ok(menu) = build_tray_menu(app) {
        if let Some(tray) = app.tray_by_id("main-tray") {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Builds the system tray icon, context menu, and event handlers.
/// Handles left-click (toggle popup) and menu actions (dark/light mode, profiles, debug, quit).
pub fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();
    let menu = build_tray_menu(handle)?;

    TrayIconBuilder::with_id("main-tray")
        .icon(crate::tray_icon::generate_tray_icon(false, false, false))
        .tooltip("Display DJ")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show_window" => {
                show_popup_window(app);
            }
            "bridge" => {
                let url = base_url();
                let _ = open::that(&url);
            }
            "dark_mode" => {
                let url = format!("{}/dark", base_url());
                http_get(url);
                crate::tray_icon::set_dark_mode_state(app, true);
            }
            "light_mode" => {
                let url = format!("{}/light", base_url());
                http_get(url);
                crate::tray_icon::set_dark_mode_state(app, false);
            }
            "open_prefs" => {
                let _ = crate::config::open_preferences_file();
            }
            "open_folder" => {
                let _ = crate::config::open_app_folder();
            }
            "tiling_enable" => {
                if let Some(state) = app.try_state::<crate::AppState>() {
                    crate::config::set_tiling_enabled(&state, true);
                }
                rebuild_tray_menu(app);
            }
            "tiling_disable" => {
                if let Some(state) = app.try_state::<crate::AppState>() {
                    crate::config::set_tiling_enabled(&state, false);
                }
                rebuild_tray_menu(app);
            }
            "expose_enable" => {
                if let Some(state) = app.try_state::<crate::AppState>() {
                    crate::config::set_expose_enabled(&state, true);
                }
                rebuild_tray_menu(app);
            }
            "expose_disable" => {
                if let Some(state) = app.try_state::<crate::AppState>() {
                    crate::config::set_expose_enabled(&state, false);
                }
                rebuild_tray_menu(app);
            }
            "debug_enable" => {
                if let Some(state) = app.try_state::<crate::AppState>() {
                    crate::config::set_debug_logging(&state, true);
                }
                rebuild_tray_menu(app);
            }
            "debug_disable" => {
                if let Some(state) = app.try_state::<crate::AppState>() {
                    crate::config::set_debug_logging(&state, false);
                }
                rebuild_tray_menu(app);
            }
            "debug_open" => {
                let _ = crate::config::open_debug_log();
            }
            "debug_dump" => {
                dump_debug_info(app);
            }
            "reset_defaults" => {
                use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
                let app_clone = app.clone();
                let confirmed = app.dialog()
                    .message("This will reset all preferences, keybindings, and profiles to their defaults. This cannot be undone.")
                    .title("Reset to Default")
                    .buttons(MessageDialogButtons::OkCancelCustom("Reset".into(), "Cancel".into()))
                    .blocking_show();
                if confirmed {
                    crate::config::reset_to_defaults();
                    // Reload in-memory state
                    if let Some(state) = app_clone.try_state::<crate::AppState>() {
                        if let Ok(mut prefs) = state.preferences.lock() {
                            *prefs = crate::config::load_preferences();
                        }
                    }
                    // Re-register shortcuts with default keybindings
                    let prefs = crate::config::Preferences::default();
                    register_shortcuts(&app_clone, &prefs.key_bindings);
                    // Rebuild tray menu to reflect reset state
                    rebuild_tray_menu(&app_clone);
                    // Notify frontend to refresh
                    let _ = app_clone.emit("monitors-changed", ());
                    let _ = app_clone.emit("dark-mode-changed", ());
                    let _ = app_clone.emit("volume-changed", ());
                }
            }
            "force_refresh" => {
                // Reload preferences from disk into in-memory state
                if let Some(state) = app.try_state::<crate::AppState>() {
                    if let Ok(mut prefs) = state.preferences.lock() {
                        *prefs = crate::config::load_preferences();
                        // Re-register shortcuts with reloaded keybindings
                        register_shortcuts(app, &prefs.key_bindings);
                    }
                }
                // Rebuild tray menu to reflect any preference changes
                rebuild_tray_menu(app);
                // Notify frontend to refresh all data
                let _ = app.emit("monitors-changed", ());
                let _ = app.emit("dark-mode-changed", ());
                let _ = app.emit("volume-changed", ());
            }
            "quit" => {
                app.exit(0);
            }
            other => {
                if let Some(layout) = other.strip_prefix("tile_") {
                    let cmd = format!("command/tile/{}", layout);
                    execute_command(app, &cmd);
                } else if let Some(idx_str) = other.strip_prefix("profile_") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        let cmd = format!("command/changeProfile/{}", idx);
                        execute_command(app, &cmd);
                    }
                } else if let Some(grid_str) = other.strip_prefix("expose_grid_") {
                    // Parse "CxR" format (e.g. "3x4")
                    if let Some((c_str, r_str)) = grid_str.split_once('x') {
                        if let (Ok(c), Ok(r)) = (c_str.parse::<u32>(), r_str.parse::<u32>()) {
                            if let Some(state) = app.try_state::<crate::AppState>() {
                                if let Ok(mut prefs) = state.preferences.lock() {
                                    prefs.tiling.expose_columns = c;
                                    prefs.tiling.expose_rows = r;
                                    crate::config::save_preferences_to_disk(&prefs);
                                }
                            }
                            rebuild_tray_menu(app);
                        }
                    }
                } else if let Some(idx_str) = other.strip_prefix("layout_preset_") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        let cmd = format!("command/layout/{}", idx);
                        execute_command(app, &cmd);
                    }
                }
            }
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
                            // Store tray rect so resize handler can reposition
                            if let Some(state) = app.try_state::<crate::AppState>() {
                                if let Ok(mut stored) = state.last_tray_rect.lock() {
                                    *stored = Some(tray_rect);
                                }
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
                        show_popup_window(app);
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
/// at 1 MB). Open it via the tray menu → "Open Debug Log".
pub fn position_window_near_tray(
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

    // Clamp to monitor bounds with a safety margin so the window is never
    // flush against the screen edge (prevents content from being cut off).
    let margin = 8.0 * target_scale;
    x = x.max(mon_x + margin).min(mon_x + mon_w - win_w - margin);
    y = y.max(mon_y + margin).min(mon_y + mon_h - win_h - margin);

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

/// Registers global keyboard shortcuts from the user's key binding preferences.
/// Unregisters all existing shortcuts first, then re-registers from the provided bindings.
pub fn register_shortcuts(app: &AppHandle, key_bindings: &[KeyBinding]) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let _ = app.global_shortcut().unregister_all();
    log::info!(
        "register_shortcuts: registering {} keybindings",
        key_bindings.len()
    );

    let mut registered = 0;
    let mut failed = 0;
    for binding in key_bindings {
        let commands: Vec<String> = match &binding.command {
            CommandValue::Single(cmd) => vec![cmd.clone()],
            CommandValue::Multiple(cmds) => cmds.clone(),
        };

        let handle = app.clone();
        let key = binding.key.clone();
        let key_for_log = binding.key.clone();
        let cmds_for_log: Vec<String> = commands.clone();

        if let Ok(shortcut) = key.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            match app.global_shortcut().on_shortcut(
                shortcut,
                move |_app, _shortcut, _event| {
                    log::info!("shortcut triggered: '{}' → {:?}", key_for_log, cmds_for_log);
                    for cmd in &commands {
                        execute_command(&handle, cmd);
                    }
                },
            ) {
                Ok(_) => {
                    log::info!(
                        "register_shortcuts: registered '{}' → {:?}",
                        binding.key,
                        match &binding.command {
                            CommandValue::Single(c) => vec![c.clone()],
                            CommandValue::Multiple(c) => c.clone(),
                        }
                    );
                    registered += 1;
                }
                Err(e) => {
                    log::warn!(
                        "register_shortcuts: failed to register '{}': {}",
                        binding.key,
                        e
                    );
                    failed += 1;
                }
            }
        } else {
            log::warn!("register_shortcuts: failed to parse shortcut: '{}'", key);
            failed += 1;
        }
    }

    log::info!(
        "register_shortcuts: done — {} registered, {} failed",
        registered,
        failed
    );
}

/// Dumps current app state to the debug log for troubleshooting.
/// Includes version, preferences, keybindings, tiling/exposé state, and platform info.
fn dump_debug_info(app: &AppHandle) {
    let mut lines: Vec<String> = Vec::new();
    lines.push("=== DEBUG INFO DUMP ===".into());
    lines.push(format!(
        "version: {}",
        crate::config::get_app_version()
    ));
    lines.push(format!("os: {} {}", std::env::consts::OS, std::env::consts::ARCH));
    lines.push(format!("bridge: 127.0.0.1:{}", crate::server_port()));

    if let Some(state) = app.try_state::<crate::AppState>() {
        if let Ok(prefs) = state.preferences.lock() {
            lines.push(format!("debug_logging: {}", prefs.debug_logging));
            lines.push(format!("launch_at_login: {}", prefs.launch_at_login));
            lines.push(format!("show_individual_displays: {}", prefs.show_individual_displays));
            lines.push(format!("show_contrast: {}", prefs.show_contrast));
            lines.push(format!("min_brightness: {}", prefs.min_brightness));

            // Tiling state
            lines.push("--- tiling ---".into());
            lines.push(format!("tiling.enabled: {}", prefs.tiling.enabled));
            lines.push(format!("tiling.half_ratio: {}", prefs.tiling.half_ratio));
            lines.push(format!("tiling.third_ratio: {}", prefs.tiling.third_ratio));
            lines.push(format!("tiling.gap: {}", prefs.tiling.gap));
            lines.push(format!("tiling.side_edge_trigger: {}", prefs.tiling.side_edge_trigger));
            lines.push(format!("tiling.top_edge_trigger: {}", prefs.tiling.top_edge_trigger));
            lines.push(format!("tiling.corner_trigger: {}", prefs.tiling.corner_trigger));

            // Exposé state
            lines.push("--- exposé ---".into());
            lines.push(format!("tiling.expose_enabled: {}", prefs.tiling.expose_enabled));
            lines.push(format!(
                "tiling.expose_grid: {}x{} = {} windows",
                prefs.tiling.expose_columns,
                prefs.tiling.expose_rows,
                prefs.tiling.expose_columns * prefs.tiling.expose_rows,
            ));

            // Night mode
            lines.push("--- night mode ---".into());
            lines.push(format!(
                "night_mode: enabled={}, night_start={}, day_start={}, night_brightness={}, day_brightness={}",
                prefs.night_mode_schedule.enabled,
                prefs.night_mode_schedule.night_start,
                prefs.night_mode_schedule.day_start,
                prefs.night_mode_schedule.night_brightness,
                prefs.night_mode_schedule.day_brightness,
            ));

            // Keybindings
            lines.push(format!("--- keybindings ({}) ---", prefs.key_bindings.len()));
            for kb in &prefs.key_bindings {
                let cmds = match &kb.command {
                    crate::config::CommandValue::Single(c) => c.clone(),
                    crate::config::CommandValue::Multiple(c) => c.join(", "),
                };
                lines.push(format!("  {} → {}", kb.key, cmds));
            }

            // Profiles
            lines.push(format!("--- profiles ({}) ---", prefs.profiles.len()));
            for (i, p) in prefs.profiles.iter().enumerate() {
                let cmds = match &p.command {
                    crate::config::CommandValue::Single(c) => c.clone(),
                    crate::config::CommandValue::Multiple(c) => c.join(", "),
                };
                lines.push(format!("  [{}] {} → {}", i, p.name, cmds));
            }

            // Monitor configs
            lines.push(format!(
                "--- monitor configs ({}) ---",
                prefs.monitor_configs.len()
            ));
            for mc in &prefs.monitor_configs {
                lines.push(format!(
                    "  uid={}, label=\"{}\", sort={}, hidden={}",
                    mc.uid, mc.label, mc.sort_order, mc.hidden
                ));
            }
        }
    }

    lines.push("=== END DEBUG INFO ===".into());
    let output = lines.join("\n");
    log::info!("{}", output);

    // Also write to debug log file regardless of debug_logging preference
    if let Some(state) = app.try_state::<crate::AppState>() {
        crate::config::write_debug_log(&state, &output);
    }
}

/// Dispatches a command string (e.g. "command/changeBrightness/50") to the appropriate
/// sidecar HTTP endpoint. Used by keyboard shortcuts, profiles, tray menu actions,
/// and the night mode schedule.
pub(crate) fn execute_command(app: &AppHandle, command: &str) {
    log::info!("execute_command: '{}'", command);
    let parts: Vec<&str> = command.split('/').collect();
    let base = base_url();
    match parts.as_slice() {
        // Set brightness for all monitors: command/changeBrightness/{value}
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
        // Set brightness for a single monitor: command/changeBrightness/{monitor_id}/{value}
        ["command", "changeBrightness", monitor_id, value] => {
            if let Ok(val) = value.parse::<u32>() {
                let min = app
                    .state::<crate::AppState>()
                    .preferences
                    .lock()
                    .map(|p| p.effective_min_brightness())
                    .unwrap_or(crate::config::ABSOLUTE_MIN_BRIGHTNESS);
                let url = format!("{}/set_one/{}/{}", base, monitor_id, val.clamp(min, 100));
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
                                let is_dark = text.contains("dark");
                                let route = if is_dark { "light" } else { "dark" };
                                let _ = reqwest::blocking::get(format!("{}/{}", base_clone, route));
                                crate::tray_icon::set_dark_mode_state(&app_clone, !is_dark);
                            }
                        }
                        let _ = app_clone.emit("dark-mode-changed", ());
                    });
                    return;
                }
                "dark" => {
                    crate::tray_icon::set_dark_mode_state(app, true);
                    format!("{}/dark", base)
                }
                "light" => {
                    crate::tray_icon::set_dark_mode_state(app, false);
                    format!("{}/light", base)
                }
                _ => return,
            };
            http_get_then_emit(url, app.clone(), "dark-mode-changed");
        }
        // Set contrast for all monitors: command/changeContrast/{value}
        ["command", "changeContrast", value] => {
            if let Ok(val) = value.parse::<u32>() {
                let url = format!("{}/set_contrast_all/{}", base, val.min(100));
                http_get_then_emit(url, app.clone(), "monitors-changed");
            }
        }
        // Set contrast for a single monitor: command/changeContrast/{monitor_id}/{value}
        ["command", "changeContrast", monitor_id, value] => {
            if let Ok(val) = value.parse::<u32>() {
                let url = format!("{}/set_contrast_one/{}/{}", base, monitor_id, val.min(100));
                http_get_then_emit(url, app.clone(), "monitors-changed");
            }
        }
        ["command", "changeVolume", value] => {
            if let Ok(val) = value.parse::<u32>() {
                let clamped = val.min(100);
                let url = format!("{}/set_volume/{}", base, clamped);
                crate::tray_icon::set_muted_state(app, clamped == 0);
                http_get_then_emit(url, app.clone(), "volume-changed");
            }
        }
        ["command", "changeProfile", idx_str] => {
            if let Ok(idx) = idx_str.parse::<usize>() {
                let profiles = app
                    .state::<crate::AppState>()
                    .preferences
                    .lock()
                    .map(|p| p.profiles.clone())
                    .unwrap_or_default();

                if let Some(profile) = profiles.get(idx) {
                    let commands: Vec<String> = match &profile.command {
                        CommandValue::Single(cmd) => vec![cmd.clone()],
                        CommandValue::Multiple(cmds) => cmds.clone(),
                    };
                    for cmd in &commands {
                        execute_command(app, cmd);
                    }
                } else {
                    log::warn!("Profile index out of range: {}", idx);
                }
            }
        }
        ["command", "tile", layout] => {
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            {
                if *layout == "expose" {
                    let app_clone = app.clone();
                    std::thread::spawn(move || {
                        crate::tiling::execute_expose(&app_clone);
                    });
                } else if *layout == "exposeApp" {
                    let app_clone = app.clone();
                    std::thread::spawn(move || {
                        crate::tiling::execute_expose_app(&app_clone);
                    });
                } else {
                    let app_clone = app.clone();
                    let layout = layout.to_string();
                    std::thread::spawn(move || {
                        crate::tiling::execute_tile(&app_clone, &layout);
                    });
                }
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            log::warn!("Tiling is not yet supported on this platform: {}", layout);
        }
        // Apply a layout preset: command/layout/{name_or_index}
        ["command", "layout", name_or_index] => {
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            {
                let app_clone = app.clone();
                let preset_id = name_or_index.to_string();
                std::thread::spawn(move || {
                    crate::tiling::execute_layout_preset(&app_clone, &preset_id);
                });
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            log::warn!("Layout presets are not supported on this platform: {}", name_or_index);
        }
        // Set wallpaper: command/wallpaper/change/{path} or command/wallpaper/change/{fit}/{path}
        ["command", "wallpaper", "change", ..] => {
            let prefix = "command/wallpaper/change/";
            if command.len() > prefix.len() {
                let remainder = &command[prefix.len()..];
                let (fit, path) = crate::wallpaper::parse_wallpaper_args(remainder);
                let app_clone = app.clone();
                let path_owned = path.to_string();
                let fit_owned = fit.map(|f| f.to_string());
                std::thread::spawn(move || {
                    let state = app_clone.state::<crate::AppState>();
                    crate::wallpaper::change_wallpaper(
                        &state,
                        &path_owned,
                        fit_owned.as_deref(),
                    );
                });
            } else {
                log::warn!("wallpaper change command missing path: {}", command);
            }
        }
        _ => {
            log::warn!("Unknown command: {}", command);
        }
    }
}

/// Builds the sidecar HTTP URL for a given command string, or returns None
/// for commands that don't map to a simple HTTP GET (profiles, tiling, dark mode toggle).
/// `base` is the sidecar base URL, `min_brightness` is the effective floor.
fn build_command_url(command: &str, base: &str, min_brightness: u32) -> Option<String> {
    let parts: Vec<&str> = command.split('/').collect();
    match parts.as_slice() {
        ["command", "changeBrightness", value] => {
            value.parse::<u32>().ok().map(|v| {
                format!("{}/set_all/{}", base, v.clamp(min_brightness, 100))
            })
        }
        ["command", "changeBrightness", monitor_id, value] => {
            value.parse::<u32>().ok().map(|v| {
                format!("{}/set_one/{}/{}", base, monitor_id, v.clamp(min_brightness, 100))
            })
        }
        ["command", "changeContrast", value] => {
            value.parse::<u32>().ok().map(|v| {
                format!("{}/set_contrast_all/{}", base, v.min(100))
            })
        }
        ["command", "changeContrast", monitor_id, value] => {
            value.parse::<u32>().ok().map(|v| {
                format!("{}/set_contrast_one/{}/{}", base, monitor_id, v.min(100))
            })
        }
        ["command", "changeVolume", value] => {
            value.parse::<u32>().ok().map(|v| {
                format!("{}/set_volume/{}", base, v.min(100))
            })
        }
        _ => None,
    }
}

/// Applies a saved profile by index, executing all of its commands.
#[tauri::command]
pub fn apply_profile(app: AppHandle, index: usize) -> Result<(), String> {
    execute_command(&app, &format!("command/changeProfile/{}", index));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "http://127.0.0.1:51337";

    /// Verifies all-monitors brightness command produces the correct sidecar URL.
    #[test]
    fn test_build_url_brightness_all() {
        let url = build_command_url("command/changeBrightness/75", BASE, 10);
        assert_eq!(url, Some(format!("{}/set_all/75", BASE)));
    }

    /// Verifies per-monitor brightness command produces the correct sidecar URL.
    #[test]
    fn test_build_url_brightness_single() {
        let url = build_command_url("command/changeBrightness/1/80", BASE, 10);
        assert_eq!(url, Some(format!("{}/set_one/1/80", BASE)));
    }

    /// Verifies per-monitor brightness with builtin monitor ID.
    #[test]
    fn test_build_url_brightness_single_builtin() {
        let url = build_command_url("command/changeBrightness/builtin/50", BASE, 10);
        assert_eq!(url, Some(format!("{}/set_one/builtin/50", BASE)));
    }

    /// Verifies brightness is clamped to min_brightness floor.
    #[test]
    fn test_build_url_brightness_clamps_to_min() {
        let url = build_command_url("command/changeBrightness/3", BASE, 10);
        assert_eq!(url, Some(format!("{}/set_all/10", BASE)));
    }

    /// Verifies per-monitor brightness is clamped to min_brightness floor.
    #[test]
    fn test_build_url_brightness_single_clamps_to_min() {
        let url = build_command_url("command/changeBrightness/1/3", BASE, 10);
        assert_eq!(url, Some(format!("{}/set_one/1/10", BASE)));
    }

    /// Verifies all-monitors contrast command produces the correct sidecar URL.
    #[test]
    fn test_build_url_contrast_all() {
        let url = build_command_url("command/changeContrast/60", BASE, 10);
        assert_eq!(url, Some(format!("{}/set_contrast_all/60", BASE)));
    }

    /// Verifies per-monitor contrast command produces the correct sidecar URL.
    #[test]
    fn test_build_url_contrast_single() {
        let url = build_command_url("command/changeContrast/2/70", BASE, 10);
        assert_eq!(url, Some(format!("{}/set_contrast_one/2/70", BASE)));
    }

    /// Verifies contrast is capped at 100.
    #[test]
    fn test_build_url_contrast_clamps_to_100() {
        let url = build_command_url("command/changeContrast/150", BASE, 10);
        assert_eq!(url, Some(format!("{}/set_contrast_all/100", BASE)));
    }

    /// Verifies volume command produces the correct sidecar URL.
    #[test]
    fn test_build_url_volume() {
        let url = build_command_url("command/changeVolume/50", BASE, 10);
        assert_eq!(url, Some(format!("{}/set_volume/50", BASE)));
    }

    /// Verifies unknown commands return None.
    #[test]
    fn test_build_url_unknown_command() {
        assert_eq!(build_command_url("command/unknown/123", BASE, 10), None);
    }

    /// Verifies non-numeric values return None.
    #[test]
    fn test_build_url_invalid_value() {
        assert_eq!(build_command_url("command/changeBrightness/abc", BASE, 10), None);
    }

    /// Verifies per-monitor command with non-numeric value returns None.
    #[test]
    fn test_build_url_single_invalid_value() {
        assert_eq!(build_command_url("command/changeBrightness/1/abc", BASE, 10), None);
    }

    /// Verifies wallpaper commands are not HTTP-routed (return None from build_command_url).
    #[test]
    fn test_build_url_wallpaper_returns_none() {
        assert_eq!(
            build_command_url("command/wallpaper/change//Users/pic.jpg", BASE, 10),
            None
        );
        assert_eq!(
            build_command_url("command/wallpaper/change/center//Users/pic.jpg", BASE, 10),
            None
        );
    }
}
