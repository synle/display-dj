/// Generates tray icons programmatically.
///
/// All layout is defined as percentages of a 100x100 grid, then scaled
/// to ICON_SIZE (128x128). This makes it easy to tweak proportions
/// without worrying about pixel math.
///
/// The icon is a monitor shape:
///   - Outer rectangle with border (white on dark, black on light)
///   - Light blue fill inside
///   - When keep-awake is active, a smaller accent rectangle inside
///
/// No PNG assets are used — everything is drawn from code.

use tauri::image::Image;
use tauri::Manager;

/// Output icon dimensions in pixels.
const ICON_SIZE: u32 = 128;

// --- Layout as percentages (0–100) of icon size ---

/// Outer padding from edge of icon to the monitor rectangle.
const MARGIN_PCT: f32 = 6.0;

/// Border thickness of the monitor rectangle.
const BORDER_PCT: f32 = 8.0;

/// Inset of the mute X from the icon edge.
const MUTE_INSET_PCT: f32 = 14.0;

/// Thickness of the mute X lines.
const MUTE_X_THICKNESS_PCT: f32 = 14.0;

/// Converts a percentage (0–100) to pixels at ICON_SIZE.
fn pct(p: f32) -> i32 {
    (p / 100.0 * ICON_SIZE as f32).round() as i32
}

/// Generates a tray icon based on current state.
///
/// Default: border + inverse fill (black border/white fill on light, white border/black fill on dark).
/// Keep-awake ON: fill becomes blue.
/// Muted: smaller red rectangle inside.
/// Both: blue fill + red inner rectangle.
pub fn generate_tray_icon(
    is_dark_mode: bool,
    is_keep_awake: bool,
    is_muted: bool,
) -> Image<'static> {
    let mut pixels = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    let border_color: [u8; 4] = if is_dark_mode {
        [255, 255, 255, 255] // white border — visible on dark menu bar
    } else {
        [0, 0, 0, 255] // black border — visible on light menu bar
    };

    // Fill: blue when keep-awake is on (darker blue on dark bar, lighter on light bar),
    // otherwise inverse of border
    let fill_color: [u8; 4] = if is_keep_awake {
        if is_dark_mode {
            [30, 100, 180, 255] // deep blue — contrasts with white border on dark bar
        } else {
            [135, 206, 235, 255] // sky blue — contrasts with black border on light bar
        }
    } else if is_dark_mode {
        [0, 0, 0, 255] // black fill on dark menu bar
    } else {
        [255, 255, 255, 255] // white fill on light menu bar
    };

    let margin = pct(MARGIN_PCT);
    let border = pct(BORDER_PCT);

    let x1 = margin;
    let y1 = margin;
    let x2 = ICON_SIZE as i32 - margin;
    let y2 = ICON_SIZE as i32 - margin;

    // Draw border by filling outer rect, then filling inner rect
    draw_filled_rect(&mut pixels, ICON_SIZE, x1, y1, x2, y2, border_color);
    draw_filled_rect(&mut pixels, ICON_SIZE, x1 + border, y1 + border, x2 - border, y2 - border, fill_color);

    // Muted indicator: red X drawn inside the icon
    if is_muted {
        let mute_color: [u8; 4] = [220, 50, 50, 255]; // red
        let inset = pct(MUTE_INSET_PCT);
        let thickness = pct(MUTE_X_THICKNESS_PCT);
        let x1 = inset;
        let y1 = inset;
        let x2 = ICON_SIZE as i32 - inset;
        let y2 = ICON_SIZE as i32 - inset;
        draw_thick_line(&mut pixels, ICON_SIZE, x1, y1, x2, y2, thickness, mute_color);
        draw_thick_line(&mut pixels, ICON_SIZE, x1, y2, x2, y1, thickness, mute_color);
    }

    Image::new_owned(pixels, ICON_SIZE, ICON_SIZE)
}

