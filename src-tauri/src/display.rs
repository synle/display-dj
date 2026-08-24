use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    /// Raw API id (e.g. "1", "builtin"). Used for brightness commands.
    pub id: String,
    /// Composite unique key: "{api_id}::{api_name}". Used for config lookups.
    pub uid: String,
    /// Display label (custom label from config, or api_name if no custom label).
    pub name: String,
    /// Original model name from the platform (never changes).
    pub original_name: String,
    pub brightness: u32,
    /// Current contrast level (None for displays that don't support DDC contrast).
    pub contrast: Option<u32>,
    pub supports_brightness: bool,
    pub is_built_in: bool,
    #[serde(default)]
    pub hidden: bool,
    /// Physical screen rect for this monitor as `(left, top, width, height)`
    /// in global physical pixels. Used by the soft-overlay brightness fallback
    /// to size and position a per-monitor dimming window. `None` for built-in
    /// displays (which dim natively) and for monitors on platforms that don't
    /// yet populate the rect (macOS, Linux — see TODOs in `core::macos` and
    /// `core::linux`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor_rect: Option<(i32, i32, i32, i32)>,
}

/// Convert a `core::DisplayInfo` (from the in-process platform layer) into the
/// app's `Monitor` struct, computing the composite UID and defaulting brightness
/// to 50 when unknown.
pub fn into_monitor(d: crate::core::DisplayInfo) -> Monitor {
    let is_built_in = d.display_type == "builtin";
    let uid = format!("{}::{}", d.id, d.name);
    Monitor {
        id: d.id,
        uid,
        name: d.name.clone(),
        original_name: d.name,
        brightness: d.brightness.unwrap_or(50),
        contrast: d.contrast,
        supports_brightness: true,
        is_built_in,
        hidden: false,
        monitor_rect: d.monitor_rect,
    }
}

/// Enumerate all connected displays via the in-process platform layer.
/// Runs the (potentially-blocking) hardware probe on a blocking thread so
/// we don't stall the async runtime.
async fn detect_monitors() -> Vec<Monitor> {
    log::info!("detect_monitors: enumerating via core::display::list_all");
    let displays = tauri::async_runtime::spawn_blocking(crate::core::display::list_all)
        .await
        .unwrap_or_default();
    log::info!("detect_monitors: found {} displays", displays.len());
    displays.into_iter().map(into_monitor).collect()
}

/// Resolve a monitor's screen rect with a cache-then-fresh fallback.
///
/// First checks the supplied cached `Monitor` (if any). If the cache entry
/// is missing or its `monitor_rect` is `None`, runs a fresh
/// `core::display::list_all()` on a blocking thread and tries again. v7.0.19
/// shipped overlay code that bailed out with "no monitor_rect" because the
/// cached `Monitor.monitor_rect` could be `None` even though the underlying
/// `core::DisplayInfo.monitor_rect` was populated on Windows — this helper
/// closes that gap so the overlay path always sees the right rect (at the
/// cost of one extra enumerate on the slow path).
pub async fn resolve_monitor_rect(
    monitor_id: &str,
    cached: &[Monitor],
) -> Option<(i32, i32, i32, i32)> {
    if let Some(rect) = cached
        .iter()
        .find(|m| m.id == monitor_id)
        .and_then(|m| m.monitor_rect)
    {
        return Some(rect);
    }
    let id = monitor_id.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        crate::core::display::list_all()
            .into_iter()
            .find(|d| d.id == id)
            .and_then(|d| d.monitor_rect)
    })
    .await
    .ok()
    .flatten()
}

/// Set brightness on a single monitor via the in-process platform layer with
/// the requested platform mode (`"ddc"`, `"gamma"`, or `"force"` for auto),
/// clamped to `[min_brightness, 100]`. Returns whether the platform call
/// succeeded — `false` is the trigger for the soft-overlay fallback in
/// "auto" mode.
async fn set_monitor_brightness(
    monitor_id: &str,
    value: u32,
    min_brightness: u32,
    mode: &str,
) -> Result<bool, String> {
    let clamped = value.clamp(min_brightness, 100);
    let id = monitor_id.to_string();
    let mode_owned = mode.to_string();
    log::info!(
        "set_monitor_brightness: id={} value={} clamped={} min={} mode={}",
        id, value, clamped, min_brightness, mode_owned,
    );
    let ok = tauri::async_runtime::spawn_blocking(move || {
        crate::core::display::set_one_brightness(&id, clamped as u16, &mode_owned)
    })
    .await
    .map_err(|e| format!("brightness task join failed: {}", e))?;
    Ok(ok)
}

/// Set brightness on all monitors via the in-process platform layer using
/// `"force"` mode (DDC -> gamma fallback per monitor). Used by the legacy
/// all-at-once path; for per-monitor mode dispatch (DDC-only, gamma-only,
/// overlay-only) callers should iterate `monitor_configs` and call
/// [`set_monitor_brightness`] per monitor instead.
///
/// Returns the per-monitor `(id, success)` results so the caller can log them
/// and surface partial failures.
async fn set_all_monitors_brightness(value: u32, min_brightness: u32) -> Result<Vec<(String, bool)>, String> {
    let clamped = value.clamp(min_brightness, 100);
    log::info!("set_all_monitors_brightness: value={} clamped={} min={}", value, clamped, min_brightness);
    let results = tauri::async_runtime::spawn_blocking(move || {
        crate::core::display::set_all_brightness(clamped as u16, "force")
    })
    .await
    .map_err(|e| format!("set_all_brightness task join failed: {}", e))?;
    Ok(results)
}

/// Set contrast on a single monitor via the in-process platform layer (0-100, DDC-only).
async fn set_monitor_contrast(monitor_id: &str, value: u32) -> Result<bool, String> {
    let clamped = value.min(100);
    let id = monitor_id.to_string();
    log::info!("set_monitor_contrast: id={} value={}", id, clamped);
    let ok = tauri::async_runtime::spawn_blocking(move || {
        crate::core::display::set_one_contrast(&id, clamped as u16)
    })
    .await
    .map_err(|e| format!("contrast task join failed: {}", e))?;
    Ok(ok)
}

/// Set contrast on all monitors via the in-process platform layer.
/// Returns the per-monitor (id, success) results so the caller can log them.
async fn set_all_monitors_contrast(value: u32) -> Result<Vec<(String, bool)>, String> {
    let clamped = value.min(100);
    log::info!("set_all_monitors_contrast: value={}", clamped);
    let results = tauri::async_runtime::spawn_blocking(move || {
        crate::core::display::set_all_contrast(clamped as u16)
    })
    .await
    .map_err(|e| format!("set_all_contrast task join failed: {}", e))?;
    Ok(results)
}

// ===========================================================================
// Brightness mode routing
// ===========================================================================

