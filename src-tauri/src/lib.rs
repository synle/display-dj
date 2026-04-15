mod config;
mod dark_mode;
mod display;
mod keep_awake;
mod tray;
mod volume;

use std::sync::atomic::{AtomicU16, Ordering};
use chrono::Timelike;
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;

static SERVER_PORT: AtomicU16 = AtomicU16::new(51337);

/// Returns the current display-dj sidecar HTTP server port.
pub fn server_port() -> u16 {
    SERVER_PORT.load(Ordering::Relaxed)
}

pub struct AppState {
    pub preferences: std::sync::Mutex<config::Preferences>,
    pub last_tray_rect: std::sync::Mutex<Option<tauri::Rect>>,
    pub sidecar_child: std::sync::Mutex<Option<CommandChild>>,
    /// True while we're waiting for the window to gain focus after being shown.
    /// Blocks spurious `Focused(false)` events that fire before `Focused(true)`
    /// (a known issue on Linux/X11 and some Windows configurations).
    /// Cleared when `Focused(true)` fires, so subsequent focus-loss auto-hides normally.
    pub expect_focus_gain: std::sync::Mutex<bool>,
    /// Holds the active keep-awake guard. When `Some`, the system is prevented
    /// from sleeping. Dropping the guard (setting to `None`) releases the assertion.
    pub keep_awake: std::sync::Mutex<Option<keepawake::KeepAwake>>,
}

