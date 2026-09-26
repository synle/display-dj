use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::config::{CommandValue, KeyBinding};

const AUDIO_OUTPUT_MENU_PREFIX: &str = "audio_output_";
const SOUND_MUTE_MENU_ID: &str = "sound_volume_0";
const SOUND_HALF_MENU_ID: &str = "sound_volume_50";
const SOUND_FULL_MENU_ID: &str = "sound_volume_100";
const AUDIO_OUTPUT_REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
const POPUP_GAP_LOGICAL: f64 = 4.0;
const POPUP_MARGIN_LOGICAL: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct PhysicalBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopupVerticalPlacement {
    Below,
    Above,
    Clamped,
}

/// Selects the containing monitor nearest an edge, or the nearest monitor across a gap.
fn monitor_index_for_point(monitors: &[PhysicalBounds], x: f64, y: f64) -> Option<usize> {
    monitors
        .iter()
        .enumerate()
        .filter(|(_, monitor)| {
            x >= monitor.x
                && x < monitor.x + monitor.width
                && y >= monitor.y
                && y < monitor.y + monitor.height
        })
        .min_by(|(_, left), (_, right)| {
            distance_to_nearest_edge(**left, x, y)
                .total_cmp(&distance_to_nearest_edge(**right, x, y))
        })
        .map(|(index, _)| index)
        .or_else(|| {
            monitors
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| {
                    distance_to_bounds_squared(**left, x, y)
                        .total_cmp(&distance_to_bounds_squared(**right, x, y))
                })
                .map(|(index, _)| index)
        })
}

/// Returns distance to the nearest edge; tray clicks sit shallowest in their real display.
fn distance_to_nearest_edge(bounds: PhysicalBounds, x: f64, y: f64) -> f64 {
    [
        x - bounds.x,
        bounds.x + bounds.width - x,
        y - bounds.y,
        bounds.y + bounds.height - y,
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min)
}

/// Returns squared distance from a point to the nearest point inside one rectangle.
fn distance_to_bounds_squared(bounds: PhysicalBounds, x: f64, y: f64) -> f64 {
    let nearest_x = x.clamp(bounds.x, bounds.x + bounds.width);
    let nearest_y = y.clamp(bounds.y, bounds.y + bounds.height);
    (x - nearest_x).powi(2) + (y - nearest_y).powi(2)
}

/// Places the popup below the tray, flips above on bottom overflow, then clamps every edge.
fn place_popup_in_monitor(
    tray: PhysicalBounds,
    popup_width: f64,
    popup_height: f64,
    monitor: PhysicalBounds,
    scale: f64,
) -> (f64, f64, PopupVerticalPlacement) {
    let gap = POPUP_GAP_LOGICAL * scale;
    let margin = POPUP_MARGIN_LOGICAL * scale;
    let min_x = monitor.x + margin;
    let max_x = (monitor.x + monitor.width - popup_width - margin).max(min_x);
    let min_y = monitor.y + margin;
    let max_y = (monitor.y + monitor.height - popup_height - margin).max(min_y);
    let x = (tray.x + tray.width / 2.0 - popup_width / 2.0).clamp(min_x, max_x);
    if popup_height > monitor.height - margin * 2.0 {
        return (x, min_y, PopupVerticalPlacement::Clamped);
    }

    let below_y = tray.y + tray.height + gap;
    if below_y <= max_y {
        return (x, below_y.max(min_y), PopupVerticalPlacement::Below);
    }

    let above_y = tray.y - popup_height - gap;
    if above_y >= min_y {
        return (x, above_y.min(max_y), PopupVerticalPlacement::Above);
    }

    (x, below_y.clamp(min_y, max_y), PopupVerticalPlacement::Clamped)
}

/// Run a closure on a background thread (so we don't block whatever called us)
/// and emit a Tauri event on `app` when it returns.
fn run_then_emit<R: tauri::Runtime, F>(app: AppHandle<R>, event: &'static str, f: F)
where
    F: FnOnce() + Send + 'static,
{
    std::thread::spawn(move || {
        f();
        let _ = app.emit(event, ());
    });
}

/// Dispatch a brightness change for a single monitor through the
/// per-monitor mode routing. Mirrors the dispatcher in
/// `display::set_brightness` but synchronous (called from the background
/// thread spawned by `run_then_emit`).
///
/// # Arguments
/// * `app` - Tauri `AppHandle` used by the overlay path to create / move /
///   hide its window.
/// * `monitor_id` - Raw `core::DisplayInfo.id`.
/// * `value` - Brightness 0..=100 (already clamped by the caller).
/// * `mode` - Resolved brightness mode (`"auto"`, `"ddc"`, `"gamma"`, `"overlay"`).
/// * `monitor_rect` - Physical rect for the overlay path; `None` is acceptable
///   on platforms where it's not populated (the overlay call no-ops).
fn dispatch_brightness_for_one<R: tauri::Runtime>(
    app: &AppHandle<R>,
    monitor_id: &str,
    value: u32,
    mode: &str,
    monitor_rect: Option<(i32, i32, i32, i32)>,
) {
    let route = crate::display::route_for_mode(mode);
    match route {
        crate::display::BrightnessRoute::DdcOnly => {
            let _ = crate::overlay::destroy_overlay(app, monitor_id);
            let _ = crate::core::display::set_one_brightness(monitor_id, value as u16, "ddc");
        }
        crate::display::BrightnessRoute::GammaOnly => {
            let _ = crate::overlay::destroy_overlay(app, monitor_id);
            let _ = crate::core::display::set_one_brightness(monitor_id, value as u16, "gamma");
        }
        crate::display::BrightnessRoute::OverlayOnly => {
            let _ = crate::overlay::set_overlay_brightness(app, monitor_id, monitor_rect, value);
        }
        crate::display::BrightnessRoute::AutoWithOverlayFallback => {
            let ok = crate::core::display::set_one_brightness(monitor_id, value as u16, "force");
            if ok {
                let _ = crate::overlay::destroy_overlay(app, monitor_id);
            } else {
                let _ = crate::overlay::set_overlay_brightness(
                    app, monitor_id, monitor_rect, value,
                );
            }
        }
    }
}

/// Dispatch a brightness change for all monitors through per-monitor mode
/// routing. Iterates the cached monitor list; falls back to a single bulk
/// `core::display::set_all_brightness("force")` call when the cache is empty
/// (so first-run keyboard shortcuts still work).
///
/// # Arguments
/// * `app` - Tauri `AppHandle`, threaded into the overlay helpers.
/// * `value` - Brightness 0..=100 (already clamped by the caller).
/// * `configs` - Snapshot of preferences.monitor_configs for mode lookup.
/// * `cached` - Snapshot of the cached `Monitor` list (with `monitor_rect`).
fn dispatch_brightness_for_all<R: tauri::Runtime>(
    app: &AppHandle<R>,
    value: u32,
    configs: &[crate::config::MonitorMetadata],
    cached: &[crate::display::Monitor],
) {
    if cached.is_empty() {
        let _ = crate::core::display::set_all_brightness(value as u16, "force");
        return;
    }
    for m in cached {
        let mode = crate::display::resolve_brightness_mode(configs, &m.id);
        dispatch_brightness_for_one(app, &m.id, value, &mode, m.monitor_rect);
    }
}

/// Positions from the latest tray click, shows the popup, emits refresh events, and sets focus.
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
        position_popup_from_last_tray_click(app, &window);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("monitors-changed", ());
        let _ = app.emit("dark-mode-changed", ());
        let _ = app.emit("volume-changed", ());
    }
}

