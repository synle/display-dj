mod config;
mod dark_mode;
mod display;
mod tray;
mod volume;

use std::sync::atomic::{AtomicU16, Ordering};
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
            display::set_contrast,
            display::set_all_brightness,
            display::set_all_contrast,
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
