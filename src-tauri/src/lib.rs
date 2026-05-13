mod config;
pub mod core;
mod dark_mode;
mod display;
mod keep_awake;
mod overlay;
pub mod sidecar_cache;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod tiling;
mod tray;
mod tray_icon;
mod volume;
mod wallpaper;

use chrono::Timelike;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt;

/// Tracks whether we've already written the one-time startup dump to the debug log.
/// Set on the first successful `fetch_all_state` call so subsequent calls are quiet.
static STARTUP_DUMP_WRITTEN: AtomicBool = AtomicBool::new(false);

/// Writes a comprehensive snapshot to the debug log the first time the frontend
/// fetches state after launch. Anchors every subsequent debug-log line to a known
/// baseline: app version, OS+arch, monitor enumeration with DDC support per panel,
/// and the platform-layer `debug_info` JSON (HMONITOR list, raw VCP brightness/
/// contrast reads, DDC enumerate error, WMI brightness). Critical for diagnosing
/// "slider moves but hardware doesn't" symptoms: the JSON shows whether DDC reads
/// even succeeded for each panel before we tried any writes.
fn write_startup_dump(app: &tauri::AppHandle) {
    use tauri::Manager;
    if STARTUP_DUMP_WRITTEN.swap(true, Ordering::Relaxed) {
        return; // already wrote it
    }
    let state = match app.try_state::<AppState>() {
        Some(s) => s,
        None => return,
    };

    let mut lines: Vec<String> = Vec::new();
    lines.push("=== STARTUP DUMP (first fetch_all_state) ===".into());
    lines.push(format!("version: {}", config::get_app_version()));
    lines.push(format!("os: {} {}", std::env::consts::OS, std::env::consts::ARCH));
    lines.push(format!("backend: in-process (display-dj-cli vendored)"));

    // Live enumerate — same code path the brightness slider hits.
    let displays = crate::core::display::list_all();
    lines.push(format!("--- core::display::list_all ({} displays) ---", displays.len()));
    for d in &displays {
        lines.push(format!(
            "  id={} type={} ddc_supported={} brightness={:?} contrast={:?} name={:?}",
            d.id, d.display_type, d.ddc_supported, d.brightness, d.contrast, d.name,
        ));
    }

    // Raw platform diagnostics — HMONITOR mapping, DDC enumerate result,
    // per-monitor VCP brightness/contrast (current+max), WMI brightness.
    lines.push("--- platform debug_info ---".into());
    let platform_dbg = <crate::core::PlatformImpl as crate::core::Platform>::debug_info();
    match serde_json::to_string_pretty(&platform_dbg) {
        Ok(s) => lines.push(s),
        Err(e) => lines.push(format!("(serialize failed: {})", e)),
    }
    lines.push("=== END STARTUP DUMP ===".into());

    config::write_debug_log(&state, &lines.join("\n"));
}

/// Stub tiling commands for platforms without tiling support (e.g. FreeBSD).
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod tiling_stubs {
    /// Tiling is not supported on this platform.
    #[tauri::command]
    pub fn get_tiling_supported() -> bool {
        false
    }

    /// Accessibility check is not applicable on this platform.
    #[tauri::command]
    pub fn get_accessibility_trusted() -> bool {
        false
    }

    /// No-op on unsupported platforms.
    #[tauri::command]
    pub fn open_accessibility_settings() {}
}

/// Response from `fetch_all_state` — all backend state in one call.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllState {
    pub monitors: Vec<display::Monitor>,
    pub is_dark: bool,
    pub volume: u32,
}

