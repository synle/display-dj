//! Soft-overlay brightness fallback.
//!
//! Some external panels (most prominently the Samsung Smart Monitor M7/M8 family
//! over USB-C on Intel Iris Xe) ignore DDC/CI brightness writes and have their
//! GDI `SetDeviceGammaRamp` calls silently rejected by the Intel iGPU driver.
//! For those displays there is no hardware path that can dim the panel —
//! every existing slider becomes a no-op.
//!
//! The industry workaround (used by Twinkle Tray, Lunar, Win10_BrightnessSlider)
//! is a software overlay: a per-monitor, always-on-top, click-through, fully
//! transparent window whose background is opaque black. The OS compositor
//! blends the overlay with everything underneath, so raising the overlay
//! opacity from 0.0 to ~0.8 produces the same visual dim as turning the
//! backlight down. It bypasses the hardware entirely, so it works on any
//! GPU/driver combination.
//!
//! This module owns the lifecycle of one Tauri `WebviewWindow` per external
//! monitor (label `overlay-{monitor_id}`). Windows are created lazily on the
//! first overlay request for a given monitor, sized to the monitor's physical
//! rect, and kept alive until [`destroy_overlay`] is called (e.g. on unplug
//! or when the user switches back to a hardware-only mode). The window's
//! HTML lives at `src/overlay.html` — a tiny page that listens for
//! `set-overlay-alpha` events and updates `document.body.style.opacity`
//! accordingly.
//!
//! ## Platform status
//!
//! - **Windows**: fully functional. `core::DisplayInfo.monitor_rect` is
//!   populated from `MONITORINFOEXW.rcMonitor` so the overlay can be sized
//!   and positioned correctly.
//! - **macOS / Linux**: the Tauri window itself spawns fine, but
//!   `monitor_rect` is currently always `None` (see TODOs in `core::macos`
//!   and `core::linux`). [`set_overlay_brightness`] silently no-ops on
//!   those platforms until those TODOs are filled in.

use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};

/// Build the Tauri window label for a given monitor id.
///
/// One overlay window per monitor, keyed by the raw `core::DisplayInfo.id`
/// (e.g. `"1"`, `"2"`). Built-in displays do not get overlays — they dim
/// natively via WMI / DisplayServices / sysfs backlight.
///
/// # Arguments
/// * `monitor_id` - The raw `core::DisplayInfo.id` of the monitor.
///
/// # Returns
/// The Tauri window label, e.g. `"overlay-1"`.
fn overlay_label(monitor_id: &str) -> String {
    format!("overlay-{}", monitor_id)
}