/// Resolve the brightness mode for a monitor identified by its raw API id.
///
/// Looks up the matching `MonitorMetadata` entry by `api_id`. Falls back to
/// `"auto"` when the monitor has no metadata yet (first sighting) or when the
/// stored mode is something the routing logic doesn't recognise — i.e. the
/// auto-discovery path is the safe default for unknown inputs.
///
/// # Arguments
/// * `configs` - Slice of saved per-monitor metadata from preferences.
/// * `api_id` - Raw `core::DisplayInfo.id` (e.g. `"1"`, `"builtin"`).
///
/// # Returns
/// One of `"auto"`, `"ddc"`, `"gamma"`, `"overlay"`.
pub fn resolve_brightness_mode(
    configs: &[crate::config::MonitorMetadata],
    api_id: &str,
) -> String {
    let stored = configs
        .iter()
        .find(|m| m.api_id == api_id)
        .map(|m| m.brightness_mode.as_str())
        .unwrap_or("auto");
    match stored {
        "auto" | "ddc" | "gamma" | "overlay" => stored.into(),
        // Unknown mode -> safe default rather than silently doing nothing.
        _ => "auto".into(),
    }
}

/// Route decision for the brightness dispatcher.
///
/// Encodes "which platform code path, and should we also touch the soft-
/// overlay window?" as a tiny enum so the routing logic is unit-testable
/// independently of any Tauri / hardware context.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BrightnessRoute {
    /// Try DDC then gamma (`"force"` mode in `core::display`); fall back to
    /// the overlay only if **both** hardware paths fail (`auto` mode).
    AutoWithOverlayFallback,
    /// DDC only. No overlay. Tear down any existing overlay so old state
    /// doesn't double-dim.
    DdcOnly,
    /// Gamma only. No overlay. Tear down any existing overlay.
    GammaOnly,
    /// Overlay only. Skip the hardware paths entirely.
    OverlayOnly,
}

/// Pure routing decision: given the user's brightness mode string, pick the
/// dispatcher path. Extracted for unit-testing; the Tauri commands just call
/// this and then act on the result.
///
/// # Arguments
/// * `mode` - One of `"auto"`, `"ddc"`, `"gamma"`, `"overlay"`. Anything else
///   collapses to [`BrightnessRoute::AutoWithOverlayFallback`].
///
/// # Returns
/// The corresponding [`BrightnessRoute`].
pub fn route_for_mode(mode: &str) -> BrightnessRoute {
    match mode {
        "ddc" => BrightnessRoute::DdcOnly,
        "gamma" => BrightnessRoute::GammaOnly,
        "overlay" => BrightnessRoute::OverlayOnly,
        _ => BrightnessRoute::AutoWithOverlayFallback,
    }
}

// ===========================================================================
// Common helpers
// ===========================================================================

/// Applies saved metadata (custom labels, hidden state) to detected monitors
/// and sorts them by the user's configured sort order.
pub fn merge_with_configs(
    monitors: Vec<Monitor>,
    configs: &[crate::config::MonitorMetadata],
) -> Vec<Monitor> {
    let mut result: Vec<Monitor> = Vec::new();

    for mut monitor in monitors {
        if let Some(meta) = configs.iter().find(|m| m.uid == monitor.uid) {
            if !meta.label.is_empty() {
                monitor.name = meta.label.clone();
            }
            monitor.hidden = meta.hidden;
        }
        result.push(monitor);
    }

    result.sort_by(|a, b| {
        let order_a = configs.iter().find(|c| c.uid == a.uid).map(|c| c.sort_order).unwrap_or(i32::MAX);
        let order_b = configs.iter().find(|c| c.uid == b.uid).map(|c| c.sort_order).unwrap_or(i32::MAX);
        order_a.cmp(&order_b).then(a.uid.cmp(&b.uid))
    });

    result
}

/// Fix up migrated entries whose api_name is "unknown" — once we detect the real
/// monitor, we can fill in the correct uid and api_name.
fn reconcile_migrated_configs(
    monitors: &[Monitor],
    configs: &mut Vec<crate::config::MonitorMetadata>,
) -> bool {
    let mut changed = false;
    for monitor in monitors {
        if configs.iter().any(|c| c.uid == monitor.uid) {
            continue;
        }
        if let Some(meta) = configs.iter_mut().find(|c| c.api_id == monitor.id && c.api_name == "unknown") {
            meta.uid = monitor.uid.clone();
            meta.api_name = monitor.original_name.clone();
            changed = true;
        }
    }
    changed
}

/// Ensure every detected monitor has a metadata entry in preferences.
/// New monitors get an entry with empty label (will display api_name).
fn ensure_metadata_for_monitors(
    monitors: &[Monitor],
    configs: &mut Vec<crate::config::MonitorMetadata>,
) -> bool {
    let mut changed = false;
    let next_order = configs.iter().map(|c| c.sort_order).max().unwrap_or(-1) + 1;

    for (i, monitor) in monitors.iter().enumerate() {
        if !configs.iter().any(|c| c.uid == monitor.uid) {
            configs.push(crate::config::MonitorMetadata {
                uid: monitor.uid.clone(),
                api_id: monitor.id.clone(),
                api_name: monitor.original_name.clone(),
                label: String::new(),
                sort_order: next_order + i as i32,
                hidden: false,
                brightness_mode: crate::config::default_brightness_mode(),
            });
            changed = true;
        }
    }
    changed
}

// ===========================================================================
// Tauri commands
// ===========================================================================

