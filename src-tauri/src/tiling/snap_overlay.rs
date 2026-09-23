//! Tauri WebView overlays used by Windows and X11 Tile Snap.
//!
//! One transparent, click-through window covers each display work area. The
//! local HTML page draws all enabled drop targets plus the active target
//! preview, avoiding a separate native window for every zone.

use super::{Rect, SnapZone, SnapZoneVisualKind};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};

const OVERLAY_EVENT_PREFIX: &str = "set-tile-snap-overlay";
const OVERLAY_LABEL_PREFIX: &str = "tile-snap-overlay";
const MAIN_THREAD_WINDOW_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Rectangle relative to one overlay window's display work area.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// One colored drop-zone indicator rendered by the overlay page.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayZone {
    rect: OverlayRect,
    kind: SnapZoneVisualKind,
}

/// Complete render state for one display overlay.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayState {
    zones: Vec<OverlayZone>,
    preview: Option<OverlayRect>,
}

/// Owns the reusable per-display overlay windows for one Tile Snap monitor.
pub(super) struct TileSnapOverlay {
    app: AppHandle,
    displays: Vec<Rect>,
    scale_factors: Vec<f64>,
    zones: Vec<Vec<OverlayZone>>,
    window_count: usize,
}

impl TileSnapOverlay {
    /// Create an empty overlay manager. Windows remain lazy until a drag starts.
    pub(super) fn new(app: AppHandle) -> Self {
        Self {
            app,
            displays: Vec::new(),
            scale_factors: Vec::new(),
            zones: Vec::new(),
            window_count: 0,
        }
    }

    /// Show all enabled Tile Snap targets across every connected display.
    pub(super) fn show_zones(
        &mut self,
        displays: &[Rect],
        snap_zones: &[SnapZone],
        scale_factors: &[f64],
    ) -> Result<(), String> {
        self.displays = displays.to_vec();
        self.scale_factors = displays
            .iter()
            .enumerate()
            .map(|(index, _)| normalized_scale_factor(scale_factors.get(index).copied()))
            .collect();
        self.zones = displays
            .iter()
            .enumerate()
            .map(|(display_index, display)| {
                let scale_factor = self.scale_factors[display_index];
                snap_zones
                    .iter()
                    .filter(|zone| zone.display_index == display_index)
                    .map(|zone| OverlayZone {
                        rect: relative_css_rect(&zone.rect, display, scale_factor),
                        kind: zone.visual_kind,
                    })
                    .collect()
            })
            .collect();

        let mut created_window = false;
        for (display_index, display) in displays.iter().enumerate() {
            created_window |= self.ensure_window(display_index, display)?;
        }

        // The local page is tiny, but its event listener still installs after
        // WebView creation. Give first-use windows one short load interval so
        // the initial zone payload cannot race page startup.
        if created_window {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        for display_index in 0..displays.len() {
            self.emit_state(display_index, None)?;
            if let Some(window) = self.app.get_webview_window(&overlay_label(display_index)) {
                window
                    .show()
                    .map_err(|error| format!("tile_snap_overlay: show failed: {error}"))?;
            }
        }
        for display_index in displays.len()..self.window_count {
            if let Some(window) = self.app.get_webview_window(&overlay_label(display_index)) {
                let _ = window.hide();
            }
        }
        Ok(())
    }

    /// Draw the active layout preview on one display and clear it elsewhere.
    pub(super) fn show_preview(&self, display_index: usize, target: &Rect) -> Result<(), String> {
        for index in 0..self.displays.len() {
            let preview = if index == display_index {
                self.displays.get(index).map(|display| {
                    let scale_factor =
                        normalized_scale_factor(self.scale_factors.get(index).copied());
                    relative_css_rect(target, display, scale_factor)
                })
            } else {
                None
            };
            self.emit_state(index, preview)?;
        }
        Ok(())
    }

    /// Clear the active preview while leaving drop-zone indicators visible.
    pub(super) fn clear_preview(&self) -> Result<(), String> {
        for display_index in 0..self.displays.len() {
            self.emit_state(display_index, None)?;
        }
        Ok(())
    }

    /// Hide all Tile Snap overlays after a drag finishes or gets cancelled.
    pub(super) fn hide(&self) {
        for display_index in 0..self.window_count {
            if let Some(window) = self.app.get_webview_window(&overlay_label(display_index)) {
                let _ = window.hide();
            }
        }
    }

    /// Create or reposition one transparent overlay window.
    ///
    /// Returns `true` only when a new WebView was created.
    fn ensure_window(&mut self, display_index: usize, display: &Rect) -> Result<bool, String> {
        let label = overlay_label(display_index);
        let (window, created) = match self.app.get_webview_window(&label) {
            Some(window) => (window, false),
            None => {
                let app = self.app.clone();
                let main_thread_app = app.clone();
                let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
                app.run_on_main_thread(move || {
                    let result = create_overlay_window(&main_thread_app, display_index);
                    let _ = result_sender.send(result);
                })
                .map_err(|error| {
                    format!("tile_snap_overlay: main-thread dispatch failed: {error}")
                })?;
                result_receiver
                    .recv_timeout(MAIN_THREAD_WINDOW_TIMEOUT)
                    .map_err(|error| {
                        format!("tile_snap_overlay: main-thread build timed out: {error}")
                    })??;
                let window = self.app.get_webview_window(&label).ok_or_else(|| {
                    "tile_snap_overlay: built window was not registered".to_string()
                })?;
                (window, true)
            }
        };

        let width = display.width.round().max(1.0) as u32;
        let height = display.height.round().max(1.0) as u32;
        window
            .set_position(PhysicalPosition::new(
                display.x.round() as i32,
                display.y.round() as i32,
            ))
            .map_err(|error| format!("tile_snap_overlay: set_position failed: {error}"))?;
        window
            .set_size(PhysicalSize::new(width, height))
            .map_err(|error| format!("tile_snap_overlay: set_size failed: {error}"))?;
        self.window_count = self.window_count.max(display_index + 1);
        Ok(created)
    }

    /// Emit one display's current zone and preview state.
    fn emit_state(&self, display_index: usize, preview: Option<OverlayRect>) -> Result<(), String> {
        let zones = self.zones.get(display_index).cloned().unwrap_or_default();
        let event = overlay_event(display_index);
        self.app
            .emit_to(
                overlay_label(display_index),
                &event,
                OverlayState { zones, preview },
            )
            .map_err(|error| format!("tile_snap_overlay: emit failed: {error}"))
    }
}

/// Creates one Tile Snap WebView on Tauri's main thread.
fn create_overlay_window(app: &AppHandle, display_index: usize) -> Result<(), String> {
    let label = overlay_label(display_index);
    let page = overlay_page_url(display_index);
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(page.into()))
        .title("Display DJ Tile Snap")
        .inner_size(1.0, 1.0)
        .position(0.0, 0.0)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .focused(false)
        .visible(false);
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let builder = builder.transparent(true);
    let window = builder
        .build()
        .map_err(|error| format!("tile_snap_overlay: build failed: {error}"))?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| format!("tile_snap_overlay: click-through setup failed: {error}"))
}