/// Show or update the soft-overlay brightness for a single external monitor.
///
/// Creates the overlay `WebviewWindow` lazily if it does not yet exist,
/// positions it over the monitor's physical rect, then emits a
/// `set-overlay-alpha` event whose payload is the target opacity `alpha`.
/// Conceptually: `alpha = 1.0 - brightness_pct / 100.0`. At
/// `brightness_pct == 100` the overlay is fully transparent — so we hide
/// the window instead of leaving an invisible always-on-top window
/// stealing input events at the WM level.
///
/// This function is a no-op (returns `Ok(())`) when `monitor_rect` is
/// `None`. That is the current state on macOS and Linux until the platform
/// TODOs in `core::macos` and `core::linux` are filled in.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` used to create / fetch the overlay window.
/// * `monitor_id` - The raw `core::DisplayInfo.id` of the target monitor.
/// * `monitor_rect` - `(left, top, width, height)` in global physical pixels.
///   Pass `None` to skip (overlay is unsupported on this platform / monitor).
/// * `brightness_pct` - Target brightness 0..=100. Values >= 100 hide the
///   overlay; values < 100 show it and set opacity accordingly.
///
/// # Returns
/// `Ok(())` on success. `Err(String)` if the Tauri window creation, move,
/// resize, or show call fails.
pub fn set_overlay_brightness<R: tauri::Runtime>(
    app: &AppHandle<R>,
    monitor_id: &str,
    monitor_rect: Option<(i32, i32, i32, i32)>,
    brightness_pct: u32,
) -> Result<(), String> {
    let rect = match monitor_rect {
        Some(r) => r,
        None => {
            log::info!(
                "overlay: skipping monitor {} — no monitor_rect (platform stub)",
                monitor_id,
            );
            return Ok(());
        }
    };

    let (left, top, width, height) = rect;
    if width <= 0 || height <= 0 {
        return Err(format!(
            "overlay: invalid monitor_rect for {}: width={} height={}",
            monitor_id, width, height,
        ));
    }

    let label = overlay_label(monitor_id);

    // Fast path: window already exists. Just move/resize (in case the
    // monitor was repositioned) and emit the new alpha.
    if let Some(window) = app.get_webview_window(&label) {
        if brightness_pct >= 100 {
            let _ = window.hide();
            return Ok(());
        }
        // Re-apply position/size in case the user changed display arrangement
        // between calls. Use logical units; the OS handles DPI scaling.
        window
            .set_position(LogicalPosition::new(left as f64, top as f64))
            .map_err(|e| format!("overlay: set_position failed: {}", e))?;
        window
            .set_size(LogicalSize::new(width as f64, height as f64))
            .map_err(|e| format!("overlay: set_size failed: {}", e))?;
        window
            .show()
            .map_err(|e| format!("overlay: show failed: {}", e))?;
        emit_alpha(app, monitor_id, brightness_pct);
        return Ok(());
    }

    if brightness_pct >= 100 {
        // Nothing to do — overlay doesn't exist yet and brightness is at max.
        return Ok(());
    }

    // Lazy creation path. The Tauri `WebviewWindowBuilder` flags here are
    // load-bearing for the click-through dimming effect:
    // - decorations(false), transparent(true), shadow(false): no chrome.
    // - always_on_top(true): stay above the dimmed content.
    // - skip_taskbar(true), focused(false): never steal focus or taskbar slot.
    // - resizable(false), maximizable(false), minimizable(false): user can't
    //   interact with it via the WM.
    // - visible(false): we explicitly show after positioning so the window
    //   never flashes at (0,0).
    // `.transparent(true)` on macOS requires the `macos-private-api` Tauri
    // feature; the overlay fallback is Windows-first (per the user request)
    // so we only call it on platforms where the public API allows it. macOS
    // / Linux still spawn the window — opacity transitions just render
    // against the default window background instead of compositing through.
    // The macOS rect TODO blocks the overlay path on macOS anyway, so this
    // gap is paper-only until that TODO is filled in.
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("overlay.html".into()))
        .title("Display DJ Overlay")
        .inner_size(width as f64, height as f64)
        .position(left as f64, top as f64)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .focused(false)
        .visible(false);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.transparent(true);
    let window = builder
        .build()
        .map_err(|e| format!("overlay: build failed: {}", e))?;

    // Click-through: the overlay must not consume mouse events, otherwise the
    // user can't interact with whatever is underneath it.
    window
        .set_ignore_cursor_events(true)
        .map_err(|e| format!("overlay: set_ignore_cursor_events failed: {}", e))?;

    window
        .show()
        .map_err(|e| format!("overlay: show failed: {}", e))?;

    emit_alpha(app, monitor_id, brightness_pct);
    log::info!(
        "overlay: created window {} at ({},{}) {}x{} brightness={}%",
        label, left, top, width, height, brightness_pct,
    );
    Ok(())
}

/// Tear down the overlay window for a single monitor, if one exists.
///
/// Called on monitor unplug, when the user switches the per-monitor
/// `brightnessMode` away from `"overlay"`/`"auto"`, or when the auto path
/// succeeds via DDC/gamma (so a previously-shown overlay clears instead of
/// double-dimming).
///
/// # Arguments
/// * `app` - Tauri `AppHandle` used to look up the overlay window by label.
/// * `monitor_id` - The raw `core::DisplayInfo.id` whose overlay should close.
///
/// # Returns
/// `Ok(())` whether the window existed or not. `Err(String)` only when the
/// close call itself failed (rare — usually means the window was already
/// torn down by Tauri).
pub fn destroy_overlay<R: tauri::Runtime>(app: &AppHandle<R>, monitor_id: &str) -> Result<(), String> {
    let label = overlay_label(monitor_id);
    if let Some(window) = app.get_webview_window(&label) {
        window
            .close()
            .map_err(|e| format!("overlay: close failed for {}: {}", label, e))?;
        log::info!("overlay: destroyed window {}", label);
    }
    Ok(())
}

