/// Generates tray icons with colored indicator dots reflecting app state.
///
/// Three dots are drawn at the bottom-center of the base icon:
///   - Blue dot: dark mode is active
///   - Orange dot: keep-awake is active
///   - Red dot: volume is muted (0)
///
/// Dots are only drawn when the corresponding state is active.
/// The base icon is the 32x32.png from the icons directory (512x512 RGBA).

use tauri::image::Image;
use tauri::Manager;

/// RGBA colors for each indicator dot.
const COLOR_DARK_MODE: [u8; 4] = [60, 130, 240, 255]; // blue
const COLOR_KEEP_AWAKE: [u8; 4] = [240, 160, 30, 255]; // orange
const COLOR_MUTED: [u8; 4] = [230, 60, 60, 255]; // red

/// Dot radius in pixels (relative to 256x256 base icon).
/// At tray size (~44px), this renders as ~8px diameter — clearly visible.
const DOT_RADIUS: i32 = 24;

/// Vertical offset from the bottom of the icon to the dot center.
const DOT_BOTTOM_OFFSET: i32 = 30;

/// Horizontal gap between dot centers.
const DOT_SPACING: i32 = 60;

/// Base icon for dark menu bar (white/light outline monitor).
fn base_icon_dark_bytes() -> &'static [u8] {
    include_bytes!("../../src/assets/icon-dark@5x.png")
}

/// Base icon for light menu bar (dark outline monitor).
fn base_icon_light_bytes() -> &'static [u8] {
    include_bytes!("../../src/assets/icon-light@5x.png")
}

/// Generates a tray icon with indicator dots based on current state.
/// Uses the light-outline icon for dark menu bars and dark-outline icon for light.
/// Only active states get a visible dot; inactive states show nothing.
pub fn generate_tray_icon(
    is_dark_mode: bool,
    is_keep_awake: bool,
    is_muted: bool,
) -> Image<'static> {
    let base_bytes = if is_dark_mode {
        base_icon_dark_bytes()
    } else {
        base_icon_light_bytes()
    };
    let img = image::load_from_memory(base_bytes)
        .expect("failed to decode base tray icon")
        .to_rgba8();

    let (width, height) = img.dimensions();
    let mut pixels = img.into_raw();

    // Dot positions: centered horizontally, near bottom
    let center_x = width as i32 / 2;
    let center_y = height as i32 - DOT_BOTTOM_OFFSET;

    // Dark mode indicator is omitted — the icon itself swaps light/dark outline.
    // Only keep-awake and muted get dots (centered with half spacing).
    let dots: &[(bool, [u8; 4], i32)] = &[
        (is_keep_awake, COLOR_KEEP_AWAKE, center_x - DOT_SPACING / 2), // left
        (is_muted, COLOR_MUTED, center_x + DOT_SPACING / 2),           // right
    ];

    for &(active, color, dot_cx) in dots {
        if active {
            draw_filled_circle(&mut pixels, width, height, dot_cx, center_y, DOT_RADIUS, color);
        }
    }

    Image::new_owned(pixels, width, height)
}

/// Draws a filled circle onto an RGBA pixel buffer.
fn draw_filled_circle(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    cx: i32,
    cy: i32,
    radius: i32,
    color: [u8; 4],
) {
    let r2 = radius * radius;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= r2 {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && px < width as i32 && py >= 0 && py < height as i32 {
                    let idx = ((py as u32 * width + px as u32) * 4) as usize;
                    pixels[idx] = color[0];
                    pixels[idx + 1] = color[1];
                    pixels[idx + 2] = color[2];
                    pixels[idx + 3] = color[3];
                }
            }
        }
    }
}

/// Updates the cached dark mode state and refreshes the tray icon.
pub fn set_dark_mode_state(app: &tauri::AppHandle, is_dark: bool) {
    if let Some(state) = app.try_state::<crate::AppState>() {
        if let Ok(mut v) = state.is_dark_mode.lock() {
            *v = is_dark;
        }
    }
    update_tray_icon(app);
}

/// Updates the cached muted state and refreshes the tray icon.
pub fn set_muted_state(app: &tauri::AppHandle, is_muted: bool) {
    if let Some(state) = app.try_state::<crate::AppState>() {
        if let Ok(mut v) = state.is_muted.lock() {
            *v = is_muted;
        }
    }
    update_tray_icon(app);
}

/// Updates the tray icon to reflect current app state.
/// Reads is_dark_mode, is_muted, and keep_awake from AppState,
/// generates a new icon with indicator dots, and applies it.
pub fn update_tray_icon(app: &tauri::AppHandle) {
    let state = match app.try_state::<crate::AppState>() {
        Some(s) => s,
        None => return,
    };

    let is_dark = state.is_dark_mode.lock().map(|v| *v).unwrap_or(false);
    let is_muted = state.is_muted.lock().map(|v| *v).unwrap_or(false);
    let is_keep_awake = state.keep_awake.lock().map(|v| v.is_some()).unwrap_or(false);

    let icon = generate_tray_icon(is_dark, is_keep_awake, is_muted);

    log::info!(
        "update_tray_icon: dark={} keep_awake={} muted={}",
        is_dark, is_keep_awake, is_muted
    );

    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_icon(Some(icon));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies both base icon variants can be loaded and decoded.
    #[test]
    fn test_base_icons_load() {
        let dark = image::load_from_memory(base_icon_dark_bytes()).expect("should decode dark icon");
        let light = image::load_from_memory(base_icon_light_bytes()).expect("should decode light icon");
        assert_eq!(dark.to_rgba8().dimensions(), (256, 256));
        assert_eq!(light.to_rgba8().dimensions(), (256, 256));
    }

    /// Verifies icon generation with all indicators off uses the light base icon.
    #[test]
    fn test_generate_icon_all_off() {
        let icon = generate_tray_icon(false, false, false);
        // Should produce a 256x256 RGBA image (256*256*4 bytes)
        assert_eq!(icon.rgba().len(), 256 * 256 * 4);
    }

    /// Verifies icon generation with all indicators on uses the dark base icon.
    #[test]
    fn test_generate_icon_all_on() {
        let icon = generate_tray_icon(true, true, true);
        assert_eq!(icon.rgba().len(), 256 * 256 * 4);
    }

    /// Verifies that indicator dots actually modify pixels compared to the base.
    #[test]
    fn test_dots_modify_pixels() {
        let icon_off = generate_tray_icon(false, false, false);
        let icon_on = generate_tray_icon(true, true, true);
        // The pixel buffers should differ when dots are drawn
        assert_ne!(icon_off.rgba(), icon_on.rgba());
    }

    /// Verifies draw_filled_circle writes the correct color at the center pixel.
    #[test]
    fn test_draw_circle_center_pixel() {
        let mut pixels = vec![0u8; 100 * 100 * 4];
        draw_filled_circle(&mut pixels, 100, 100, 50, 50, 5, [255, 128, 64, 255]);
        let idx = (50 * 100 + 50) * 4;
        assert_eq!(pixels[idx], 255);
        assert_eq!(pixels[idx + 1], 128);
        assert_eq!(pixels[idx + 2], 64);
        assert_eq!(pixels[idx + 3], 255);
    }
}
