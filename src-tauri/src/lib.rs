mod config;
mod dark_mode;
mod display;
mod tray;
mod volume;

use std::sync::atomic::{AtomicU16, Ordering};
use chrono::Timelike;
use tauri::Manager;
use tauri_plugin_shell::ShellExt;

static SERVER_PORT: AtomicU16 = AtomicU16::new(51337);

pub fn server_port() -> u16 {
    SERVER_PORT.load(Ordering::Relaxed)
}

pub struct AppState {
    pub preferences: std::sync::Mutex<config::Preferences>,
    pub monitor_configs: std::sync::Mutex<config::MonitorConfigs>,
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
    if night_start < day_start {
        // e.g. night=21:00 day=07:00 — night wraps around midnight
        now >= night_start || now < day_start
    } else {
        // e.g. night=22:00 day=18:00 — unusual but handle it
        now >= night_start && now < day_start
    }
}

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

pub fn run() {
    env_logger::init();

    let preferences = config::load_preferences();
    let monitor_configs = config::load_monitor_configs();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState {
            preferences: std::sync::Mutex::new(preferences.clone()),
            monitor_configs: std::sync::Mutex::new(monitor_configs),
        })
        .invoke_handler(tauri::generate_handler![
            display::get_monitors,
            display::set_brightness,
            display::set_all_brightness,
            display::rename_monitor,
            dark_mode::get_dark_mode,
            dark_mode::set_dark_mode,
            volume::get_volume,
            volume::set_volume,
            config::get_preferences,
            config::save_preferences,
            config::get_monitor_configs,
            config::save_monitor_config,
            config::open_config_file,
            config::open_preferences_file,
            config::open_debug_log,
            config::get_app_version,
        ])
        .setup(move |app| {
            // Find an available port and store it
            let port = find_available_port(51337);
            SERVER_PORT.store(port, Ordering::Relaxed);

            // Spawn display-dj HTTP server as a background sidecar
            let (_rx, _child) = app
                .shell()
                .sidecar("display-dj-server")
                .expect("display-dj-server sidecar not found")
                .args(["serve", &port.to_string()])
                .spawn()
                .expect("failed to start display-dj server");

            // Wait for the server to be ready (in a background thread to not block UI)
            std::thread::spawn(move || {
                wait_for_server(port);
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

            // Start night mode schedule checker
            let schedule_handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(60));
                    check_night_mode_schedule(&schedule_handle);
                }
            });

            // Hide window when it loses focus
            if let Some(window) = app.get_webview_window("main") {
                let win_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let win = win_clone.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(200));
                            if !win.is_focused().unwrap_or(true) {
                                let _ = win.hide();
                            }
                        });
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running display-dj");
}