/// Build a stable Tauri label for one display overlay.
fn overlay_label(display_index: usize) -> String {
    format!("{OVERLAY_LABEL_PREFIX}-{display_index}")
}

/// Build the event name consumed by one display overlay.
fn overlay_event(display_index: usize) -> String {
    format!("{OVERLAY_EVENT_PREFIX}-{display_index}")
}

/// Build the local page URL with the display-scoped event name.
fn overlay_page_url(display_index: usize) -> String {
    format!(
        "tile-snap-overlay.html?event={}",
        overlay_event(display_index)
    )
}

/// Return a safe CSS-to-physical scale, defaulting invalid values to 1.
fn normalized_scale_factor(scale_factor: Option<f64>) -> f64 {
    match scale_factor {
        Some(scale_factor) if scale_factor.is_finite() && scale_factor > 0.0 => scale_factor,
        _ => 1.0,
    }
}

/// Convert physical display coordinates into WebView CSS pixels.
fn relative_css_rect(rect: &Rect, display: &Rect, scale_factor: f64) -> OverlayRect {
    let scale_factor = normalized_scale_factor(Some(scale_factor));
    OverlayRect {
        x: (rect.x - display.x) / scale_factor,
        y: (rect.y - display.y) / scale_factor,
        width: rect.width / scale_factor,
        height: rect.height / scale_factor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each display must receive an isolated event stream.
    #[test]
    fn overlay_events_are_scoped_to_display() {
        assert_eq!(overlay_event(0), "set-tile-snap-overlay-0");
        assert_eq!(overlay_event(1), "set-tile-snap-overlay-1");
        assert_ne!(overlay_event(0), overlay_event(1));
    }

    /// Each page receives the event name for its own overlay window.
    #[test]
    fn overlay_page_url_carries_scoped_event() {
        assert_eq!(
            overlay_page_url(2),
            "tile-snap-overlay.html?event=set-tile-snap-overlay-2"
        );
    }

    /// Global screen coordinates become local coordinates without changing size.
    #[test]
    fn relative_css_rect_offsets_by_display_origin_at_default_scale() {
        let display = Rect {
            x: -1920.0,
            y: 40.0,
            width: 1920.0,
            height: 1040.0,
        };
        let rect = Rect {
            x: -960.0,
            y: 60.0,
            width: 960.0,
            height: 520.0,
        };

        let local = relative_css_rect(&rect, &display, 1.0);

        assert_eq!(local.x, 960.0);
        assert_eq!(local.y, 20.0);
        assert_eq!(local.width, 960.0);
        assert_eq!(local.height, 520.0);
    }

    /// DPI-scaled displays keep bottom zones inside the WebView viewport.
    #[test]
    fn relative_css_rect_converts_physical_pixels_to_css_pixels() {
        let display = Rect {
            x: 1920.0,
            y: 0.0,
            width: 2560.0,
            height: 1392.0,
        };
        let bottom_zone = Rect {
            x: 2880.0,
            y: 1372.0,
            width: 640.0,
            height: 20.0,
        };

        let local = relative_css_rect(&bottom_zone, &display, 1.25);

        assert_eq!(local.x, 768.0);
        assert_eq!(local.y, 1097.6);
        assert_eq!(local.width, 512.0);
        assert_eq!(local.height, 16.0);
        assert!(local.y + local.height <= display.height / 1.25);
    }

    /// Invalid monitor scale values preserve the unscaled overlay behavior.
    #[test]
    fn relative_css_rect_defaults_invalid_scale_to_one() {
        let display = Rect {
            x: 100.0,
            y: 200.0,
            width: 800.0,
            height: 600.0,
        };
        let rect = Rect {
            x: 300.0,
            y: 400.0,
            width: 400.0,
            height: 300.0,
        };

        let local = relative_css_rect(&rect, &display, 0.0);

        assert_eq!(local.x, 200.0);
        assert_eq!(local.y, 200.0);
        assert_eq!(local.width, 400.0);
        assert_eq!(local.height, 300.0);
    }
}