/// Draws a filled rectangle onto an RGBA pixel buffer.
fn draw_filled_rect(
    pixels: &mut [u8],
    width: u32,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color: [u8; 4],
) {
    let x_start = x1.max(0) as u32;
    let x_end = (x2 as u32).min(width);
    let y_start = y1.max(0) as u32;
    let y_end = (y2 as u32).min(width);

    for y in y_start..y_end {
        for x in x_start..x_end {
            let idx = ((y * width + x) * 4) as usize;
            pixels[idx] = color[0];
            pixels[idx + 1] = color[1];
            pixels[idx + 2] = color[2];
            pixels[idx + 3] = color[3];
        }
    }
}

/// Draws a thick line between two points using distance-from-line detection.
fn draw_thick_line(
    pixels: &mut [u8],
    width: u32,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    thickness: i32,
    color: [u8; 4],
) {
    let min_x = x1.min(x2).max(0);
    let max_x = x1.max(x2).min(width as i32);
    let min_y = y1.min(y2).max(0);
    let max_y = y1.max(y2).min(width as i32);

    let dx = (x2 - x1) as f32;
    let dy = (y2 - y1) as f32;
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return;
    }

    let half = thickness as f32 / 2.0;

    for py in min_y..max_y {
        for px in min_x..max_x {
            // Distance from point to line segment
            let t = ((px - x1) as f32 * dx + (py - y1) as f32 * dy) / (len * len);
            let t = t.clamp(0.0, 1.0);
            let proj_x = x1 as f32 + t * dx;
            let proj_y = y1 as f32 + t * dy;
            let dist_x = px as f32 - proj_x;
            let dist_y = py as f32 - proj_y;
            let dist = (dist_x * dist_x + dist_y * dist_y).sqrt();

            if dist <= half {
                let idx = ((py as u32 * width + px as u32) * 4) as usize;
                pixels[idx] = color[0];
                pixels[idx + 1] = color[1];
                pixels[idx + 2] = color[2];
                pixels[idx + 3] = color[3];
            }
        }
    }
}

/// Updates the cached dark mode state and refreshes the tray icon.
pub fn set_dark_mode_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, is_dark: bool) {
    if let Some(state) = app.try_state::<crate::AppState>() {
        if let Ok(mut v) = state.is_dark_mode.lock() {
            *v = is_dark;
        }
    }
    update_tray_icon(app);
}

/// Updates the cached muted state and refreshes the tray icon.
pub fn set_muted_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, is_muted: bool) {
    if let Some(state) = app.try_state::<crate::AppState>() {
        if let Ok(mut v) = state.is_muted.lock() {
            *v = is_muted;
        }
    }
    update_tray_icon(app);
}

/// Updates the tray icon to reflect current app state.
/// Reads is_dark_mode, is_muted, and keep_awake from AppState,
/// generates a new icon with indicators, and applies it.
pub fn update_tray_icon<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
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

    /// Verifies pct conversion at 128px icon size.
    #[test]
    fn test_pct_conversion() {
        assert_eq!(pct(0.0), 0);
        assert_eq!(pct(50.0), 64);
        assert_eq!(pct(100.0), 128);
    }

    /// Verifies icon generation produces a 128x128 RGBA image.
    #[test]
    fn test_generate_icon_size() {
        let icon = generate_tray_icon(false, false, false);
        assert_eq!(icon.rgba().len(), 128 * 128 * 4);
    }

    /// Verifies dark and light mode produce different icons.
    #[test]
    fn test_dark_vs_light_differ() {
        let dark = generate_tray_icon(true, false, false);
        let light = generate_tray_icon(false, false, false);
        assert_ne!(dark.rgba(), light.rgba());
    }

    /// Verifies keep-awake indicator modifies the icon.
    #[test]
    fn test_keep_awake_modifies_icon() {
        let off = generate_tray_icon(false, false, false);
        let on = generate_tray_icon(false, true, false);
        assert_ne!(off.rgba(), on.rgba());
    }

    /// Verifies draw_filled_rect writes correct pixels.
    #[test]
    fn test_draw_rect() {
        let mut pixels = vec![0u8; 64 * 64 * 4];
        draw_filled_rect(&mut pixels, 64, 10, 10, 20, 20, [255, 0, 0, 255]);
        let idx = (15 * 64 + 15) * 4;
        assert_eq!(pixels[idx], 255);
        assert_eq!(pixels[idx + 3], 255);
        // Outside is untouched
        assert_eq!(pixels[0], 0);
    }
}
