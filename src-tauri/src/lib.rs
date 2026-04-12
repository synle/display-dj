mod config;
mod dark_mode;
mod display;
mod tray;
mod volume;

use tauri::Manager;

pub struct AppState {
    pub preferences: std::sync::Mutex<config::Preferences>,
    pub monitor_configs: std::sync::Mutex<config::MonitorConfigs>,
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
                        let _ = win_clone.hide();
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running display-dj");
}