/// Find an available port starting from the default.
fn find_available_port(start: u16) -> u16 {
    for port in start..start + 100 {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    start
}

/// Fetch the `/debug` endpoint from the sidecar and prepend to the debug log.
fn fetch_debug_on_startup(port: u16) {
    let url = format!("http://127.0.0.1:{}/debug", port);
    match reqwest::blocking::get(&url) {
        Ok(resp) => {
            if let Ok(body) = resp.text() {
                let path = config::debug_log_path();
                let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
                let header = format!(
                    "[{}] === display-dj-cli debug dump (startup) ===\n{}\n[{}] === end debug dump ===\n",
                    timestamp, body, timestamp
                );
                // Prepend to existing log content
                let existing = std::fs::read_to_string(&path).unwrap_or_default();
                std::fs::write(&path, format!("{}{}", header, existing)).ok();
            }
        }
        Err(e) => {
            log::warn!("Failed to fetch /debug endpoint: {}", e);
        }
    }
}

/// Wait for the display-dj server to become ready.
fn wait_for_server(port: u16) {
    let url = format!("http://127.0.0.1:{}/health", port);
    for _ in 0..50 {
        if let Ok(resp) = reqwest::blocking::get(&url) {
            if resp.status().is_success() {
                log::info!("display-dj server ready on port {}", port);
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    log::warn!("display-dj server did not become ready on port {} within 5s", port);
}

/// Parse "HH:MM" into minutes since midnight.
fn parse_time_minutes(time_str: &str) -> Option<u32> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() == 2 {
        let h = parts[0].parse::<u32>().ok()?;
        let m = parts[1].parse::<u32>().ok()?;
        Some(h * 60 + m)
    } else {
        None
    }
}

/// Check if current time is in the "night" window.
fn is_night_time(night_start: u32, day_start: u32, now: u32) -> bool {
    if night_start <= day_start {
        // No midnight wrap: e.g. night=02:00 day=08:00 — night is [02:00, 08:00)
        now >= night_start && now < day_start
    } else {
        // Wraps midnight: e.g. night=21:00 day=07:00 — night is [21:00, 24:00) ∪ [00:00, 07:00)
        now >= night_start || now < day_start
    }
}

/// Check if the current time falls within the night mode schedule and apply
/// the corresponding brightness and dark/light mode via the sidecar.
fn check_night_mode_schedule(app: &tauri::AppHandle) {
    let schedule = {
        let state = app.state::<AppState>();
        let prefs = match state.preferences.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        prefs.night_mode_schedule.clone()
    };

    if !schedule.enabled {
        return;
    }

    let night_start = match parse_time_minutes(&schedule.night_start) {
        Some(m) => m,
        None => return,
    };
    let day_start = match parse_time_minutes(&schedule.day_start) {
        Some(m) => m,
        None => return,
    };

    let now_local = chrono::Local::now();
    let now_minutes = now_local.hour() * 60 + now_local.minute();

    let base = format!("http://127.0.0.1:{}", server_port());
    let is_night = is_night_time(night_start, day_start, now_minutes);

    let (brightness, dark_mode_route) = if is_night {
        (schedule.night_brightness, "dark")
    } else {
        (schedule.day_brightness, "light")
    };

    let min_brightness = {
        let state = app.state::<AppState>();
        state
            .preferences
            .lock()
            .map(|p| p.effective_min_brightness())
            .unwrap_or(config::ABSOLUTE_MIN_BRIGHTNESS)
    };

    let brightness = brightness.clamp(min_brightness, 100);

    let _ = reqwest::blocking::get(format!("{}/set_all/{}", base, brightness));
    let _ = reqwest::blocking::get(format!("{}/{}", base, dark_mode_route));

    use tauri::Emitter;
    let _ = app.emit("monitors-changed", ());
    let _ = app.emit("dark-mode-changed", ());
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_time_minutes --

    #[test]
    fn test_parse_time_minutes_valid() {
        assert_eq!(parse_time_minutes("00:00"), Some(0));
        assert_eq!(parse_time_minutes("07:00"), Some(420));
        assert_eq!(parse_time_minutes("12:30"), Some(750));
        assert_eq!(parse_time_minutes("21:00"), Some(1260));
        assert_eq!(parse_time_minutes("23:59"), Some(1439));
    }

    #[test]
    fn test_parse_time_minutes_single_digits() {
        assert_eq!(parse_time_minutes("0:0"), Some(0));
        assert_eq!(parse_time_minutes("9:5"), Some(545));
    }

    #[test]
    fn test_parse_time_minutes_invalid() {
        assert_eq!(parse_time_minutes(""), None);
        assert_eq!(parse_time_minutes("invalid"), None);
        assert_eq!(parse_time_minutes("12"), None);
        assert_eq!(parse_time_minutes("12:ab"), None);
        assert_eq!(parse_time_minutes("ab:30"), None);
        assert_eq!(parse_time_minutes("12:30:00"), None);
    }

    // -- is_night_time --

    #[test]
    fn test_is_night_time_wraps_midnight() {
        // Typical schedule: night=21:00 (1260), day=07:00 (420)
        let night = 1260;
        let day = 420;

        // Night window: [21:00, 07:00)
        assert!(is_night_time(night, day, 1260));  // exactly 21:00 — night starts
        assert!(is_night_time(night, day, 1400));  // 23:20 — late night
        assert!(is_night_time(night, day, 0));      // 00:00 — midnight
        assert!(is_night_time(night, day, 300));    // 05:00 — early morning
        assert!(is_night_time(night, day, 419));    // 06:59 — still night

        // Day window: [07:00, 21:00)
        assert!(!is_night_time(night, day, 420));   // exactly 07:00 — day starts
        assert!(!is_night_time(night, day, 720));   // 12:00 — noon
        assert!(!is_night_time(night, day, 1200));  // 20:00 — evening
        assert!(!is_night_time(night, day, 1259));  // 20:59 — last minute of day
    }

    #[test]
    fn test_is_night_time_no_midnight_wrap() {
        // Unusual schedule: night=02:00 (120), day=08:00 (480)
        let night = 120;
        let day = 480;

        // Night window: [02:00, 08:00)
        assert!(is_night_time(night, day, 120));    // exactly 02:00 — night starts
        assert!(is_night_time(night, day, 300));    // 05:00 — middle of night
        assert!(is_night_time(night, day, 479));    // 07:59 — last minute of night

        // Day window: [08:00, 02:00)
        assert!(!is_night_time(night, day, 480));   // exactly 08:00 — day starts
        assert!(!is_night_time(night, day, 720));   // 12:00 — day
        assert!(!is_night_time(night, day, 0));      // 00:00 — day (before night starts)
        assert!(!is_night_time(night, day, 119));    // 01:59 — day
        assert!(!is_night_time(night, day, 1260));  // 21:00 — day
    }

    #[test]
    fn test_is_night_time_same_start() {
        // Edge case: night_start == day_start — always day
        assert!(!is_night_time(420, 420, 0));
        assert!(!is_night_time(420, 420, 420));
        assert!(!is_night_time(420, 420, 1000));
    }
}

/// Main entry point: builds the Tauri app, spawns the display-dj sidecar,
/// sets up the system tray, registers shortcuts, and starts the event loop.
pub fn run() {
    env_logger::init();

    let preferences = config::load_preferences();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState {
            preferences: std::sync::Mutex::new(preferences.clone()),
            last_tray_rect: std::sync::Mutex::new(None),
            sidecar_child: std::sync::Mutex::new(None),
            expect_focus_gain: std::sync::Mutex::new(false),
            keep_awake: std::sync::Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            display::get_monitors,
            display::set_brightness,
            display::set_all_brightness,
            display::set_contrast,
            display::set_all_contrast,
            display::rename_monitor,
            display::save_monitor_order,
            display::set_monitor_visibility,
            dark_mode::get_dark_mode,
            dark_mode::set_dark_mode,
            volume::get_volume,
            volume::set_volume,
            config::get_preferences,
            config::save_preferences,
            config::open_preferences_file,
            config::open_debug_log,
            config::get_app_version,
            tray::apply_profile,
            keep_awake::get_keep_awake,
            keep_awake::set_keep_awake,
        ])
        .setup(move |app| {
            // Find an available port and store it
            let port = find_available_port(51337);
            SERVER_PORT.store(port, Ordering::Relaxed);

            // Spawn display-dj HTTP server as a background sidecar
            let (_rx, child) = app
                .shell()
                .sidecar("display-dj-server")
                .expect("display-dj-server sidecar not found")
                .args(["serve", &port.to_string()])
                .spawn()
                .expect("failed to start display-dj server");

            // Store sidecar child so we can kill it on exit
            let state = app.state::<AppState>();
            *state.sidecar_child.lock().unwrap() = Some(child);

            // Wait for the server to be ready, then optionally dump debug info
            let debug_enabled = state.preferences.lock().map(|p| p.debug_logging).unwrap_or(false);
            std::thread::spawn(move || {
                wait_for_server(port);
                if debug_enabled {
                    fetch_debug_on_startup(port);
                }
            });

            // Hide dock icon on macOS
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            // Set up system tray
            tray::setup_tray(app)?;

            // Register global shortcuts from saved preferences
            let handle = app.handle().clone();
            tray::register_shortcuts(&handle, &preferences.key_bindings);

            // Sync autostart state with saved preference
            let autostart = app.autolaunch();
            if preferences.launch_at_login {
                let _ = autostart.enable();
            } else {
                let _ = autostart.disable();
            }

            // Start night mode schedule checker
            let schedule_handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(60));
                    check_night_mode_schedule(&schedule_handle);
                }
            });

            // Hide window when it loses focus; reposition after content-driven resize
            if let Some(window) = app.get_webview_window("main") {
                let win_clone = window.clone();
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::Focused(true) => {
                            // Window gained focus: clear the flag so future focus-loss hides normally.
                            if let Some(state) = app_handle.try_state::<AppState>() {
                                if let Ok(mut e) = state.expect_focus_gain.lock() {
                                    *e = false;
                                }
                            }
                        }
                        tauri::WindowEvent::Focused(false) => {
                            // Skip hide if we're still waiting for the window to gain focus.
                            // On Linux/X11 a spurious Focused(false) fires before Focused(true)
                            // right after show(); the boolean flag blocks that false positive.
                            let expecting = app_handle
                                .try_state::<AppState>()
                                .and_then(|s| s.expect_focus_gain.lock().ok().map(|e| *e))
                                .unwrap_or(false);
                            if !expecting {
                                let _ = win_clone.hide();
                            }
                        }
                        tauri::WindowEvent::Resized(_) => {
                            if win_clone.is_visible().unwrap_or(false) {
                                let tray_rect = app_handle
                                    .try_state::<AppState>()
                                    .and_then(|s| s.last_tray_rect.lock().ok().and_then(|r| *r));
                                if let Some(rect) = tray_rect {
                                    let state = app_handle.try_state::<AppState>();
                                    let _ = tray::position_window_near_tray(&win_clone, rect, state);
                                }
                            }
                        }
                        _ => {}
                    }
                });
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building display-dj")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app_handle.state::<AppState>();
                if let Ok(mut guard) = state.sidecar_child.lock() {
                    if let Some(child) = guard.take() {
                        log::info!("killing display-dj sidecar on exit");
                        let _ = child.kill();
                    }
                };
            }
        });
}