/// Returns all connected monitors with saved metadata applied.
/// Uses a 5-minute TTL cache to avoid re-probing hardware on every poll.
/// Reconciles migrated configs and ensures new monitors get metadata entries.
#[tauri::command]
pub async fn get_monitors(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<Monitor>, String> {
    let t0 = std::time::Instant::now();
    crate::config::write_debug_log(&state, "benchmark: get_monitors — START");

    // Return cached monitors if fresh
    if let Some(cached) = state.sidecar_cache.get_monitors() {
        crate::config::write_debug_log(
            &state,
            &format!("benchmark: get_monitors — {:.1}ms (cache hit)", t0.elapsed().as_secs_f64() * 1000.0),
        );
        return Ok(cached);
    }

    crate::config::write_debug_log(&state, "benchmark: get_monitors — probing hardware...");
    let monitors = detect_monitors().await;
    let t_probe = t0.elapsed();
    crate::config::write_debug_log(
        &state,
        &format!("benchmark: get_monitors — probe returned in {:.1}ms ({} displays)", t_probe.as_secs_f64() * 1000.0, monitors.len()),
    );

    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;

    let mut dirty = reconcile_migrated_configs(&monitors, &mut prefs.monitor_configs);
    dirty |= ensure_metadata_for_monitors(&monitors, &mut prefs.monitor_configs);
    if dirty {
        crate::config::save_preferences_to_disk(&prefs);
    }

    let result = merge_with_configs(monitors, &prefs.monitor_configs);
    state.sidecar_cache.set_monitors(result.clone());

    crate::config::write_debug_log(
        &state,
        &format!(
            "benchmark: get_monitors — {:.1}ms total (probe={:.1}ms, {} monitors)",
            t0.elapsed().as_secs_f64() * 1000.0,
            t_probe.as_secs_f64() * 1000.0,
            result.len(),
        ),
    );
    Ok(result)
}

/// Sets brightness for a single monitor, enforcing the minimum brightness
/// floor and routing through the per-monitor `brightnessMode` preference.
///
/// Dispatch rules (see [`BrightnessRoute`]):
/// - `"auto"` — try DDC then gamma (`core::display` "force" mode). If both
///   hardware paths fail, show the soft-overlay dimmer. On success, tear
///   down any existing overlay so a previous overlay state doesn't keep
///   double-dimming the panel.
/// - `"ddc"` / `"gamma"` — call the matching platform path only; never show
///   an overlay; tear down any existing overlay first.
/// - `"overlay"` — skip hardware entirely; show / update the overlay.
#[tauri::command]
pub async fn set_brightness(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    monitor_id: String,
    value: u32,
) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    // Snapshot the preference fields we need *before* awaiting anything so we
    // don't hold the mutex across `.await` (AGENTS.md tray-icon pitfall).
    let (min, mode) = {
        let prefs = state.preferences.lock().map_err(|e| e.to_string())?;
        let mode = resolve_brightness_mode(&prefs.monitor_configs, &monitor_id);
        (prefs.effective_min_brightness(), mode)
    };
    // Resolve the monitor rect with a cache-first, fresh-enumerate fallback.
    // v7.0.19 shipped an overlay path that bailed with "no monitor_rect"
    // because the cache could miss the rect even when the underlying core
    // `DisplayInfo` had it populated; the helper now falls back to a fresh
    // enumerate when that happens.
    let cached_monitors = state.sidecar_cache.get_monitors().unwrap_or_default();
    let monitor_rect = resolve_monitor_rect(&monitor_id, &cached_monitors).await;
    log::info!(
        "set_brightness: id={} mode={} cache_size={} rect={:?}",
        monitor_id, mode, cached_monitors.len(), monitor_rect,
    );
    crate::config::write_debug_log(
        &state,
        &format!(
            "set_brightness: id={} value={} min={} mode={} has_rect={} cache_size={} — START",
            monitor_id, value, min, mode, monitor_rect.is_some(), cached_monitors.len(),
        ),
    );
    state.sidecar_cache.invalidate_monitors();

    let route = route_for_mode(&mode);
    let result: Result<bool, String> = match route {
        BrightnessRoute::DdcOnly => {
            let _ = crate::overlay::destroy_overlay(&app, &monitor_id);
            set_monitor_brightness(&monitor_id, value, min, "ddc").await
        }
        BrightnessRoute::GammaOnly => {
            let _ = crate::overlay::destroy_overlay(&app, &monitor_id);
            set_monitor_brightness(&monitor_id, value, min, "gamma").await
        }
        BrightnessRoute::OverlayOnly => {
            // Pure overlay mode: skip hardware entirely.
            crate::overlay::set_overlay_brightness(&app, &monitor_id, monitor_rect, value)
                .map(|_| true)
        }
        BrightnessRoute::AutoWithOverlayFallback => {
            // Try the hardware path first; fall through to overlay on failure.
            let hw_ok = set_monitor_brightness(&monitor_id, value, min, "force").await?;
            if hw_ok {
                let _ = crate::overlay::destroy_overlay(&app, &monitor_id);
                Ok(true)
            } else {
                log::info!(
                    "set_brightness: id={} hardware path failed, falling back to overlay",
                    monitor_id,
                );
                crate::overlay::set_overlay_brightness(&app, &monitor_id, monitor_rect, value)
                    .map(|_| true)
            }
        }
    };
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
    match &result {
        Ok(ok) => crate::config::write_debug_log(
            &state,
            &format!(
                "set_brightness: id={} mode={} platform_ok={} — {:.1}ms",
                monitor_id, mode, ok, elapsed,
            ),
        ),
        Err(e) => crate::config::write_debug_log(
            &state,
            &format!(
                "set_brightness: id={} mode={} ERROR={} — {:.1}ms",
                monitor_id, mode, e, elapsed,
            ),
        ),
    }
    match result? {
        true => Ok(()),
        false => Err(format!("set_brightness failed for monitor {}", monitor_id)),
    }
}