/// Reuses the latest left- or right-click tray anchor for shared popup placement.
fn position_popup_from_last_tray_click(app: &AppHandle, window: &tauri::WebviewWindow) {
    let Some(state) = app.try_state::<crate::AppState>() else {
        return;
    };
    let tray_rect = state.last_tray_rect.lock().ok().and_then(|rect| *rect);
    let click_position = state
        .last_tray_click_position
        .lock()
        .ok()
        .and_then(|position| *position);
    let (Some(tray_rect), Some(click_position)) = (tray_rect, click_position) else {
        crate::config::write_debug_log(&state, "popup_placement: no tray click anchor available");
        return;
    };

    let result = position_window_near_tray(window, tray_rect, click_position, Some(state.clone()));
    crate::config::write_debug_log(
        &state,
        &format!(
            "popup_placement: shared tray anchor result={:?}",
            result.as_ref().map(|_| "ok")
        ),
    );
}

/// Encodes arbitrary platform endpoint IDs into menu-safe hexadecimal IDs.
fn audio_output_menu_id(device_id: &str) -> String {
    let encoded = device_id
        .as_bytes()
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>();
    format!("{}{}", AUDIO_OUTPUT_MENU_PREFIX, encoded)
}

/// Decodes one audio-output menu ID back into its exact platform endpoint ID.
fn parse_audio_output_menu_id(menu_id: &str) -> Option<String> {
    let encoded = menu_id.strip_prefix(AUDIO_OUTPUT_MENU_PREFIX)?;
    if encoded.is_empty() || encoded.len() % 2 != 0 {
        return None;
    }

    let bytes = (0..encoded.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&encoded[index..index + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// Returns enabled tray choices with the selected endpoint marked.
fn audio_output_menu_entries(
    output_state: Option<&crate::core::audio_output::AudioOutputState>,
) -> Vec<(String, String)> {
    let Some(output_state) = output_state else {
        return Vec::new();
    };

    output_state
        .devices
        .iter()
        .filter(|device| {
            device.state == crate::core::audio_output::AudioOutputDeviceState::Enabled
        })
        .map(|device| {
            let selected = output_state.selected_device_id.as_deref() == Some(device.id.as_str());
            let marker = if selected { "● " } else { "   " };
            (
                audio_output_menu_id(&device.id),
                format!("{}{}", marker, device.name),
            )
        })
        .collect()
}

/// Maps one tray sound preset ID to the existing in-process volume command.
fn sound_menu_command(menu_id: &str) -> Option<&'static str> {
    match menu_id {
        SOUND_MUTE_MENU_ID => Some("command/changeVolume/0"),
        SOUND_HALF_MENU_ID => Some("command/changeVolume/50"),
        SOUND_FULL_MENU_ID => Some("command/changeVolume/100"),
        _ => None,
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
    let audio_output_state = app
        .try_state::<crate::AppState>()
        .and_then(|state| state.audio_output_state.lock().ok()?.clone());
    let mut audio_output_submenu = SubmenuBuilder::new(app, "Speakers");
    let output_entries = audio_output_menu_entries(audio_output_state.as_ref());
    if output_entries.is_empty() {
        let empty = MenuItemBuilder::with_id("audio_output_empty", "No output devices")
            .enabled(false)
            .build(app)?;
        audio_output_submenu = audio_output_submenu.item(&empty);
    } else {
        for (id, label) in output_entries {
            let item = MenuItemBuilder::with_id(id, label).build(app)?;
            audio_output_submenu = audio_output_submenu.item(&item);
        }
    }
    let mute = MenuItemBuilder::with_id(SOUND_MUTE_MENU_ID, "Mute").build(app)?;
    let half = MenuItemBuilder::with_id(SOUND_HALF_MENU_ID, "50%").build(app)?;
    let full = MenuItemBuilder::with_id(SOUND_FULL_MENU_ID, "100%").build(app)?;
    audio_output_submenu = audio_output_submenu
        .separator()
        .item(&mute)
        .item(&half)
        .item(&full);
    let audio_output_submenu = audio_output_submenu.build()?;

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
    // macOS-only: quick link to Accessibility settings (required for tiling)
    #[cfg(target_os = "macos")]
    let accessibility_settings =
        MenuItemBuilder::with_id("accessibility_settings", "Accessibility Settings").build(app)?;

    let force_refresh =
        MenuItemBuilder::with_id("force_refresh", "Force Refresh").build(app)?;

    let clear_wallpaper_cache =
        MenuItemBuilder::with_id("clear_wallpaper_cache", "Clear Wallpaper Cache").build(app)?;
    let reset_defaults =
        MenuItemBuilder::with_id("reset_defaults", "Reset to Default").build(app)?;

    let debug_submenu = if debug_on {
        let builder = SubmenuBuilder::new(app, "Debug")
            .item(&debug_disable)
            .item(&debug_open)
            .item(&debug_dump)
            .separator()
            .item(&open_prefs)
            .item(&open_folder);
        #[cfg(target_os = "macos")]
        let builder = builder.item(&accessibility_settings);
        builder
            .separator()
            .item(&force_refresh)
            .item(&clear_wallpaper_cache)
            .separator()
            .item(&reset_defaults)
            .build()?
    } else {
        let builder = SubmenuBuilder::new(app, "Debug")
            .item(&debug_enable)
            .item(&debug_dump)
            .separator()
            .item(&open_prefs)
            .item(&open_folder);
        #[cfg(target_os = "macos")]
        let builder = builder.item(&accessibility_settings);
        builder
            .separator()
            .item(&force_refresh)
            .item(&clear_wallpaper_cache)
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
        // Layout strategy (fill vs spread)
        let (cur_cols, cur_rows, cur_strategy) = if let Some(state) = app.try_state::<crate::AppState>() {
            state
                .preferences
                .lock()
                .map(|p| (p.tiling.expose_columns, p.tiling.expose_rows, p.tiling.expose_layout_strategy.clone()))
                .unwrap_or((2, 3, "fill".into()))
        } else {
            (2, 3, "fill".into())
        };
        let fill_check = if cur_strategy == "fill" { "● " } else { "   " };
        let spread_check = if cur_strategy == "spread" { "● " } else { "   " };
        builder = builder
            .item(&MenuItemBuilder::with_id("expose_strategy_fill", &format!("{}Fill (pack first)", fill_check)).build(app)?)
            .item(&MenuItemBuilder::with_id("expose_strategy_spread", &format!("{}Spread (distribute)", spread_check)).build(app)?)
            .separator();
        // Grid size options (columns × rows presets)
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
        .item(&audio_output_submenu)
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

    let about = MenuItemBuilder::with_id("about", "About Display DJ").build(app)?;

    let menu = menu
        .separator()
        .item(&debug_submenu)
        .separator()
        .item(&about)
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

/// Schedules a tray-menu rebuild on the runtime main thread.
pub(crate) fn schedule_tray_menu_rebuild(app: &AppHandle) {
    let app = app.clone();
    if let Err(error) = app.clone().run_on_main_thread(move || rebuild_tray_menu(&app)) {
        log::warn!("failed to schedule tray menu rebuild: {}", error);
    }
}

/// Refreshes the tray's audio-output snapshot every five seconds.
pub fn start_audio_output_refresh(app: AppHandle) {
    std::thread::spawn(move || loop {
        match crate::volume::refresh_audio_output_state(&app) {
            Ok((output_state, true)) => {
                crate::volume::notify_audio_output_state_changed(&app, &output_state)
            }
            Ok((_, false)) => {}
            Err(error) => log::debug!("failed to refresh audio output devices: {}", error),
        }
        std::thread::sleep(AUDIO_OUTPUT_REFRESH_INTERVAL);
    });
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
            "dark_mode" => {
                std::thread::spawn(|| {
                    let _ = crate::core::theme::set_dark_mode(true);
                });
                crate::tray_icon::set_dark_mode_state(app, true);
            }
            "light_mode" => {
                std::thread::spawn(|| {
                    let _ = crate::core::theme::set_dark_mode(false);
                });
                crate::tray_icon::set_dark_mode_state(app, false);
            }
            "open_prefs" => {
                let _ = crate::config::open_preferences_file();
            }
            "open_folder" => {
                let _ = crate::config::open_app_folder();
            }
            // macOS-only: open Accessibility settings for tiling permission
            "accessibility_settings" => {
                let _ = open::that(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                );
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
                    // Reload in-memory state and invalidate cache
                    if let Some(state) = app_clone.try_state::<crate::AppState>() {
                        if let Ok(mut prefs) = state.preferences.lock() {
                            *prefs = crate::config::load_preferences();
                        }
                        state.sidecar_cache.invalidate_all();
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
            "clear_wallpaper_cache" => {
                if let Some(state) = app.try_state::<crate::AppState>() {
                    crate::wallpaper::clear_wallpaper_cache(&state);
                }
            }
            "force_refresh" => {
                // Reload preferences from disk into in-memory state
                if let Some(state) = app.try_state::<crate::AppState>() {
                    if let Ok(mut prefs) = state.preferences.lock() {
                        *prefs = crate::config::load_preferences();
                        register_shortcuts(app, &prefs.key_bindings);
                    }
                    // Invalidate sidecar cache so next fetch is fresh
                    state.sidecar_cache.invalidate_all();
                }
                // Rebuild tray menu to reflect any preference changes
                rebuild_tray_menu(app);
                // Notify frontend to refresh all data
                let _ = app.emit("monitors-changed", ());
                let _ = app.emit("dark-mode-changed", ());
                let _ = app.emit("volume-changed", ());
            }
            "about" => {
                show_popup_window(app);
                let _ = app.emit("show-about", ());
            }
            "quit" => {
                app.exit(0);
            }
            other => {
                if let Some(command) = sound_menu_command(other) {
                    execute_command(app, command);
                } else if let Some(device_id) = parse_audio_output_menu_id(other) {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) =
                            crate::volume::select_audio_output_device(device_id, app).await
                        {
                            log::warn!("failed to select tray audio output: {}", error);
                        }
                    });
                } else if let Some(layout) = other.strip_prefix("tile_") {
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
                } else if other == "expose_strategy_fill" || other == "expose_strategy_spread" {
                    let strategy = if other == "expose_strategy_fill" { "fill" } else { "spread" };
                    if let Some(state) = app.try_state::<crate::AppState>() {
                        if let Ok(mut prefs) = state.preferences.lock() {
                            prefs.tiling.expose_layout_strategy = strategy.to_string();
                            crate::config::save_preferences_to_disk(&prefs);
                        }
                    }
                    rebuild_tray_menu(app);
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
                rect: tray_rect,
                position: click_position,
                button,
                button_state,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(state) = app.try_state::<crate::AppState>() {
                    if let Ok(mut stored) = state.last_tray_rect.lock() {
                        *stored = Some(tray_rect);
                    }
                    if let Ok(mut stored) = state.last_tray_click_position.lock() {
                        *stored = Some(click_position);
                    }
                }
                if button_state != MouseButtonState::Up {
                    return;
                }
                log_tray_click(app, button, click_position, tray_rect);
                if button != MouseButton::Left {
                    return;
                }
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
                        show_popup_window(app);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Writes one bounded tray-click snapshot when debug logging is enabled.
fn log_tray_click(
    app: &AppHandle,
    button: MouseButton,
    click_position: tauri::PhysicalPosition<f64>,
    tray_rect: tauri::Rect,
) {
    let Some(state) = app.try_state::<crate::AppState>() else {
        return;
    };
    let monitors = app.available_monitors().unwrap_or_default();
    let monitor_bounds = monitors
        .iter()
        .map(|monitor| PhysicalBounds {
            x: monitor.position().x as f64,
            y: monitor.position().y as f64,
            width: monitor.size().width as f64,
            height: monitor.size().height as f64,
        })
        .collect::<Vec<_>>();
    let clicked_monitor =
        monitor_index_for_point(&monitor_bounds, click_position.x, click_position.y);
    let screens = monitors
        .iter()
        .enumerate()
        .map(|(index, monitor)| {
            format!(
                "screen[{index}] name={:?} pos=({}, {}) size={}x{} @{}x",
                monitor.name(),
                monitor.position().x,
                monitor.position().y,
                monitor.size().width,
                monitor.size().height,
                monitor.scale_factor()
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    crate::config::write_debug_log(
        &state,
        &format!(
            "tray_icon_click: button={button:?} mouse=({:.1}, {:.1}) screen={clicked_monitor:?} tray_rect={tray_rect:?}; {screens}",
            click_position.x, click_position.y
        ),
    );
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
    click_position: tauri::PhysicalPosition<f64>,
    state: Option<tauri::State<'_, crate::AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::PhysicalPosition;

    let dbg = |msg: &str| {
        if let Some(ref s) = state {
            crate::config::write_debug_log(s, msg);
        }
    };

    let monitors = window.available_monitors()?;
    let monitor_bounds = monitors
        .iter()
        .map(|monitor| PhysicalBounds {
            x: monitor.position().x as f64,
            y: monitor.position().y as f64,
            width: monitor.size().width as f64,
            height: monitor.size().height as f64,
        })
        .collect::<Vec<_>>();
    let target_idx = monitor_index_for_point(
        &monitor_bounds,
        click_position.x,
        click_position.y,
    );
    for (i, m) in monitors.iter().enumerate() {
        let pos = m.position();
        let size = m.size();
        let scale = m.scale_factor();
        dbg(&format!(
            "  monitor[{}]: pos=({},{}) size={}x{} scale={}",
            i, pos.x, pos.y, size.width, size.height, scale
        ));
    }

    let target = target_idx.map(|i| &monitors[i]);
    let target_scale = target
        .map(|m| m.scale_factor())
        .unwrap_or(window.scale_factor().unwrap_or(1.0));
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

    // Window size in target monitor's physical pixels.
    // Read actual width/height from outer_size (NOT hardcoded) so width changes
    // in tauri.conf.json don't desync the placement clamp. outer_size returns
    // physical pixels at the window's current scale, so divide by window_scale
    // to get logical, then multiply by target_scale to get target-physical.
    let win_w = window.outer_size()?.width as f64 * target_scale / window_scale;
    let win_h = window.outer_size()?.height as f64 * target_scale / window_scale;
    dbg(&format!(
        "tray x={} y={} w={} h={} | win_w={} win_h={}",
        tray_x, tray_y, tray_w, tray_h, win_w, win_h
    ));

    let target_bounds = target_idx
        .and_then(|index| monitor_bounds.get(index).copied())
        .ok_or("no monitor available for tray popup")?;

    let (x, y, vertical_placement) = place_popup_in_monitor(
        PhysicalBounds {
            x: tray_x,
            y: tray_y,
            width: tray_w,
            height: tray_h,
        },
        win_w,
        win_h,
        target_bounds,
        target_scale,
    );
    dbg(&format!(
        "placement: click=({:.1},{:.1}) target={target_idx:?} mode={vertical_placement:?} monitor=({},{}) {}x{} popup=({x},{y}) {win_w}x{win_h}",
        click_position.x,
        click_position.y,
        target_bounds.x,
        target_bounds.y,
        target_bounds.width,
        target_bounds.height,
    ));

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

/// Dispatches shell shortcuts after modifier release and all other shortcuts on key-down.
fn dispatch_shortcut_event(
    state: tauri_plugin_global_shortcut::ShortcutState,
    command: &str,
    dispatch: impl FnOnce(),
) {
    let dispatch_state = if command.starts_with("command/system/") {
        tauri_plugin_global_shortcut::ShortcutState::Released
    } else {
        tauri_plugin_global_shortcut::ShortcutState::Pressed
    };
    if state != dispatch_state {
        return;
    }

    dispatch();
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

        if let Ok(shortcut) = key.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            match app.global_shortcut().on_shortcut(
                shortcut,
                move |_app, _shortcut, event| {
                    for cmd in &commands {
                        dispatch_shortcut_event(event.state, cmd, || {
                            log::info!("shortcut triggered: '{}' → {}", key_for_log, cmd);
                            execute_command(&handle, cmd);
                        });
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
    lines.push("backend: in-process (display-dj-cli vendored)".to_string());

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

    // Live hardware probe — exercises the same code path the brightness slider
    // uses, so we can tell whether DDC enumerate/get/set succeed for each panel.
    // Mirrors the rich diagnostics the standalone display-dj-cli used to print.
    lines.push("--- live displays (core::display::list_all) ---".into());
    for d in crate::core::display::list_all() {
        lines.push(format!(
            "  id={} type={} ddc_supported={} brightness={:?} contrast={:?} name={:?}",
            d.id, d.display_type, d.ddc_supported, d.brightness, d.contrast, d.name
        ));
    }

    // Raw platform diagnostics — HMONITOR mapping, DDC enumerate result,
    // per-monitor VCP brightness/contrast (current+max), WMI brightness.
    // This is what lets us diagnose silent DDC failures on a specific panel.
    lines.push("--- platform debug_info ---".into());
    let platform_dbg = <crate::core::PlatformImpl as crate::core::Platform>::debug_info();
    match serde_json::to_string_pretty(&platform_dbg) {
        Ok(s) => lines.push(s),
        Err(e) => lines.push(format!("(serialize failed: {})", e)),
    }

    lines.push("=== END DEBUG INFO ===".into());
    let output = lines.join("\n");
    log::info!("{}", output);

    // Also write to debug log file regardless of debug_logging preference
    if let Some(state) = app.try_state::<crate::AppState>() {
        crate::config::write_debug_log(&state, &output);
    }
}

/// Dispatches a command string (e.g. "command/changeBrightness/50") to the
/// appropriate in-process platform call. Used by keyboard shortcuts, profiles,
/// tray menu actions, and the night mode schedule.
pub(crate) fn execute_command(app: &AppHandle, command: &str) {
    log::info!("execute_command: '{}'", command);
    let parts: Vec<&str> = command.split('/').collect();
    match parts.as_slice() {
        // Set brightness for all monitors: command/changeBrightness/{value}
        //
        // Honors the per-monitor `brightnessMode` preference: monitors set to
        // "overlay" go through the soft-overlay path; "ddc"/"gamma" use that
        // single hardware path; "auto" tries hardware then falls back to the
        // overlay. See `display::set_all_brightness` for the full dispatcher.
        ["command", "changeBrightness", value] => {
            if let Ok(val) = value.parse::<u32>() {
                let (min, configs, cached) = {
                    let state = app.state::<crate::AppState>();
                    let prefs = state.preferences.lock();
                    let min = prefs
                        .as_ref()
                        .map(|p| p.effective_min_brightness())
                        .unwrap_or(crate::config::ABSOLUTE_MIN_BRIGHTNESS);
                    let configs = prefs
                        .as_ref()
                        .map(|p| p.monitor_configs.clone())
                        .unwrap_or_default();
                    let cached = state.sidecar_cache.get_monitors().unwrap_or_default();
                    (min, configs, cached)
                };
                let clamped = val.clamp(min, 100);
                if let Some(state) = app.try_state::<crate::AppState>() {
                    state.sidecar_cache.invalidate_monitors();
                }
                let app_clone = app.clone();
                run_then_emit(app.clone(), "monitors-changed", move || {
                    dispatch_brightness_for_all(&app_clone, clamped, &configs, &cached);
                });
            }
        }
        // Set brightness for a single monitor: command/changeBrightness/{monitor_id}/{value}
        ["command", "changeBrightness", monitor_id, value] => {
            if let Ok(val) = value.parse::<u32>() {
                let (min, mode, monitor_rect) = {
                    let state = app.state::<crate::AppState>();
                    let prefs = state.preferences.lock();
                    let min = prefs
                        .as_ref()
                        .map(|p| p.effective_min_brightness())
                        .unwrap_or(crate::config::ABSOLUTE_MIN_BRIGHTNESS);
                    let mode = prefs
                        .as_ref()
                        .map(|p| {
                            crate::display::resolve_brightness_mode(
                                &p.monitor_configs, monitor_id,
                            )
                        })
                        .unwrap_or_else(|_| "auto".into());
                    let monitor_rect = state
                        .sidecar_cache
                        .get_monitors()
                        .and_then(|monitors| {
                            monitors
                                .into_iter()
                                .find(|m| m.id == *monitor_id)
                                .and_then(|m| m.monitor_rect)
                        });
                    (min, mode, monitor_rect)
                };
                let clamped = val.clamp(min, 100);
                let id = monitor_id.to_string();
                if let Some(state) = app.try_state::<crate::AppState>() {
                    state.sidecar_cache.invalidate_monitors();
                }
                let app_clone = app.clone();
                run_then_emit(app.clone(), "monitors-changed", move || {
                    dispatch_brightness_for_one(&app_clone, &id, clamped, &mode, monitor_rect);
                });
            }
        }
        ["command", "changeDarkMode", mode] => {
            match *mode {
                "toggle" => {
                    // For toggle, read current state first via the in-process platform layer.
                    let app_clone = app.clone();
                    std::thread::spawn(move || {
                        let is_dark = crate::core::theme::get_dark_mode().unwrap_or(false);
                        let _ = crate::core::theme::set_dark_mode(!is_dark);
                        crate::tray_icon::set_dark_mode_state(&app_clone, !is_dark);
                        if let Some(state) = app_clone.try_state::<crate::AppState>() {
                            state.sidecar_cache.invalidate_dark_mode();
                        }
                        let _ = app_clone.emit("dark-mode-changed", ());
                    });
                }
                "dark" => {
                    crate::tray_icon::set_dark_mode_state(app, true);
                    if let Some(state) = app.try_state::<crate::AppState>() {
                        state.sidecar_cache.invalidate_dark_mode();
                    }
                    run_then_emit(app.clone(), "dark-mode-changed", || {
                        let _ = crate::core::theme::set_dark_mode(true);
                    });
                }
                "light" => {
                    crate::tray_icon::set_dark_mode_state(app, false);
                    if let Some(state) = app.try_state::<crate::AppState>() {
                        state.sidecar_cache.invalidate_dark_mode();
                    }
                    run_then_emit(app.clone(), "dark-mode-changed", || {
                        let _ = crate::core::theme::set_dark_mode(false);
                    });
                }
                _ => {}
            }
        }
        // Set contrast for all monitors: command/changeContrast/{value}
        ["command", "changeContrast", value] => {
            if let Ok(val) = value.parse::<u32>() {
                let clamped = val.min(100);
                if let Some(state) = app.try_state::<crate::AppState>() {
                    state.sidecar_cache.invalidate_monitors();
                }
                run_then_emit(app.clone(), "monitors-changed", move || {
                    let _ = crate::core::display::set_all_contrast(clamped as u16);
                });
            }
        }
        // Set contrast for a single monitor: command/changeContrast/{monitor_id}/{value}
        ["command", "changeContrast", monitor_id, value] => {
            if let Ok(val) = value.parse::<u32>() {
                let clamped = val.min(100);
                let id = monitor_id.to_string();
                if let Some(state) = app.try_state::<crate::AppState>() {
                    state.sidecar_cache.invalidate_monitors();
                }
                run_then_emit(app.clone(), "monitors-changed", move || {
                    let _ = crate::core::display::set_one_contrast(&id, clamped as u16);
                });
            }
        }
        ["command", "changeVolume", value] => {
            if let Ok(val) = value.parse::<u32>() {
                let clamped = val.min(100);
                crate::tray_icon::set_muted_state(app, clamped == 0);
                if let Some(state) = app.try_state::<crate::AppState>() {
                    state.sidecar_cache.invalidate_volume();
                }
                run_then_emit(app.clone(), "volume-changed", move || {
                    let _ = crate::core::volume::set_volume(clamped as u16);
                });
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
        ["command", "system", "taskView"] => {
            if let Err(error) = crate::core::task_view::open() {
                log::warn!("task_view: {error}");
            }
        }
        ["command", "system", "showDesktop"] => {
            if let Err(error) = crate::core::task_view::show_desktop() {
                log::warn!("show_desktop: {error}");
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
        // Z-order control: command/window/{moveToFront,moveToBack},
        // command/app/{moveToFront,moveToBack}.
        // Wildcard match — the parser is the source of truth for valid actions.
        ["command", "window", _] | ["command", "app", _] => {
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            {
                if let Some(action) = crate::tiling::parse_zorder_command(command) {
                    let app_clone = app.clone();
                    std::thread::spawn(move || {
                        crate::tiling::execute_zorder(&app_clone, action);
                    });
                } else {
                    log::warn!("Unknown z-order command: {}", command);
                }
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            log::warn!("Z-order is not supported on this platform: {}", command);
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
        // Set wallpaper on single monitor: command/wallpaper/change_single/{monitor}/{path}
        // or command/wallpaper/change_single/{monitor}/{fit}/{path}
        ["command", "wallpaper", "change_single", ..] => {
            let prefix = "command/wallpaper/change_single/";
            if command.len() > prefix.len() {
                let remainder = &command[prefix.len()..];
                // First segment is the monitor query, rest is [fit/]path
                if let Some(slash_pos) = remainder.find('/') {
                    let monitor_query = &remainder[..slash_pos];
                    let after_monitor = &remainder[slash_pos + 1..];
                    let (fit, path) = crate::wallpaper::parse_wallpaper_args(after_monitor);
                    let app_clone = app.clone();
                    let monitor_owned = monitor_query.to_string();
                    let path_owned = path.to_string();
                    let fit_owned = fit.map(|f| f.to_string());
                    std::thread::spawn(move || {
                        let state = app_clone.state::<crate::AppState>();
                        crate::wallpaper::change_wallpaper_single(
                            &state,
                            &monitor_owned,
                            &path_owned,
                            fit_owned.as_deref(),
                        );
                    });
                } else {
                    log::warn!("wallpaper change_single command missing path: {}", command);
                }
            } else {
                log::warn!("wallpaper change_single command missing monitor and path: {}", command);
            }
        }
        // Start slideshow: command/wallpaper/slideshow/{path}
        // or command/wallpaper/slideshow/{interval}/{order}/{path}
        ["command", "wallpaper", "slideshow", ..] => {
            let prefix = "command/wallpaper/slideshow/";
            if command.len() > prefix.len() {
                let remainder = &command[prefix.len()..];
                let (interval, order, path) = crate::wallpaper::parse_slideshow_args(remainder);
                let app_clone = app.clone();
                let path_owned = path.to_string();
                let order_owned = order.map(|o| o.to_string());
                std::thread::spawn(move || {
                    let state = app_clone.state::<crate::AppState>();
                    crate::wallpaper::start_slideshow(
                        &state,
                        &path_owned,
                        interval,
                        order_owned.as_deref(),
                    );
                });
            } else {
                log::warn!("wallpaper slideshow command missing folder path: {}", command);
            }
        }
        // Stop slideshow: command/wallpaper/slideshow_stop
        ["command", "wallpaper", "slideshow_stop"] => {
            let app_clone = app.clone();
            std::thread::spawn(move || {
                let state = app_clone.state::<crate::AppState>();
                crate::wallpaper::stop_slideshow(&state);
            });
        }
        // Remote slideshow: command/wallpaper/slideshow_remote/{url_to_zip}
        ["command", "wallpaper", "slideshow_remote", ..] => {
            let prefix = "command/wallpaper/slideshow_remote/";
            if command.len() > prefix.len() {
                let url = &command[prefix.len()..];
                let app_clone = app.clone();
                let url_owned = url.to_string();
                std::thread::spawn(move || {
                    let state = app_clone.state::<crate::AppState>();
                    crate::wallpaper::download_and_start_remote_slideshow(&state, &url_owned);
                });
            } else {
                log::warn!("wallpaper slideshow_remote command missing URL: {}", command);
            }
        }
        _ => {
            log::warn!("Unknown command: {}", command);
        }
    }
}

/// Historically built a sidecar HTTP URL for a given command string. Now that
/// every command dispatches in-process via `execute_command`, this function
/// always returns `None`. Kept for backward compatibility with tests and as
/// documentation that no command maps to an external URL anymore.
#[cfg(test)]
fn build_command_url(_command: &str, _base: &str, _min_brightness: u32) -> Option<String> {
    None
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

    /// All commands now dispatch in-process — there is no sidecar URL to build,
    /// so `build_command_url` returns `None` for every input. The test sweep
    /// covers the formerly-routed commands (brightness, contrast, volume) as
    /// well as commands that always returned None (wallpaper, z-order, tile,
    /// profiles, dark mode toggle, unknown commands).
    #[test]
    fn test_build_url_always_returns_none() {
        let inputs = [
            "command/changeBrightness/75",
            "command/changeBrightness/1/80",
            "command/changeBrightness/builtin/50",
            "command/changeBrightness/3",
            "command/changeBrightness/1/3",
            "command/changeContrast/60",
            "command/changeContrast/2/70",
            "command/changeContrast/150",
            "command/changeVolume/50",
            "command/changeBrightness/abc",
            "command/changeBrightness/1/abc",
            "command/unknown/123",
            "command/wallpaper/change//Users/pic.jpg",
            "command/wallpaper/change/center//Users/pic.jpg",
            "command/window/moveToFront",
            "command/app/moveToFront",
            "command/window/moveToBack",
            "command/app/moveToBack",
            "command/window/toggleFrontBack",
            "command/app/toggleFrontBack",
            "command/tile/leftHalf",
            "command/changeProfile/0",
            "command/changeDarkMode/toggle",
        ];
        for cmd in inputs {
            assert_eq!(
                build_command_url(cmd, BASE, 10),
                None,
                "expected None for: {}",
                cmd,
            );
        }
    }

    /// Normal shortcuts dispatch on press and ignore release.
    #[test]
    fn normal_shortcut_dispatches_on_press_only() {
        let mut dispatch_count = 0;

        dispatch_shortcut_event(
            tauri_plugin_global_shortcut::ShortcutState::Pressed,
            "command/tile/leftHalf",
            || {
                dispatch_count += 1;
            },
        );
        dispatch_shortcut_event(
            tauri_plugin_global_shortcut::ShortcutState::Released,
            "command/tile/leftHalf",
            || {
                dispatch_count += 1;
            },
        );

        assert_eq!(dispatch_count, 1);
    }

    /// System shortcuts wait for release so held modifiers cannot alter the OS action.
    #[test]
    fn system_shortcut_dispatches_on_release_only() {
        let mut dispatch_count = 0;

        dispatch_shortcut_event(
            tauri_plugin_global_shortcut::ShortcutState::Pressed,
            "command/system/showDesktop",
            || {
                dispatch_count += 1;
            },
        );
        dispatch_shortcut_event(
            tauri_plugin_global_shortcut::ShortcutState::Released,
            "command/system/showDesktop",
            || {
                dispatch_count += 1;
            },
        );

        assert_eq!(dispatch_count, 1);
    }

    /// Both platform shell actions use release-time dispatch.
    #[test]
    fn task_view_shortcut_dispatches_on_release_only() {
        let mut dispatch_count = 0;

        dispatch_shortcut_event(
            tauri_plugin_global_shortcut::ShortcutState::Pressed,
            "command/system/taskView",
            || {
                dispatch_count += 1;
            },
        );
        dispatch_shortcut_event(
            tauri_plugin_global_shortcut::ShortcutState::Released,
            "command/system/taskView",
            || {
            dispatch_count += 1;
            },
        );

        assert_eq!(dispatch_count, 1);
    }

    /// Platform endpoint IDs survive the menu-safe encoding without loss.
    #[test]
    fn audio_output_menu_id_round_trips_platform_identifier() {
        let device_id = r#"{0.0.0.00000000}.{speaker-guid}\BuiltIn"#;
        let menu_id = audio_output_menu_id(device_id);

        assert_eq!(parse_audio_output_menu_id(&menu_id).as_deref(), Some(device_id));
        assert!(parse_audio_output_menu_id("audio_output_not-hex").is_none());
    }

    /// Speaker tray presets route through the existing volume command dispatcher.
    #[test]
    fn speaker_menu_presets_map_to_volume_commands() {
        assert_eq!(
            sound_menu_command(SOUND_MUTE_MENU_ID),
            Some("command/changeVolume/0")
        );
        assert_eq!(
            sound_menu_command(SOUND_HALF_MENU_ID),
            Some("command/changeVolume/50")
        );
        assert_eq!(
            sound_menu_command(SOUND_FULL_MENU_ID),
            Some("command/changeVolume/100")
        );
        assert_eq!(sound_menu_command("sound_volume_unknown"), None);
    }

    /// Speakers submenu keeps output selection above a separator and volume presets below it.
    #[test]
    fn speakers_submenu_contains_output_and_volume_sections() {
        let source = include_str!("tray.rs");
        let builder = source
            .split("let mut audio_output_submenu = SubmenuBuilder::new(app, \"Speakers\")")
            .nth(1)
            .expect("Speakers submenu builder must exist")
            .split("let audio_output_submenu = audio_output_submenu.build()")
            .next()
            .expect("Speakers submenu builder must have a bounded body");

        assert!(builder.contains("audio_output_menu_entries"));
        assert!(builder.contains(".separator()"));
        assert!(builder.contains("SOUND_MUTE_MENU_ID, \"Mute\""));
        assert!(builder.contains("SOUND_HALF_MENU_ID, \"50%\""));
        assert!(builder.contains("SOUND_FULL_MENU_ID, \"100%\""));
    }

    /// Tray choices include only enabled outputs and mark the current endpoint.
    #[test]
    fn audio_output_menu_entries_filter_and_mark_devices() {
        use crate::core::audio_output::{
            AudioOutputDevice, AudioOutputDeviceState, AudioOutputState,
        };

        let output_state = AudioOutputState {
            devices: vec![
                AudioOutputDevice {
                    id: "built-in".into(),
                    name: "MacBook Pro Speakers".into(),
                    original_name: "MacBook Pro Speakers".into(),
                    state: AudioOutputDeviceState::Enabled,
                    is_built_in: true,
                },
                AudioOutputDevice {
                    id: "dock".into(),
                    name: "Dock".into(),
                    original_name: "Dock".into(),
                    state: AudioOutputDeviceState::Enabled,
                    is_built_in: false,
                },
                AudioOutputDevice {
                    id: "teams".into(),
                    name: "Microsoft Teams Audio".into(),
                    original_name: "Microsoft Teams Audio".into(),
                    state: AudioOutputDeviceState::Hidden,
                    is_built_in: false,
                },
            ],
            selected_device_id: Some("dock".into()),
        };

        let entries = audio_output_menu_entries(Some(&output_state));

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].1, "   MacBook Pro Speakers");
        assert_eq!(entries[1].1, "● Dock");
        assert_eq!(
            parse_audio_output_menu_id(&entries[1].0).as_deref(),
            Some("dock")
        );
    }

    /// Popup placement uses the clicked tray rectangle so a secondary macOS
    /// menu bar cannot be replaced by a later primary-menu-bar lookup.
    #[test]
    fn tray_click_positions_popup_from_event_rectangle() {
        let source = include_str!("tray.rs");
        let handler = source
            .split(".on_tray_icon_event")
            .nth(1)
            .expect("tray event handler must exist")
            .split(".build(app)")
            .next()
            .expect("tray event handler must have a bounded body");

        assert!(handler.contains("rect: tray_rect"));
        assert!(handler.contains("position: click_position"));
        assert!(handler.contains("*stored = Some(tray_rect)"));
        assert!(handler.contains("*stored = Some(click_position)"));
        assert!(handler.contains("if button_state != MouseButtonState::Up"));
        assert!(handler.contains("if button != MouseButton::Left"));
        assert!(handler.contains("show_popup_window(app)"));
        assert!(!handler.contains("if let Ok(Some(tray_rect)) = tray.rect()"));
    }

    /// Context-menu Show Window and left-click opening share the same placement seam.
    #[test]
    fn show_window_menu_reuses_latest_tray_click_placement() {
        let source = include_str!("tray.rs");
        let show_popup = source
            .split("fn show_popup_window")
            .nth(1)
            .expect("shared popup helper must exist")
            .split("/// Reuses the latest")
            .next()
            .expect("shared popup helper must have a bounded body");
        let menu_handler = source
            .split(".on_menu_event")
            .nth(1)
            .expect("menu event handler must exist")
            .split(".on_tray_icon_event")
            .next()
            .expect("menu event handler must have a bounded body");

        assert!(show_popup.contains("position_popup_from_last_tray_click(app, &window)"));
        assert!(menu_handler.contains("\"show_window\" => {\n                show_popup_window(app);"));
    }

    /// Right-button down stores a fresh anchor before macOS opens its synchronous context menu.
    #[test]
    fn right_click_anchor_is_stored_before_button_up_filter() {
        let source = include_str!("tray.rs");
        let handler = source
            .split(".on_tray_icon_event")
            .nth(1)
            .expect("tray icon event handler must exist")
            .split(".build(app)?")
            .next()
            .expect("tray icon event handler must have a bounded body");
        let store_index = handler
            .find("*stored = Some(click_position)")
            .expect("tray click position must be stored");
        let up_filter_index = handler
            .find("if button_state != MouseButtonState::Up")
            .expect("button-up action filter must exist");

        assert!(store_index < up_filter_index);
    }

    /// A click on the lower screen in a vertical stack selects that screen.
    #[test]
    fn tray_click_selects_lower_stacked_monitor() {
        let monitors = [
            PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            PhysicalBounds {
                x: 0.0,
                y: 1080.0,
                width: 1920.0,
                height: 1080.0,
            },
        ];

        assert_eq!(monitor_index_for_point(&monitors, 1500.0, 1095.0), Some(1));
    }

    /// Overlapping mixed-DPI bounds select the display whose tray edge is nearest the click.
    #[test]
    fn tray_click_disambiguates_live_mixed_dpi_stack() {
        let monitors = [
            PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width: 3456.0,
                height: 2234.0,
            },
            PhysicalBounds {
                x: -63.0,
                y: 1117.0,
                width: 1920.0,
                height: 1200.0,
            },
        ];

        assert_eq!(monitor_index_for_point(&monitors, 1303.7, 1132.5), Some(1));
        assert_eq!(monitor_index_for_point(&monitors, 2331.9, 26.7), Some(0));
    }

    /// Overlapping bounds also resolve side-mounted trays by their shallow edge distance.
    #[test]
    fn tray_click_disambiguates_overlapping_side_edges() {
        let monitors = [
            PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width: 3840.0,
                height: 2160.0,
            },
            PhysicalBounds {
                x: 1900.0,
                y: 100.0,
                width: 1800.0,
                height: 1080.0,
            },
        ];

        assert_eq!(monitor_index_for_point(&monitors, 3690.0, 600.0), Some(1));
        assert_eq!(monitor_index_for_point(&monitors, 10.0, 600.0), Some(0));
    }

    /// Negative origins remain valid when a secondary display sits left and below primary.
    #[test]
    fn tray_click_selects_negative_origin_monitor() {
        let monitors = [
            PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width: 2560.0,
                height: 1600.0,
            },
            PhysicalBounds {
                x: -1920.0,
                y: 1600.0,
                width: 1920.0,
                height: 1080.0,
            },
        ];

        assert_eq!(monitor_index_for_point(&monitors, -50.0, 1610.0), Some(1));
    }

    /// A point between display bounds selects the nearest display deterministically.
    #[test]
    fn tray_click_in_gap_selects_nearest_monitor() {
        let monitors = [
            PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 1000.0,
            },
            PhysicalBounds {
                x: 0.0,
                y: 1200.0,
                width: 1000.0,
                height: 1000.0,
            },
        ];

        assert_eq!(monitor_index_for_point(&monitors, 500.0, 1150.0), Some(1));
    }

    /// Exact overlap ties remain stable by selecting the first enumerated display.
    #[test]
    fn tray_click_overlap_tie_is_stable() {
        let monitors = [
            PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
        ];

        assert_eq!(monitor_index_for_point(&monitors, 500.0, 10.0), Some(0));
    }

    /// Tray click diagnostics include both buttons, DPI notation, and the physical mouse point.
    #[test]
    fn tray_click_diagnostics_include_button_mouse_and_dpi() {
        let source = include_str!("tray.rs");
        let logger = source
            .split("fn log_tray_click")
            .nth(1)
            .expect("tray click logger must exist")
            .split("/// Position the popup")
            .next()
            .expect("tray click logger must have a bounded body");

        assert!(logger.contains("button={button:?}"));
        assert!(logger.contains("mouse=({:.1}, {:.1})"));
        assert!(logger.contains("screen={clicked_monitor:?}"));
        assert!(logger.contains("@{}x"));
    }

    /// Top-edge trays place the popup below while all corners stay on-screen.
    #[test]
    fn tray_popup_stays_below_top_tray() {
        let monitor = PhysicalBounds {
            x: 0.0,
            y: 1080.0,
            width: 1920.0,
            height: 1080.0,
        };
        let tray = PhysicalBounds {
            x: 1500.0,
            y: 1080.0,
            width: 24.0,
            height: 24.0,
        };

        let (x, y, placement) = place_popup_in_monitor(tray, 420.0, 700.0, monitor, 1.0);

        assert_eq!(placement, PopupVerticalPlacement::Below);
        assert_eq!(y, 1108.0);
        assert!(x >= monitor.x + POPUP_MARGIN_LOGICAL);
        assert!(x + 420.0 <= monitor.x + monitor.width - POPUP_MARGIN_LOGICAL);
        assert!(y + 700.0 <= monitor.y + monitor.height - POPUP_MARGIN_LOGICAL);
    }

    /// Bottom-edge trays flip the complete popup above the clicked tray.
    #[test]
    fn tray_popup_flips_above_bottom_tray() {
        let monitor = PhysicalBounds {
            x: 0.0,
            y: 1080.0,
            width: 1920.0,
            height: 1080.0,
        };
        let tray = PhysicalBounds {
            x: 1500.0,
            y: 2130.0,
            width: 24.0,
            height: 24.0,
        };

        let (_, y, placement) = place_popup_in_monitor(tray, 420.0, 700.0, monitor, 1.0);

        assert_eq!(placement, PopupVerticalPlacement::Above);
        assert_eq!(y, 1426.0);
        assert!(y >= monitor.y + POPUP_MARGIN_LOGICAL);
        assert!(y + 700.0 <= monitor.y + monitor.height - POPUP_MARGIN_LOGICAL);
    }

    /// Right-edge tray clicks shift the whole popup left onto the same monitor.
    #[test]
    fn tray_popup_clamps_horizontal_overflow() {
        let monitor = PhysicalBounds {
            x: 1920.0,
            y: 0.0,
            width: 1280.0,
            height: 1024.0,
        };
        let tray = PhysicalBounds {
            x: 3175.0,
            y: 0.0,
            width: 24.0,
            height: 24.0,
        };

        let (x, _, _) = place_popup_in_monitor(tray, 420.0, 700.0, monitor, 1.0);

        assert_eq!(x, 2772.0);
        assert!(x + 420.0 <= monitor.x + monitor.width - POPUP_MARGIN_LOGICAL);
    }

    /// Left-edge trays shift the popup right while preserving below-first placement.
    #[test]
    fn tray_popup_stays_inside_left_edge() {
        let monitor = PhysicalBounds {
            x: -1920.0,
            y: 200.0,
            width: 1920.0,
            height: 1080.0,
        };
        let tray = PhysicalBounds {
            x: -1920.0,
            y: 500.0,
            width: 30.0,
            height: 30.0,
        };

        let (x, y, placement) = place_popup_in_monitor(tray, 420.0, 600.0, monitor, 1.0);

        assert_eq!(placement, PopupVerticalPlacement::Below);
        assert_eq!(x, -1912.0);
        assert_eq!(y, 534.0);
        assert_popup_fits(x, y, 420.0, 600.0, monitor, 1.0);
    }

    /// Right-edge trays shift the popup left while preserving below-first placement.
    #[test]
    fn tray_popup_stays_inside_right_edge() {
        let monitor = PhysicalBounds {
            x: 1920.0,
            y: 200.0,
            width: 1920.0,
            height: 1080.0,
        };
        let tray = PhysicalBounds {
            x: 3810.0,
            y: 500.0,
            width: 30.0,
            height: 30.0,
        };

        let (x, y, placement) = place_popup_in_monitor(tray, 420.0, 600.0, monitor, 1.0);

        assert_eq!(placement, PopupVerticalPlacement::Below);
        assert_eq!(x, 3412.0);
        assert_eq!(y, 534.0);
        assert_popup_fits(x, y, 420.0, 600.0, monitor, 1.0);
    }

    /// Every tray corner keeps all popup corners inside the same display.
    #[test]
    fn tray_popup_stays_inside_all_monitor_corners() {
        let monitor = PhysicalBounds {
            x: -500.0,
            y: 300.0,
            width: 1600.0,
            height: 1000.0,
        };
        let trays = [
            PhysicalBounds {
                x: -500.0,
                y: 300.0,
                width: 30.0,
                height: 30.0,
            },
            PhysicalBounds {
                x: 1070.0,
                y: 300.0,
                width: 30.0,
                height: 30.0,
            },
            PhysicalBounds {
                x: -500.0,
                y: 1270.0,
                width: 30.0,
                height: 30.0,
            },
            PhysicalBounds {
                x: 1070.0,
                y: 1270.0,
                width: 30.0,
                height: 30.0,
            },
        ];

        for tray in trays {
            let (x, y, _) = place_popup_in_monitor(tray, 420.0, 600.0, monitor, 1.0);
            assert_popup_fits(x, y, 420.0, 600.0, monitor, 1.0);
        }
    }

    /// Asserts every popup edge remains within the monitor's scaled safety margin.
    fn assert_popup_fits(
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        monitor: PhysicalBounds,
        scale: f64,
    ) {
        let margin = POPUP_MARGIN_LOGICAL * scale;
        assert!(x >= monitor.x + margin);
        assert!(y >= monitor.y + margin);
        assert!(x + width <= monitor.x + monitor.width - margin);
        assert!(y + height <= monitor.y + monitor.height - margin);
    }

    /// A popup taller than the usable display aligns to the top margin instead of mislabeling below.
    #[test]
    fn tray_popup_clamps_oversized_height_to_top_margin() {
        let monitor = PhysicalBounds {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 600.0,
        };
        let tray = PhysicalBounds {
            x: 900.0,
            y: 0.0,
            width: 24.0,
            height: 24.0,
        };

        let (_, y, placement) = place_popup_in_monitor(tray, 420.0, 700.0, monitor, 1.0);

        assert_eq!(placement, PopupVerticalPlacement::Clamped);
        assert_eq!(y, POPUP_MARGIN_LOGICAL);
    }

    /// Retina scaling applies logical gap and margin values in physical pixels.
    #[test]
    fn tray_popup_scales_gap_and_margin_for_retina_display() {
        let monitor = PhysicalBounds {
            x: 0.0,
            y: 0.0,
            width: 3840.0,
            height: 2160.0,
        };
        let tray = PhysicalBounds {
            x: 3800.0,
            y: 0.0,
            width: 40.0,
            height: 48.0,
        };

        let (x, y, placement) = place_popup_in_monitor(tray, 840.0, 1400.0, monitor, 2.0);

        assert_eq!(placement, PopupVerticalPlacement::Below);
        assert_eq!(x, 2984.0);
        assert_eq!(y, 56.0);
    }

    /// Windows shell shortcuts release trigger modifiers before balanced Win-key events.
    #[test]
    fn platform_shell_shortcuts_use_native_actions() {
        let source = include_str!("core/task_view.rs");
        let function = source
            .split("fn send_windows_chord")
            .nth(1)
            .expect("Windows shell input helper must exist")
            .split("/// Open Windows Task View")
            .next()
            .expect("Windows shell input helper must have a bounded body");
        let expected_order = [
            "key_input(VK_CONTROL, KEYEVENTF_KEYUP)",
            "key_input(VK_MENU, KEYEVENTF_KEYUP)",
            "key_input(VK_SHIFT, KEYEVENTF_KEYUP)",
            "key_input(VK_LWIN, Default::default())",
            "key_input(key, Default::default())",
            "key_input(key, KEYEVENTF_KEYUP)",
            "key_input(VK_LWIN, KEYEVENTF_KEYUP)",
        ];

        let mut remainder = function;
        for event in expected_order {
            remainder = remainder
                .split_once(event)
                .unwrap_or_else(|| panic!("missing or out-of-order shell shortcut event: {event}"))
                .1;
        }
        assert!(source.contains("send_windows_chord(VK_TAB)"));
        assert!(source.contains("send_windows_chord(VK_D)"));
        assert!(source.contains("MACOS_MISSION_CONTROL_ACTION: &str = \"0\""));
        assert!(source.contains("MACOS_SHOW_DESKTOP_ACTION: &str = \"2\""));
        assert!(source.contains(".spawn()"));
        assert!(source.contains("child.wait()"));
        assert!(!source.contains(".status()"));
    }
}
