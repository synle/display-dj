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

const OVERLAY_EVENT: &str = "set-tile-snap-overlay";
const OVERLAY_LABEL_PREFIX: &str = "tile-snap-overlay";

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
    zones: Vec<Vec<OverlayZone>>,
    window_count: usize,
}

impl TileSnapOverlay {
    /// Create an empty overlay manager. Windows remain lazy until a drag starts.
    pub(super) fn new(app: AppHandle) -> Self {
        Self {
            app,
            displays: Vec::new(),
            zones: Vec::new(),
            window_count: 0,
        }
    }

    /// Show all enabled Tile Snap targets across every connected display.
    pub(super) fn show_zones(
        &mut self,
        displays: &[Rect],
        snap_zones: &[SnapZone],
    ) -> Result<(), String> {
        self.displays = displays.to_vec();
        self.zones = displays
            .iter()
            .enumerate()
            .map(|(display_index, display)| {
                snap_zones
                    .iter()
                    .filter(|zone| zone.display_index == display_index)
                    .map(|zone| OverlayZone {
                        rect: relative_rect(&zone.rect, display),
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
                self.displays
                    .get(index)
                    .map(|display| relative_rect(target, display))
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
                let builder = WebviewWindowBuilder::new(
                    &self.app,
                    &label,
                    WebviewUrl::App("tile-snap-overlay.html".into()),
                )
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
                window.set_ignore_cursor_events(true).map_err(|error| {
                    format!("tile_snap_overlay: click-through setup failed: {error}")
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
        self.app
            .emit_to(
                overlay_label(display_index),
                OVERLAY_EVENT,
                OverlayState { zones, preview },
            )
            .map_err(|error| format!("tile_snap_overlay: emit failed: {error}"))
    }
}

/// Build a stable Tauri label for one display overlay.
fn overlay_label(display_index: usize) -> String {
    format!("{OVERLAY_LABEL_PREFIX}-{display_index}")
}

/// Convert a global screen rectangle into display-local overlay coordinates.
fn relative_rect(rect: &Rect, display: &Rect) -> OverlayRect {
    OverlayRect {
        x: rect.x - display.x,
        y: rect.y - display.y,
        width: rect.width,
        height: rect.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Global screen coordinates become local coordinates without changing size.
    #[test]
    fn relative_rect_offsets_by_display_origin() {
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

        let local = relative_rect(&rect, &display);

        assert_eq!(local.x, 960.0);
        assert_eq!(local.y, 20.0);
        assert_eq!(local.width, 960.0);
        assert_eq!(local.height, 520.0);
    }
}