/// Sets brightness for all monitors, enforcing the minimum brightness floor
/// and routing each monitor through its individual `brightnessMode`.
///
/// Fast path: if **every** monitor in the cached list is in `"auto"` mode
/// (or the cache is empty), this falls back to the legacy bulk
/// `core::display::set_all_brightness("force")` call which is one platform
/// round-trip total. Slow path: when any monitor has a non-auto mode (or any
/// monitor has an overlay set), each monitor is dispatched individually so
/// per-monitor mode is honored. The slow path is `O(n)` blocking-thread
/// hops; n is typically 1-3 monitors so this is fine.
#[tauri::command]
pub async fn set_all_brightness(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    value: u32,
) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    let (min, configs, cached_monitors) = {
        let prefs = state.preferences.lock().map_err(|e| e.to_string())?;
        (
            prefs.effective_min_brightness(),
            prefs.monitor_configs.clone(),
            state.sidecar_cache.get_monitors().unwrap_or_default(),
        )
    };
    crate::config::write_debug_log(
        &state,
        &format!("set_all_brightness: value={} min={} — START", value, min),
    );
    state.sidecar_cache.invalidate_monitors();

    // Decide whether we can take the fast path. The fast path is fine when
    // every monitor is in "auto" mode AND no monitor currently has an overlay
    // window (otherwise the bulk call would leave the overlay state stale).
    let all_auto = cached_monitors.iter().all(|m| {
        m.is_built_in
            || resolve_brightness_mode(&configs, &m.id) == "auto"
    });

    if all_auto && !cached_monitors.is_empty() {
        // Fast path: one bulk hardware call covers all monitors. Any monitor
        // that fails will get an overlay fallback by running through the slow
        // loop below; here we just check the aggregate result.
        let result = set_all_monitors_brightness(value, min).await;
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        match &result {
            Ok(per_monitor) => {
                // Tear down stale overlays on monitors that succeeded;
                // fall back to overlay for any monitor that failed.
                for (id, ok) in per_monitor {
                    if *ok {
                        let _ = crate::overlay::destroy_overlay(&app, id);
                    } else if let Some(m) = cached_monitors.iter().find(|m| &m.id == id) {
                        if !m.is_built_in {
                            // Resolve the rect via the cache-then-fresh helper so
                            // we don't bail when the cache happens to be missing
                            // monitor_rect on an external panel.
                            let rect = resolve_monitor_rect(id, &cached_monitors).await;
                            log::info!(
                                "set_all_brightness: id={} hardware failed, falling back to overlay (rect={:?})",
                                id, rect,
                            );
                            let _ = crate::overlay::set_overlay_brightness(
                                &app, id, rect, value,
                            );
                        }
                    }
                }
                let summary = per_monitor.iter()
                    .map(|(id, ok)| format!("{}={}", id, ok))
                    .collect::<Vec<_>>()
                    .join(", ");
                crate::config::write_debug_log(
                    &state,
                    &format!(
                        "set_all_brightness: value={} per_monitor=[{}] — {:.1}ms (fast)",
                        value, summary, elapsed,
                    ),
                );
            }
            Err(e) => crate::config::write_debug_log(
                &state,
                &format!(
                    "set_all_brightness: value={} ERROR={} — {:.1}ms (fast)",
                    value, e, elapsed,
                ),
            ),
        }
        return result.map(|_| ());
    }

    // Slow path: dispatch per-monitor so each monitor's `brightnessMode` is
    // honored. We can't iterate `configs` directly because those are stored
    // metadata (which can include unplugged monitors). Iterate the cached
    // live monitor list instead. If the cache is empty, fall back to the
    // legacy bulk call so we don't silently no-op.
    if cached_monitors.is_empty() {
        let result = set_all_monitors_brightness(value, min).await;
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        crate::config::write_debug_log(
            &state,
            &format!(
                "set_all_brightness: value={} cache_empty fallback — {:.1}ms",
                value, elapsed,
            ),
        );
        return result.map(|_| ());
    }

    let mut summary_lines: Vec<String> = Vec::with_capacity(cached_monitors.len());
    for m in &cached_monitors {
        let mode = resolve_brightness_mode(&configs, &m.id);
        let route = route_for_mode(&mode);
        let res: Result<bool, String> = match route {
            BrightnessRoute::DdcOnly => {
                let _ = crate::overlay::destroy_overlay(&app, &m.id);
                set_monitor_brightness(&m.id, value, min, "ddc").await
            }
            BrightnessRoute::GammaOnly => {
                let _ = crate::overlay::destroy_overlay(&app, &m.id);
                set_monitor_brightness(&m.id, value, min, "gamma").await
            }
            BrightnessRoute::OverlayOnly => {
                let rect = resolve_monitor_rect(&m.id, &cached_monitors).await;
                crate::overlay::set_overlay_brightness(&app, &m.id, rect, value)
                    .map(|_| true)
            }
            BrightnessRoute::AutoWithOverlayFallback => {
                // NOTE: deliberately no `?` here. This runs inside the
                // per-monitor loop, so propagating would abandon every
                // *remaining* monitor because one panel errored — the user
                // would see some displays dim and the rest stay put. Fold the
                // error into this monitor's result and keep going.
                match set_monitor_brightness(&m.id, value, min, "force").await {
                    Ok(true) => {
                        let _ = crate::overlay::destroy_overlay(&app, &m.id);
                        Ok(true)
                    }
                    Ok(false) => {
                        let rect = resolve_monitor_rect(&m.id, &cached_monitors).await;
                        crate::overlay::set_overlay_brightness(&app, &m.id, rect, value)
                            .map(|_| true)
                    }
                    Err(e) => Err(e),
                }
            }
        };
        match res {
            Ok(ok) => summary_lines.push(format!("{}(mode={})={}", m.id, mode, ok)),
            Err(e) => summary_lines.push(format!("{}(mode={})=ERR:{}", m.id, mode, e)),
        }
    }
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
    crate::config::write_debug_log(
        &state,
        &format!(
            "set_all_brightness: value={} per_monitor=[{}] — {:.1}ms (slow/per-mode)",
            value, summary_lines.join(", "), elapsed,
        ),
    );
    Ok(())
}

/// Sets contrast for a single monitor (0-100, DDC-only).
#[tauri::command]
pub async fn set_contrast(
    state: tauri::State<'_, crate::AppState>,
    monitor_id: String,
    value: u32,
) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    crate::config::write_debug_log(
        &state,
        &format!("set_contrast: id={} value={} — START", monitor_id, value),
    );
    let result = set_monitor_contrast(&monitor_id, value).await;
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
    match &result {
        Ok(ok) => crate::config::write_debug_log(
            &state,
            &format!("set_contrast: id={} platform_ok={} — {:.1}ms", monitor_id, ok, elapsed),
        ),
        Err(e) => crate::config::write_debug_log(
            &state,
            &format!("set_contrast: id={} ERROR={} — {:.1}ms", monitor_id, e, elapsed),
        ),
    }
    match result? {
        true => Ok(()),
        false => Err(format!("set_contrast failed for monitor {}", monitor_id)),
    }
}

/// Sets contrast for all monitors (0-100, DDC-only).
#[tauri::command]
pub async fn set_all_contrast(
    state: tauri::State<'_, crate::AppState>,
    value: u32,
) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    crate::config::write_debug_log(
        &state,
        &format!("set_all_contrast: value={} — START", value),
    );
    let result = set_all_monitors_contrast(value).await;
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
    match &result {
        Ok(per_monitor) => {
            let summary = per_monitor.iter()
                .map(|(id, ok)| format!("{}={}", id, ok))
                .collect::<Vec<_>>()
                .join(", ");
            crate::config::write_debug_log(
                &state,
                &format!("set_all_contrast: value={} per_monitor=[{}] — {:.1}ms", value, summary, elapsed),
            );
        }
        Err(e) => crate::config::write_debug_log(
            &state,
            &format!("set_all_contrast: value={} ERROR={} — {:.1}ms", value, e, elapsed),
        ),
    }
    result.map(|_| ())
}

/// Updates a monitor's custom label in preferences. Creates a new metadata entry
/// if the monitor isn't tracked yet.
#[tauri::command]
pub fn rename_monitor(
    state: tauri::State<'_, crate::AppState>,
    uid: String,
    name: String,
) -> Result<(), String> {
    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    if let Some(meta) = prefs.monitor_configs.iter_mut().find(|m| m.uid == uid) {
        meta.label = name;
    } else {
        let parts: Vec<&str> = uid.splitn(2, "::").collect();
        let (api_id, api_name) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (uid.clone(), String::new())
        };
        prefs.monitor_configs.push(crate::config::MonitorMetadata {
            uid: uid.clone(),
            api_id,
            api_name,
            label: name,
            sort_order: 0,
            hidden: false,
            brightness_mode: crate::config::default_brightness_mode(),
        });
    }
    crate::config::save_preferences_to_disk(&prefs);
    Ok(())
}

/// Persists the user's custom monitor sort order to preferences.
#[tauri::command]
pub fn save_monitor_order(
    state: tauri::State<'_, crate::AppState>,
    orders: Vec<(String, i32)>,
) -> Result<(), String> {
    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    for (uid, sort_order) in orders {
        if let Some(meta) = prefs.monitor_configs.iter_mut().find(|m| m.uid == uid) {
            meta.sort_order = sort_order;
        } else {
            let parts: Vec<&str> = uid.splitn(2, "::").collect();
            let (api_id, api_name) = if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (uid.clone(), String::new())
            };
            prefs.monitor_configs.push(crate::config::MonitorMetadata {
                uid,
                api_id,
                api_name,
                label: String::new(),
                sort_order,
                hidden: false,
                brightness_mode: crate::config::default_brightness_mode(),
            });
        }
    }
    crate::config::save_preferences_to_disk(&prefs);
    Ok(())
}