/// Compute and emit the alpha value to the overlay window.
///
/// `alpha = 1.0 - brightness_pct / 100.0`, clamped to `[0.0, 0.9]` so the
/// user can never lose the panel entirely (a 100% opaque overlay would make
/// the monitor unusable until brightness was raised again).
///
/// Emits via the global app event channel, scoped by label so each overlay
/// only updates its own opacity.
fn emit_alpha<R: tauri::Runtime>(app: &AppHandle<R>, monitor_id: &str, brightness_pct: u32) {
    let alpha = compute_alpha(brightness_pct);
    let label = overlay_label(monitor_id);
    if let Err(e) = app.emit_to(label.as_str(), "set-overlay-alpha", alpha) {
        log::warn!(
            "overlay: emit set-overlay-alpha failed for {}: {}",
            monitor_id, e,
        );
    }
}

/// Pure helper: convert a brightness percentage to an overlay alpha value.
///
/// `alpha = 1.0 - brightness_pct / 100.0`, clamped to `[0.0, 0.9]`. The
/// upper clamp prevents the user from making a panel completely opaque
/// (which would make recovery via the slider impossible because the
/// monitor would be entirely black).
///
/// Extracted so it's unit-testable without a Tauri context.
///
/// # Arguments
/// * `brightness_pct` - Target brightness 0..=100 (values above 100 saturate to 0.0).
///
/// # Returns
/// Overlay opacity in the range `[0.0, 0.9]`.
pub fn compute_alpha(brightness_pct: u32) -> f64 {
    let pct = brightness_pct.min(100) as f64;
    let raw = 1.0 - pct / 100.0;
    raw.clamp(0.0, 0.9)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the alpha formula at the canonical breakpoints.
    #[test]
    fn test_compute_alpha_canonical_points() {
        assert_eq!(compute_alpha(100), 0.0);
        assert_eq!(compute_alpha(50), 0.5);
        // 0% brightness clamps to 0.9 (never fully opaque so the slider can recover)
        assert_eq!(compute_alpha(0), 0.9);
    }

    /// Verifies values above 100 saturate to 0.0 (no negative alpha leak).
    #[test]
    fn test_compute_alpha_oversaturated() {
        assert_eq!(compute_alpha(200), 0.0);
        assert_eq!(compute_alpha(u32::MAX), 0.0);
    }

    /// Verifies the label format is stable — the frontend overlay.html
    /// relies on matching this exact prefix for event listening.
    #[test]
    fn test_overlay_label_format() {
        assert_eq!(overlay_label("1"), "overlay-1");
        assert_eq!(overlay_label("builtin"), "overlay-builtin");
    }

    /// set_overlay_brightness with `monitor_rect = None` is the
    /// no-op platform-stub path (current macOS/Linux behavior). Must
    /// return Ok(()) without attempting any Tauri window calls.
    #[test]
    fn test_set_overlay_brightness_none_rect_is_noop() {
        let app = tauri::test::mock_app();
        let result = set_overlay_brightness(app.handle(), "1", None, 50);
        assert!(result.is_ok(), "None rect should be a silent no-op");
    }

    /// set_overlay_brightness must reject a degenerate rect (zero or
    /// negative width/height) — those would crash the window builder
    /// downstream on real platforms.
    #[test]
    fn test_set_overlay_brightness_invalid_rect_errors() {
        let app = tauri::test::mock_app();
        let r1 = set_overlay_brightness(app.handle(), "1", Some((0, 0, 0, 600)), 50);
        let r2 = set_overlay_brightness(app.handle(), "1", Some((0, 0, 1920, 0)), 50);
        let r3 = set_overlay_brightness(app.handle(), "1", Some((0, 0, -10, 600)), 50);
        assert!(r1.is_err());
        assert!(r2.is_err());
        assert!(r3.is_err());
    }

    /// set_overlay_brightness at brightness >= 100 with no pre-existing
    /// window short-circuits to Ok(()) — we don't create a transparent
    /// window only to hide it.
    #[test]
    fn test_set_overlay_brightness_full_brightness_no_window() {
        let app = tauri::test::mock_app();
        let result = set_overlay_brightness(app.handle(), "9999", Some((0, 0, 1920, 1080)), 100);
        assert!(result.is_ok());
        // Also at >100 (saturating).
        let result = set_overlay_brightness(app.handle(), "9999", Some((0, 0, 1920, 1080)), 200);
        assert!(result.is_ok());
    }

    /// destroy_overlay on a missing window must succeed silently —
    /// the unplug path calls it without checking whether an overlay
    /// was ever created.
    #[test]
    fn test_destroy_overlay_missing_window_is_ok() {
        let app = tauri::test::mock_app();
        let result = destroy_overlay(app.handle(), "nonexistent");
        assert!(result.is_ok());
    }
}