/// Fetches monitors, dark mode, and volume in parallel via the in-process platform
/// layer. Returns all three in a single response so the frontend only makes one
/// IPC call. Logs benchmark timing for each sub-call and the total.
#[tauri::command]
async fn fetch_all_state(
    app: tauri::AppHandle,
) -> Result<AllState, String> {
    use tauri::Manager;
    let t0 = std::time::Instant::now();
    // One-time per-launch dump: version, OS, full live monitor enumeration,
    // platform debug_info JSON. Anchors every later debug-log line to a known
    // baseline so we don't have to ask "what does the hardware look like".
    write_startup_dump(&app);
    if let Some(s) = app.try_state::<AppState>() {
        config::write_debug_log(&s, "benchmark: fetch_all_state — START");
    }

    // Run all 3 probes in parallel using spawned tasks. Each task gets its
    // own AppHandle clone (cheap Arc clone).
    let a1 = app.clone();
    let a2 = app.clone();
    let a3 = app.clone();

    let h_monitors = tauri::async_runtime::spawn(async move {
        let state = a1.state::<AppState>();
        display::get_monitors(state).await
    });
    let h_dark = tauri::async_runtime::spawn(async move {
        let state = a2.state::<AppState>();
        dark_mode::get_dark_mode(state).await
    });
    let h_volume = tauri::async_runtime::spawn(async move {
        let state = a3.state::<AppState>();
        volume::get_volume(state).await
    });

    let monitors_result = h_monitors.await.map_err(|e| e.to_string())?;
    let dark_result = h_dark.await.map_err(|e| e.to_string())?;
    let volume_result = h_volume.await.map_err(|e| e.to_string())?;

    let monitors = monitors_result.unwrap_or_default();
    let is_dark = dark_result.unwrap_or(false);
    let volume = volume_result.unwrap_or(0);

    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
    if let Some(s) = app.try_state::<AppState>() {
        config::write_debug_log(
            &s,
            &format!(
                "benchmark: fetch_all_state — {:.1}ms total ({} monitors, is_dark={}, volume={})",
                elapsed, monitors.len(), is_dark, volume,
            ),
        );
    }

    Ok(AllState {
        monitors,
        is_dark,
        volume,
    })
}

pub struct AppState {
    pub preferences: std::sync::Mutex<config::Preferences>,
    pub last_tray_rect: std::sync::Mutex<Option<tauri::Rect>>,
    /// True while we're waiting for the window to gain focus after being shown.
    /// Blocks spurious `Focused(false)` events that fire before `Focused(true)`
    /// (a known issue on Linux/X11 and some Windows configurations).
    /// Cleared when `Focused(true)` fires, so subsequent focus-loss auto-hides normally.
    pub expect_focus_gain: std::sync::Mutex<bool>,
    /// Holds the active keep-awake guard. When `Some`, the system is prevented
    /// from sleeping. Dropping the guard (setting to `None`) releases the assertion.
    pub keep_awake: std::sync::Mutex<Option<keepawake::KeepAwake>>,
    /// Cached tray icon state: true when dark mode is active.
    pub is_dark_mode: std::sync::Mutex<bool>,
    /// Cached tray icon state: true when volume is 0 (muted).
    pub is_muted: std::sync::Mutex<bool>,
    /// Per-window tiling state (original positions, current layout, display index).
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    pub tiling_state: std::sync::Mutex<tiling::TilingState>,
    /// TTL-based cache for monitor / dark mode / volume probes.
    pub sidecar_cache: sidecar_cache::SidecarCache,
}

/// `log::Log` implementation that tees every record to `env_logger` (stderr)
/// AND to the user's `debug.log` file via `config::write_debug_log_unbound`.
/// The tee is necessary because GUI builds have no console attached — stderr
/// alone produces no observable output for the user. Gated on
/// `config::DEBUG_LOG_ENABLED` so users with debug logging off don't pay the
/// disk-write cost.
struct TeeLogger {
    inner: env_logger::Logger,
}