/// Toggles a monitor's hidden state in preferences (hidden monitors are excluded from the main UI).
#[tauri::command]
pub fn set_monitor_visibility(
    state: tauri::State<'_, crate::AppState>,
    uid: String,
    hidden: bool,
) -> Result<(), String> {
    let mut prefs = state.preferences.lock().map_err(|e| e.to_string())?;
    if let Some(meta) = prefs.monitor_configs.iter_mut().find(|m| m.uid == uid) {
        meta.hidden = hidden;
    }
    crate::config::save_preferences_to_disk(&prefs);
    Ok(())
}

/// Resolves a monitor identifier string to a Monitor, trying (in order):
/// 1. Exact match on `id` ("1", "2", "builtin")
/// 2. Exact match on `uid` ("1::Dell U2723QE")
/// 3. Case-insensitive substring match on `name` or `original_name` ("Dell", "LG")
/// Returns the first match found and its 0-based index, or None.
pub(crate) fn resolve_monitor(monitors: &[Monitor], query: &str) -> Option<(usize, Monitor)> {
    // 1. Exact id match
    if let Some((i, m)) = monitors.iter().enumerate().find(|(_, m)| m.id == query) {
        return Some((i, m.clone()));
    }
    // 2. Exact uid match
    if let Some((i, m)) = monitors.iter().enumerate().find(|(_, m)| m.uid == query) {
        return Some((i, m.clone()));
    }
    // 3. Case-insensitive substring on name or original_name
    let needle = query.to_lowercase();
    monitors.iter().enumerate().find(|(_, m)| {
        m.name.to_lowercase().contains(&needle)
            || m.original_name.to_lowercase().contains(&needle)
    }).map(|(i, m)| (i, m.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_monitor(id: &str, name: &str, is_built_in: bool) -> Monitor {
        Monitor {
            id: id.into(),
            uid: format!("{}::{}", id, name),
            name: name.into(),
            original_name: name.into(),
            brightness: 50,
            contrast: None,
            supports_brightness: true,
            is_built_in,
            hidden: false,
            monitor_rect: None,
        }
    }

    fn make_meta(uid: &str, label: &str, sort_order: i32) -> crate::config::MonitorMetadata {
        let parts: Vec<&str> = uid.splitn(2, "::").collect();
        crate::config::MonitorMetadata {
            uid: uid.into(),
            api_id: parts.first().unwrap_or(&"").to_string(),
            api_name: parts.get(1).unwrap_or(&"").to_string(),
            label: label.into(),
            sort_order,
            hidden: false,
            brightness_mode: crate::config::default_brightness_mode(),
        }
    }

    #[test]
    fn test_monitor_serialization_camel_case() {
        let monitor = make_monitor("builtin", "Built-in", true);
        let json = serde_json::to_string(&monitor).unwrap();
        assert!(json.contains("\"supportsBrightness\""));
        assert!(json.contains("\"isBuiltIn\""));
        assert!(json.contains("\"uid\""));
        assert!(!json.contains("supports_brightness"));
        assert!(!json.contains("is_built_in"));
    }

    #[test]
    fn test_monitor_deserialization() {
        let json = r#"{
            "id": "1",
            "uid": "1::Dell U2723QE",
            "name": "Dell U2723QE",
            "originalName": "Dell U2723QE",
            "brightness": 80,
            "contrast": 60,
            "supportsBrightness": true,
            "isBuiltIn": false
        }"#;
        let monitor: Monitor = serde_json::from_str(json).unwrap();
        assert_eq!(monitor.id, "1");
        assert_eq!(monitor.uid, "1::Dell U2723QE");
        assert_eq!(monitor.name, "Dell U2723QE");
        assert_eq!(monitor.brightness, 80);
        assert!(!monitor.is_built_in);
    }

    #[test]
    fn test_monitor_roundtrip_serialization() {
        let original = make_monitor("2", "LG 27UK850", false);
        let json = serde_json::to_string(&original).unwrap();
        let restored: Monitor = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, original.id);
        assert_eq!(restored.uid, original.uid);
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.brightness, original.brightness);
        assert_eq!(restored.supports_brightness, original.supports_brightness);
        assert_eq!(restored.is_built_in, original.is_built_in);
    }

    #[test]
    fn test_merge_with_configs_renames_monitor() {
        let monitors = vec![make_monitor("1", "External Display 1", false)];
        let configs = vec![make_meta("1::External Display 1", "My Dell", 0)];
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "My Dell");
    }

    #[test]
    fn test_merge_with_configs_sorts_by_sort_order() {
        let monitors = vec![
            make_monitor("1", "Monitor A", false),
            make_monitor("2", "Monitor B", false),
            make_monitor("builtin", "Built-in", true),
        ];
        let configs = vec![
            make_meta("2::Monitor B", "Monitor B", 1),
            make_meta("builtin::Built-in", "Built-in", 0),
        ];
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].uid, "builtin::Built-in");
        assert_eq!(result[1].uid, "2::Monitor B");
        assert_eq!(result[2].uid, "1::Monitor A");
    }

    #[test]
    fn test_merge_with_configs_empty_label_keeps_original() {
        let monitors = vec![make_monitor("1", "Original Name", false)];
        let configs = vec![make_meta("1::Original Name", "", 0)];
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result[0].name, "Original Name");
    }

    #[test]
    fn test_merge_with_configs_no_configs() {
        let monitors = vec![
            make_monitor("builtin", "Built-in", true),
            make_monitor("1", "External", false),
        ];
        let configs: Vec<crate::config::MonitorMetadata> = Vec::new();
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_merge_with_configs_preserves_original_name() {
        let monitors = vec![make_monitor("1", "Dell U2723QE", false)];
        let configs = vec![make_meta("1::Dell U2723QE", "My Custom Label", 0)];
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result[0].name, "My Custom Label");
        assert_eq!(result[0].original_name, "Dell U2723QE"); // must NOT change
    }

    #[test]
    fn test_merge_with_configs_same_sort_order_tiebreaks_by_uid() {
        let monitors = vec![
            make_monitor("2", "Monitor B", false),
            make_monitor("1", "Monitor A", false),
        ];
        let configs = vec![
            make_meta("2::Monitor B", "", 0),
            make_meta("1::Monitor A", "", 0), // same sort_order
        ];
        let result = merge_with_configs(monitors, &configs);
        // Tiebreaker is uid ascending: "1::Monitor A" < "2::Monitor B"
        assert_eq!(result[0].uid, "1::Monitor A");
        assert_eq!(result[1].uid, "2::Monitor B");
    }

    #[test]
    fn test_merge_with_configs_unplugged_monitors_not_in_result() {
        // Config has entries for 3 monitors, but only 1 is currently connected
        let monitors = vec![make_monitor("1", "Dell", false)];
        let configs = vec![
            make_meta("1::Dell", "My Dell", 0),
            make_meta("2::LG", "Office Left", 1),       // unplugged
            make_meta("builtin::Built-in", "MacBook", 2), // unplugged
        ];
        let result = merge_with_configs(monitors, &configs);
        assert_eq!(result.len(), 1); // only the connected monitor
        assert_eq!(result[0].name, "My Dell");
    }

    /// Verifies the `into_monitor` helper preserves the original_name from the
    /// platform layer's DisplayInfo.
    #[test]
    fn test_into_monitor_preserves_original_name() {
        let info = crate::core::DisplayInfo {
            id: "1".into(),
            name: "Dell U2723QE".into(),
            display_type: "external".into(),
            brightness: Some(70),
            contrast: Some(60),
            ddc_supported: true,
            monitor_rect: None,
        };
        let m = into_monitor(info);
        assert_eq!(m.name, "Dell U2723QE");
        assert_eq!(m.original_name, "Dell U2723QE");
        assert_eq!(m.name, m.original_name);
    }

    /// Verifies the `into_monitor` helper detects builtin displays.
    #[test]
    fn test_into_monitor_builtin() {
        let info = crate::core::DisplayInfo {
            id: "builtin".into(),
            name: "Built-in Display".into(),
            display_type: "builtin".into(),
            brightness: Some(80),
            contrast: None,
            ddc_supported: false,
            monitor_rect: None,
        };
        let m = into_monitor(info);
        assert!(m.is_built_in);
        assert_eq!(m.brightness, 80);
        assert_eq!(m.uid, "builtin::Built-in Display");
    }

    /// Verifies the `into_monitor` helper preserves contrast on DDC displays.
    #[test]
    fn test_into_monitor_external_ddc() {
        let info = crate::core::DisplayInfo {
            id: "1".into(),
            name: "Dell U2723QE".into(),
            display_type: "external".into(),
            brightness: Some(50),
            contrast: Some(75),
            ddc_supported: true,
            monitor_rect: None,
        };
        let m = into_monitor(info);
        assert!(!m.is_built_in);
        assert_eq!(m.brightness, 50);
        assert_eq!(m.contrast, Some(75));
        assert_eq!(m.uid, "1::Dell U2723QE");
    }

    /// Verifies the `into_monitor` helper defaults brightness to 50 when None.
    #[test]
    fn test_into_monitor_null_brightness() {
        let info = crate::core::DisplayInfo {
            id: "2".into(),
            name: "Unknown".into(),
            display_type: "external".into(),
            brightness: None,
            contrast: None,
            ddc_supported: false,
            monitor_rect: None,
        };
        let m = into_monitor(info);
        assert_eq!(m.brightness, 50);
        assert_eq!(m.uid, "2::Unknown");
    }

    #[test]
    fn test_reconcile_migrated_configs() {
        let monitors = vec![make_monitor("1", "Dell U2723QE", false)];
        let mut configs = vec![crate::config::MonitorMetadata {
            uid: "1::unknown".into(),
            api_id: "1".into(),
            api_name: "unknown".into(),
            label: "My Dell".into(),
            sort_order: 0,
            hidden: false,
            brightness_mode: crate::config::default_brightness_mode(),
        }];
        let changed = reconcile_migrated_configs(&monitors, &mut configs);
        assert!(changed);
        assert_eq!(configs[0].uid, "1::Dell U2723QE");
        assert_eq!(configs[0].api_name, "Dell U2723QE");
        assert_eq!(configs[0].label, "My Dell"); // label preserved
    }

    #[test]
    fn test_ensure_metadata_for_monitors() {
        let monitors = vec![
            make_monitor("builtin", "Built-in", true),
            make_monitor("1", "Dell", false),
        ];
        let mut configs: Vec<crate::config::MonitorMetadata> = Vec::new();
        let changed = ensure_metadata_for_monitors(&monitors, &mut configs);
        assert!(changed);
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].uid, "builtin::Built-in");
        assert_eq!(configs[1].uid, "1::Dell");
        assert_eq!(configs[0].label, ""); // default empty label

        // Running again should not add duplicates
        let changed2 = ensure_metadata_for_monitors(&monitors, &mut configs);
        assert!(!changed2);
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_ensure_metadata_sort_order_continues_from_existing() {
        let monitors = vec![
            make_monitor("builtin", "Built-in", true),
            make_monitor("3", "New Monitor", false),
        ];
        // Pre-existing configs with sort orders 0, 5, 10
        let mut configs = vec![
            make_meta("builtin::Built-in", "MacBook", 0),
            make_meta("1::Dell", "Left", 5),   // unplugged but persisted
            make_meta("2::LG", "Right", 10),    // unplugged but persisted
        ];
        let changed = ensure_metadata_for_monitors(&monitors, &mut configs);
        assert!(changed); // "3::New Monitor" is new
        assert_eq!(configs.len(), 4);
        // New monitor should get sort_order = max(0,5,10) + 1 + index_in_monitors_list
        // "3::New Monitor" is at index 1 in monitors vec, so sort_order = 11 + 1 = 12
        let new = configs.iter().find(|c| c.uid == "3::New Monitor").unwrap();
        assert_eq!(new.sort_order, 12);
    }

    #[test]
    fn test_reconcile_migrated_configs_multiple_monitors() {
        let monitors = vec![
            make_monitor("1", "Dell U2723QE", false),
            make_monitor("2", "LG 27UK850", false),
            make_monitor("builtin", "Built-in Display", true),
        ];
        let mut configs = vec![
            // Two migrated entries with "unknown"
            crate::config::MonitorMetadata {
                uid: "1::unknown".into(),
                api_id: "1".into(),
                api_name: "unknown".into(),
                label: "Left Monitor".into(),
                sort_order: 0,
                hidden: false,
                brightness_mode: crate::config::default_brightness_mode(),
            },
            crate::config::MonitorMetadata {
                uid: "2::unknown".into(),
                api_id: "2".into(),
                api_name: "unknown".into(),
                label: "Right Monitor".into(),
                sort_order: 1,
                hidden: false,
                brightness_mode: crate::config::default_brightness_mode(),
            },
            // One already-known entry (not migrated)
            crate::config::MonitorMetadata {
                uid: "builtin::Built-in Display".into(),
                api_id: "builtin".into(),
                api_name: "Built-in Display".into(),
                label: "MacBook".into(),
                sort_order: 2,
                hidden: false,
                brightness_mode: crate::config::default_brightness_mode(),
            },
        ];
        let changed = reconcile_migrated_configs(&monitors, &mut configs);
        assert!(changed);
        // Migrated entries should be reconciled
        assert_eq!(configs[0].uid, "1::Dell U2723QE");
        assert_eq!(configs[0].api_name, "Dell U2723QE");
        assert_eq!(configs[0].label, "Left Monitor"); // preserved
        assert_eq!(configs[1].uid, "2::LG 27UK850");
        assert_eq!(configs[1].api_name, "LG 27UK850");
        assert_eq!(configs[1].label, "Right Monitor"); // preserved
        // Already-known entry unchanged
        assert_eq!(configs[2].uid, "builtin::Built-in Display");
        assert_eq!(configs[2].label, "MacBook");
    }

    #[test]
    fn test_reconcile_skips_when_uid_already_matches() {
        let monitors = vec![make_monitor("1", "Dell", false)];
        let mut configs = vec![make_meta("1::Dell", "My Dell", 0)];
        let changed = reconcile_migrated_configs(&monitors, &mut configs);
        assert!(!changed); // uid already matches, nothing to do
    }

    /// Verifies resolve_monitor matches by exact id.
    #[test]
    fn test_resolve_monitor_by_id() {
        let monitors = vec![
            make_monitor("1", "Dell U2723QE", false),
            make_monitor("builtin", "Built-in Display", true),
        ];
        let result = resolve_monitor(&monitors, "builtin");
        assert!(result.is_some());
        let (idx, m) = result.unwrap();
        assert_eq!(idx, 1);
        assert_eq!(m.id, "builtin");
    }

    /// Verifies resolve_monitor matches by exact uid.
    #[test]
    fn test_resolve_monitor_by_uid() {
        let monitors = vec![
            make_monitor("1", "Dell U2723QE", false),
            make_monitor("2", "LG 27UK850", false),
        ];
        let result = resolve_monitor(&monitors, "2::LG 27UK850");
        assert!(result.is_some());
        let (idx, m) = result.unwrap();
        assert_eq!(idx, 1);
        assert_eq!(m.name, "LG 27UK850");
    }

    /// Verifies resolve_monitor matches by case-insensitive name substring.
    #[test]
    fn test_resolve_monitor_by_name_substring() {
        let monitors = vec![
            make_monitor("1", "Dell U2723QE", false),
            make_monitor("2", "LG 27UK850", false),
        ];
        let result = resolve_monitor(&monitors, "dell");
        assert!(result.is_some());
        let (idx, _) = result.unwrap();
        assert_eq!(idx, 0);
    }

    /// Verifies resolve_monitor returns None when no match found.
    #[test]
    fn test_resolve_monitor_no_match() {
        let monitors = vec![make_monitor("1", "Dell U2723QE", false)];
        assert!(resolve_monitor(&monitors, "Samsung").is_none());
    }

    /// Verifies resolve_monitor prefers id match over substring match.
    #[test]
    fn test_resolve_monitor_id_takes_priority() {
        let monitors = vec![
            make_monitor("1", "Monitor 1", false),
            make_monitor("2", "Monitor 2", false),
        ];
        // "1" should match id "1", not substring "1" in "Monitor 1"
        let result = resolve_monitor(&monitors, "1");
        assert!(result.is_some());
        let (idx, _) = result.unwrap();
        assert_eq!(idx, 0);
    }

    /// Smoke test: list_all should not panic. May return empty Vec on systems
    /// without displays (e.g. CI), which is fine.
    #[test]
    fn test_core_list_all_does_not_panic() {
        let _ = crate::core::display::list_all();
    }

    /// Verifies the brightness_mode resolver picks up the stored mode by
    /// matching on `api_id` (not `uid`), so monitor renames or display
    /// reorderings don't break dispatch.
    #[test]
    fn test_resolve_brightness_mode_by_api_id() {
        let configs = vec![
            crate::config::MonitorMetadata {
                uid: "1::Dell".into(),
                api_id: "1".into(),
                api_name: "Dell".into(),
                label: "".into(),
                sort_order: 0,
                hidden: false,
                brightness_mode: "overlay".into(),
            },
            crate::config::MonitorMetadata {
                uid: "2::LG".into(),
                api_id: "2".into(),
                api_name: "LG".into(),
                label: "".into(),
                sort_order: 1,
                hidden: false,
                brightness_mode: "ddc".into(),
            },
        ];
        assert_eq!(resolve_brightness_mode(&configs, "1"), "overlay");
        assert_eq!(resolve_brightness_mode(&configs, "2"), "ddc");
    }

    /// Verifies the resolver returns "auto" when the monitor has no config
    /// entry yet (first sighting on a new install).
    #[test]
    fn test_resolve_brightness_mode_defaults_to_auto() {
        let configs: Vec<crate::config::MonitorMetadata> = Vec::new();
        assert_eq!(resolve_brightness_mode(&configs, "1"), "auto");
    }

    /// Verifies the resolver safely collapses an unknown stored mode to
    /// "auto" rather than letting a typo silently disable brightness.
    #[test]
    fn test_resolve_brightness_mode_unknown_collapses_to_auto() {
        let configs = vec![crate::config::MonitorMetadata {
            uid: "1::Dell".into(),
            api_id: "1".into(),
            api_name: "Dell".into(),
            label: "".into(),
            sort_order: 0,
            hidden: false,
            brightness_mode: "moonbeam".into(),
        }];
        assert_eq!(resolve_brightness_mode(&configs, "1"), "auto");
    }

    /// Verifies each mode string maps to the expected route variant.
    #[test]
    fn test_route_for_mode_all_modes() {
        assert_eq!(route_for_mode("auto"), BrightnessRoute::AutoWithOverlayFallback);
        assert_eq!(route_for_mode("ddc"), BrightnessRoute::DdcOnly);
        assert_eq!(route_for_mode("gamma"), BrightnessRoute::GammaOnly);
        assert_eq!(route_for_mode("overlay"), BrightnessRoute::OverlayOnly);
        // Unknown -> auto (safe default).
        assert_eq!(route_for_mode("whatever"), BrightnessRoute::AutoWithOverlayFallback);
        assert_eq!(route_for_mode(""), BrightnessRoute::AutoWithOverlayFallback);
    }

    // --- Tauri-state tests with DISPLAY_DJ_CONFIG_DIR override ---

    use std::panic::{catch_unwind, AssertUnwindSafe};
    use tauri::Manager;

    /// Run `body` with DISPLAY_DJ_CONFIG_DIR pointed at a fresh tempdir.
    /// Restores prior env and cleans up the tempdir even if the body panics.
    fn with_tempdir_config<F: FnOnce(&std::path::Path)>(body: F) {
        let _lock = crate::config::TEST_CONFIG_DIR_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var("DISPLAY_DJ_CONFIG_DIR").ok();
        let tmp = tempfile::tempdir().expect("create tempdir");
        std::env::set_var("DISPLAY_DJ_CONFIG_DIR", tmp.path());
        let result = catch_unwind(AssertUnwindSafe(|| body(tmp.path())));
        match prev {
            Some(v) => std::env::set_var("DISPLAY_DJ_CONFIG_DIR", v),
            None => std::env::remove_var("DISPLAY_DJ_CONFIG_DIR"),
        }
        drop(tmp);
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    /// Build a Tauri test app with our AppState managed (for State-only tests).
    fn make_app() -> tauri::App<tauri::test::MockRuntime> {
        let app = tauri::test::mock_app();
        app.manage(crate::AppState::default());
        app
    }

    /// rename_monitor creates a new MonitorMetadata when one doesn't exist
    /// and persists it through save_preferences_to_disk.
    #[test]
    fn test_rename_monitor_creates_new() {
        with_tempdir_config(|_| {
            let app = make_app();
            let state = app.state::<crate::AppState>();
            rename_monitor(state, "1::Dell U2720Q".to_string(), "MyDell".to_string()).unwrap();
            let st = app.state::<crate::AppState>(); let prefs = st.preferences.lock().unwrap();
            let meta = prefs.monitor_configs.iter().find(|m| m.uid == "1::Dell U2720Q").unwrap();
            assert_eq!(meta.label, "MyDell");
            assert_eq!(meta.api_id, "1");
            assert_eq!(meta.api_name, "Dell U2720Q");
        });
    }

    /// rename_monitor updates the label of an existing MonitorMetadata.
    #[test]
    fn test_rename_monitor_updates_existing() {
        with_tempdir_config(|_| {
            let app = make_app();
            // Seed one entry
            {
                let st = app.state::<crate::AppState>(); let mut prefs = st.preferences.lock().unwrap();
                prefs.monitor_configs.push(crate::config::MonitorMetadata {
                    uid: "1::Dell".to_string(),
                    api_id: "1".to_string(),
                    api_name: "Dell".to_string(),
                    label: "old".to_string(),
                    sort_order: 0,
                    hidden: false,
                    brightness_mode: crate::config::default_brightness_mode(),
                });
            }
            let state = app.state::<crate::AppState>();
            rename_monitor(state, "1::Dell".to_string(), "new".to_string()).unwrap();
            let st = app.state::<crate::AppState>(); let prefs = st.preferences.lock().unwrap();
            let meta = prefs.monitor_configs.iter().find(|m| m.uid == "1::Dell").unwrap();
            assert_eq!(meta.label, "new");
            assert_eq!(prefs.monitor_configs.len(), 1, "should update, not insert");
        });
    }

    /// rename_monitor handles uids without "::" separator gracefully.
    #[test]
    fn test_rename_monitor_no_separator() {
        with_tempdir_config(|_| {
            let app = make_app();
            let state = app.state::<crate::AppState>();
            rename_monitor(state, "builtin".to_string(), "Internal".to_string()).unwrap();
            let st = app.state::<crate::AppState>(); let prefs = st.preferences.lock().unwrap();
            let meta = prefs.monitor_configs.iter().find(|m| m.uid == "builtin").unwrap();
            assert_eq!(meta.api_id, "builtin");
            assert_eq!(meta.api_name, "");
        });
    }

    /// save_monitor_order creates new MonitorMetadata entries when needed.
    #[test]
    fn test_save_monitor_order_creates_new() {
        with_tempdir_config(|_| {
            let app = make_app();
            let orders = vec![("1::Dell".to_string(), 5)];
            let state = app.state::<crate::AppState>();
            save_monitor_order(state, orders).unwrap();
            let st = app.state::<crate::AppState>(); let prefs = st.preferences.lock().unwrap();
            let meta = prefs.monitor_configs.iter().find(|m| m.uid == "1::Dell").unwrap();
            assert_eq!(meta.sort_order, 5);
        });
    }

    /// save_monitor_order updates sort_order on existing entries.
    #[test]
    fn test_save_monitor_order_updates_existing() {
        with_tempdir_config(|_| {
            let app = make_app();
            {
                let st = app.state::<crate::AppState>(); let mut prefs = st.preferences.lock().unwrap();
                prefs.monitor_configs.push(crate::config::MonitorMetadata {
                    uid: "1::Dell".to_string(),
                    api_id: "1".to_string(),
                    api_name: "Dell".to_string(),
                    label: "Dell".to_string(),
                    sort_order: 0,
                    hidden: false,
                    brightness_mode: crate::config::default_brightness_mode(),
                });
            }
            let orders = vec![("1::Dell".to_string(), 99)];
            let state = app.state::<crate::AppState>();
            save_monitor_order(state, orders).unwrap();
            let st = app.state::<crate::AppState>(); let prefs = st.preferences.lock().unwrap();
            let meta = prefs.monitor_configs.iter().find(|m| m.uid == "1::Dell").unwrap();
            assert_eq!(meta.sort_order, 99);
            assert_eq!(prefs.monitor_configs.len(), 1);
        });
    }

    /// set_monitor_visibility toggles the hidden flag on an existing entry.
    #[test]
    fn test_set_monitor_visibility_toggles() {
        with_tempdir_config(|_| {
            let app = make_app();
            {
                let st = app.state::<crate::AppState>(); let mut prefs = st.preferences.lock().unwrap();
                prefs.monitor_configs.push(crate::config::MonitorMetadata {
                    uid: "1::Dell".to_string(),
                    api_id: "1".to_string(),
                    api_name: "Dell".to_string(),
                    label: "Dell".to_string(),
                    sort_order: 0,
                    hidden: false,
                    brightness_mode: crate::config::default_brightness_mode(),
                });
            }
            let state = app.state::<crate::AppState>();
            set_monitor_visibility(state, "1::Dell".to_string(), true).unwrap();
            let st = app.state::<crate::AppState>(); let prefs = st.preferences.lock().unwrap();
            assert!(prefs.monitor_configs.iter().find(|m| m.uid == "1::Dell").unwrap().hidden);
        });
    }

    /// set_monitor_visibility is a no-op for nonexistent uids (no error).
    #[test]
    fn test_set_monitor_visibility_unknown_uid() {
        with_tempdir_config(|_| {
            let app = make_app();
            let state = app.state::<crate::AppState>();
            // Should not error even when uid doesn't exist.
            set_monitor_visibility(state, "nonexistent::foo".to_string(), true).unwrap();
            let st = app.state::<crate::AppState>(); let prefs = st.preferences.lock().unwrap();
            assert!(prefs.monitor_configs.is_empty());
        });
    }

    /// get_monitors smoke test — returns Ok regardless of platform.
    #[test]
    fn test_get_monitors_smoke() {
        with_tempdir_config(|_| {
            let app = make_app();
            let state = app.state::<crate::AppState>();
            let result = tauri::async_runtime::block_on(get_monitors(state));
            // May return empty vec on CI; just verify Ok.
            assert!(result.is_ok());
        });
    }

    /// set_contrast smoke test — completes (may error if monitor not found).
    #[test]
    fn test_set_contrast_smoke() {
        with_tempdir_config(|_| {
            let app = make_app();
            let state = app.state::<crate::AppState>();
            let result = tauri::async_runtime::block_on(set_contrast(
                state,
                "1".to_string(),
                50,
            ));
            // Either Ok (hardware succeeded) or Err (monitor not found) — both acceptable.
            let _ = result;
        });
    }

    /// set_all_contrast smoke test — returns Ok regardless of HW success.
    #[test]
    fn test_set_all_contrast_smoke() {
        with_tempdir_config(|_| {
            let app = make_app();
            let state = app.state::<crate::AppState>();
            let result = tauri::async_runtime::block_on(set_all_contrast(state, 50));
            assert!(result.is_ok());
        });
    }
}