impl log::Log for TeeLogger {
    /// Defer to env_logger's level/target filtering.
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.inner.enabled(metadata)
    }

    /// Forward to env_logger (stderr) and append a formatted line to
    /// `debug.log`. The debug-log line includes the module path + level
    /// prefix so a single dump shows where each line came from
    /// (`core::windows`, `display`, `tray`, …).
    fn log(&self, record: &log::Record) {
        self.inner.log(record);
        if self.inner.enabled(record.metadata()) {
            let line = format!(
                "[{}] {}: {}",
                record.level(),
                record.module_path().unwrap_or("?"),
                record.args(),
            );
            config::write_debug_log_unbound(&line);
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

/// Fetch initial dark mode and volume state via the in-process platform layer
/// and update the tray icon. Called on startup to seed the icon state.
fn fetch_initial_tray_state(app: &tauri::AppHandle) {
    if let Some(is_dark) = core::theme::get_dark_mode() {
        tray_icon::set_dark_mode_state(app, is_dark);
        log::info!("initial tray state: dark_mode={}", is_dark);
    }
    if let Some(info) = core::volume::get_volume() {
        let is_muted = info.muted || info.volume == 0;
        tray_icon::set_muted_state(app, is_muted);
        log::info!("initial tray state: muted={}", is_muted);
    }
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
/// the corresponding actions. When custom commands are configured, executes
/// those; otherwise falls back to the default brightness + dark/light mode.
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

    let is_night = is_night_time(night_start, day_start, now_minutes);

    // Use custom commands if configured, otherwise fall back to default behavior
    let commands = if is_night {
        &schedule.night_commands
    } else {
        &schedule.day_commands
    };

    if !commands.is_empty() {
        // Custom commands mode: execute each command in order
        for cmd in commands {
            tray::execute_command(app, cmd);
        }
    } else {
        // Default behavior: brightness + dark/light mode via the in-process platform layer
        let (brightness, is_dark) = if is_night {
            (schedule.night_brightness, true)
        } else {
            (schedule.day_brightness, false)
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
        let _ = core::display::set_all_brightness(brightness as u16, "force");
        let _ = core::theme::set_dark_mode(is_dark);

        tray_icon::set_dark_mode_state(app, is_night);
    }

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

    // -- Windows console-flash regression test --

    /// Regression test for the v7.0.9 Windows console-flash bug.
    ///
    /// The GUI parent runs without a console (`windows_subsystem = "windows"`).
    /// A bare `Command::new("powershell")` / `Command::new("reg")` from a
    /// `#[cfg(target_os = "windows")]` code path allocates a console for the
    /// child by default and pops a visible black flash on every brightness,
    /// volume, theme, or wallpaper change. Use `core::win_cmd::hidden_command`
    /// instead — it pre-applies the `CREATE_NO_WINDOW` (`0x08000000`) creation
    /// flag.
    ///
    /// This test fails the build if a bare spawn drifts back into the vendored
    /// `core/{windows,volume,theme,wallpaper}.rs` files.
    #[test]
    fn no_bare_powershell_or_reg_spawns_in_core() {
        let files: &[(&str, &str)] = &[
            ("core/windows.rs", include_str!("core/windows.rs")),
            ("core/volume.rs", include_str!("core/volume.rs")),
            ("core/theme.rs", include_str!("core/theme.rs")),
            ("core/wallpaper.rs", include_str!("core/wallpaper.rs")),
        ];
        let banned = [
            r#"Command::new("powershell")"#,
            r#"Command::new("reg")"#,
        ];
        for (path, src) in files {
            for pattern in &banned {
                assert!(
                    !src.contains(pattern),
                    "{}: found bare `{}` — use `super::win_cmd::hidden_command(...)` \
                     instead to avoid the Windows console flash. See the v7.0.9 fix.",
                    path,
                    pattern
                );
            }
        }
    }
}

/// Main entry point: builds the Tauri app, sets up the system tray, registers
/// shortcuts, and starts the event loop. All display, theme, volume, and
/// wallpaper operations are handled in-process via the vendored `core` module —
/// no sidecar process is spawned.
pub fn run() {
    // Default to `info` level when `RUST_LOG` isn't set, so the per-call
    // `log::info!("set_brightness[external]: …")` diagnostics in
    // `core::windows` (and equivalents in the other platforms) actually
    // surface in stderr / the bundled log capture instead of being dropped
    // at the default `error` level. Users (and especially log dumps shared
    // for support) get a real audit trail of which write path was attempted,
    // whether DDC accepted the I2C write, and whether `SetDeviceGammaRamp`
    // was rejected by the GPU driver — without that, "slider moves but
    // nothing happens" looks identical to "slider moves and the panel dims"
    // from the outside. `RUST_LOG` still wins when explicitly set.
    //
    // We then wrap env_logger in a tee that ALSO appends to `debug.log` —
    // GUI builds have no console attached, so stderr is invisible. Without
    // the tee, every `log::info!()` in `core/*` (the entire DDC/gamma write
    // audit trail) was being dropped on the floor. The tee gates on the
    // `DEBUG_LOG_ENABLED` atomic so users who haven't enabled debug logging
    // don't accumulate disk writes for nothing.
    let env = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .build();
    let max_level = env.filter();
    let logger: Box<dyn log::Log> = Box::new(TeeLogger { inner: env });
    let _ = log::set_boxed_logger(logger);
    log::set_max_level(max_level);

    let preferences = config::load_preferences();
    // Sync the AppState-less debug-log gate now that preferences are loaded.
    // `save_preferences` re-syncs it on each save so the user can toggle it
    // live from the Settings panel.
    config::DEBUG_LOG_ENABLED.store(
        preferences.debug_logging,
        std::sync::atomic::Ordering::Relaxed,
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState {
            preferences: std::sync::Mutex::new(preferences.clone()),
            last_tray_rect: std::sync::Mutex::new(None),
            expect_focus_gain: std::sync::Mutex::new(false),
            keep_awake: std::sync::Mutex::new(None),
            is_dark_mode: std::sync::Mutex::new(false),
            is_muted: std::sync::Mutex::new(false),
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            tiling_state: std::sync::Mutex::new(tiling::TilingState::new()),
            sidecar_cache: sidecar_cache::SidecarCache::new(),
        })
        .invoke_handler(tauri::generate_handler![
            fetch_all_state,
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
            config::open_app_folder,
            config::get_app_version,
            config::get_about_info,
            tray::apply_profile,
            keep_awake::get_keep_awake,
            keep_awake::set_keep_awake,
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            tiling::get_tiling_supported,
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            tiling::get_accessibility_trusted,
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            tiling::open_accessibility_settings,
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            tiling_stubs::get_tiling_supported,
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            tiling_stubs::get_accessibility_trusted,
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            tiling_stubs::open_accessibility_settings,
        ])
        .setup(move |app| {
            // Pre-warm the cache and seed the tray icon state in a background
            // thread so the main-thread tray setup isn't delayed by hardware probes.
            let startup_handle = app.handle().clone();
            std::thread::spawn(move || {
                fetch_initial_tray_state(&startup_handle);
                // Pre-warm the cache so the first popup open is instant.
                let state = startup_handle.state::<AppState>();
                let displays = core::display::list_all();
                let monitors: Vec<display::Monitor> = displays.into_iter().map(display::into_monitor).collect();
                if let Ok(prefs) = state.preferences.lock() {
                    let merged = display::merge_with_configs(monitors, &prefs.monitor_configs);
                    state.sidecar_cache.set_monitors(merged);
                }
                if let Some(is_dark) = core::theme::get_dark_mode() {
                    state.sidecar_cache.set_dark_mode(is_dark);
                }
                if let Some(info) = core::volume::get_volume() {
                    state.sidecar_cache.set_volume(info.volume);
                }
                log::info!("startup probe + cache pre-warm complete");
                // Resume wallpaper slideshow if it was enabled before shutdown
                wallpaper::resume_slideshow_if_enabled(&state);
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

            // First-launch accessibility nudge on macOS: if tiling is enabled
            // but Accessibility permission has not been granted, open the
            // Privacy & Security → Accessibility pane so the user can flip
            // the toggle for Display DJ. Without this permission, every
            // tile keybinding silently no-ops, which is confusing on a
            // fresh install. We only check (and potentially prompt) when
            // tiling is enabled — otherwise the user opted out of tiling
            // and shouldn't be nagged.
            #[cfg(target_os = "macos")]
            {
                if preferences.tiling.enabled && !tiling::is_accessibility_trusted_now() {
                    log::warn!(
                        "accessibility: not granted on launch — opening Privacy & Security pane"
                    );
                    let _ = open::that(
                        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                    );
                }
            }

            // Start tile snap (mouse edge snapping) on macOS — requires both
            // tiling.enabled and tiling.tile_snap_enabled to be true
            #[cfg(target_os = "macos")]
            {
                if preferences.tiling.enabled && preferences.tiling.tile_snap_enabled {
                    let snap_handle = app.handle().clone();
                    tiling::start_tile_snap(snap_handle);
                } else {
                    log::info!(
                        "tile_snap: skipped (tiling.enabled={}, tile_snap_enabled={})",
                        preferences.tiling.enabled,
                        preferences.tiling.tile_snap_enabled,
                    );
                }
            }

            // Z-order self-test (debug aid — opt-in via env var).
            // When DISPLAY_DJ_ZORDER_SELFTEST=1, spawn a background thread
            // that runs `run_zorder_selftest`. The routine sleeps 5s before
            // doing anything (so the operator has time to focus the window
            // they want to test), then exercises all 6 z-order commands
            // with snapshots between steps. Logs everything with a
            // `[zorder-selftest]` prefix. Defaults OFF — running this on
            // every launch would be jarring (it manipulates whatever
            // window is focused).
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            {
                if std::env::var("DISPLAY_DJ_ZORDER_SELFTEST")
                    .ok()
                    .filter(|v| !v.is_empty() && v != "0")
                    .is_some()
                {
                    let selftest_handle = app.handle().clone();
                    std::thread::spawn(move || {
                        tiling::run_zorder_selftest(&selftest_handle);
                    });
                }
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
        .run(tauri::generate_context!())
        .expect("error while running display-dj");
}
