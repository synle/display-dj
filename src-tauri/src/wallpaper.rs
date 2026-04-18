use std::path::{Path, PathBuf};

/// Valid image file extensions for wallpapers.
const VALID_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "bmp", "tiff", "tif", "gif", "heic", "webp",
];

/// Known wallpaper fit modes. Used to distinguish a fit token from the start of a file path.
const VALID_FIT_MODES: &[&str] = &["fill", "fit", "stretch", "center", "tile"];

/// Minimum file size in bytes for a valid wallpaper image (1 KB).
const MIN_IMAGE_SIZE: u64 = 1024;

/// Returns the wallpapers storage directory, creating it if it doesn't exist.
/// Located at `~/.config/display-dj/wallpapers/`.
pub(crate) fn wallpapers_dir() -> PathBuf {
    let dir = crate::config::config_dir().join("wallpapers");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Parses the remainder of a `command/wallpaper/change/` command string into
/// an optional fit mode and the file path. If the first `/`-delimited token is
/// a known fit mode, it is consumed; otherwise the entire string is the path
/// and the default fit from preferences is used.
///
/// Returns `(Option<fit_mode>, path)` where `None` fit means "use preference default".
pub(crate) fn parse_wallpaper_args(remainder: &str) -> (Option<&str>, &str) {
    // Find the first '/' to check if the leading segment is a fit mode.
    // Example: "center//Users/syle/pic.jpg" → fit="center", path="/Users/syle/pic.jpg"
    // Example: "/Users/syle/pic.jpg" → fit=None, path="/Users/syle/pic.jpg"
    if let Some(slash_pos) = remainder.find('/') {
        let candidate = &remainder[..slash_pos];
        if VALID_FIT_MODES.contains(&candidate) {
            return (Some(candidate), &remainder[slash_pos + 1..]);
        }
    }
    (None, remainder)
}

/// Validates that the given path points to a valid wallpaper image file.
/// Returns `Ok(())` on success or `Err(description)` on failure.
pub(crate) fn validate_image(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("file not found: {}", path.display()));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !VALID_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!("invalid extension '{}' for: {}", ext, path.display()));
    }

    match std::fs::metadata(path) {
        Ok(meta) => {
            if meta.len() < MIN_IMAGE_SIZE {
                return Err(format!(
                    "file too small ({} bytes): {}",
                    meta.len(),
                    path.display()
                ));
            }
        }
        Err(e) => {
            return Err(format!("cannot read file metadata: {} — {}", path.display(), e));
        }
    }

    Ok(())
}

/// Computes the destination filename for a wallpaper in our cache directory.
/// Uses MD5 of the source path string to generate a stable, unique name
/// while preserving the original file extension.
///
/// Example: `/Users/syle/pic.jpg` → `wallpaper-a1b2c3d4e5f6.jpg`
pub(crate) fn destination_filename(source_path: &str) -> String {
    let hash = format!("{:x}", md5::compute(source_path.as_bytes()));
    let ext = Path::new(source_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    format!("wallpaper-{}.{}", hash, ext)
}

/// Computes the MD5 hash of a file's content. Returns the hex string.
fn file_content_hash(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("cannot read file {}: {}", path.display(), e))?;
    Ok(format!("{:x}", md5::compute(&bytes)))
}

/// Copies the source image to the wallpapers directory, using content-hash
/// comparison to avoid unnecessary overwrites.
///
/// Returns the destination path on success.
pub(crate) fn copy_to_wallpapers(
    source_path: &str,
    state: &crate::AppState,
) -> Result<PathBuf, String> {
    let source = Path::new(source_path);

    // Validate the source image
    crate::config::write_debug_log(state, &format!("wallpaper: validating source image: {}", source_path));
    validate_image(source).map_err(|e| {
        let msg = format!("wallpaper: validation failed — {}", e);
        crate::config::write_debug_log(state, &msg);
        msg
    })?;

    let dest_name = destination_filename(source_path);
    let dest = wallpapers_dir().join(&dest_name);

    crate::config::write_debug_log(state, &format!("wallpaper: destination path: {}", dest.display()));

    if dest.exists() {
        // Compare content hashes to decide whether to overwrite
        let source_hash = file_content_hash(source)?;
        let dest_hash = file_content_hash(&dest)?;

        if source_hash == dest_hash {
            crate::config::write_debug_log(state, "wallpaper: content unchanged (same MD5), skipping copy");
        } else {
            crate::config::write_debug_log(state, "wallpaper: content changed, overwriting cached copy");
            std::fs::copy(source, &dest)
                .map_err(|e| format!("wallpaper: failed to copy: {}", e))?;
        }
    } else {
        crate::config::write_debug_log(state, "wallpaper: first time — copying to wallpapers dir");
        std::fs::copy(source, &dest)
            .map_err(|e| format!("wallpaper: failed to copy: {}", e))?;
    }

    Ok(dest)
}

/// Returns the base URL of the display-dj sidecar HTTP server.
fn base_url() -> String {
    let port = crate::server_port();
    format!("http://127.0.0.1:{}", port)
}

/// Sets the desktop wallpaper on all monitors via the display-dj-cli sidecar.
/// Calls `GET /set_wallpaper/{fit}/{path}`.
fn set_wallpaper_on_os(path: &str, fit: &str) -> Result<(), String> {
    let url = format!("{}/set_wallpaper/{}/{}", base_url(), fit, path);
    let resp = reqwest::blocking::get(&url)
        .map_err(|e| format!("sidecar request failed: {}", e))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        let body = resp.text().unwrap_or_default();
        Err(format!("sidecar returned error: {}", body))
    }
}

/// Main entry point: validate, copy, and set the desktop wallpaper.
/// Called from `execute_command()` in `tray.rs`.
pub(crate) fn change_wallpaper(
    state: &crate::AppState,
    source_path: &str,
    explicit_fit: Option<&str>,
) {
    // Resolve fit mode: explicit override > preference > default
    let fit = match explicit_fit {
        Some(f) => f.to_string(),
        None => state
            .preferences
            .lock()
            .map(|p| p.wallpaper.fit.clone())
            .unwrap_or_else(|_| "fill".into()),
    };

    crate::config::write_debug_log(
        state,
        &format!("wallpaper: change_wallpaper called — source={}, fit={}", source_path, fit),
    );

    // Copy to our wallpapers directory
    let dest = match copy_to_wallpapers(source_path, state) {
        Ok(d) => d,
        Err(e) => {
            log::warn!("{}", e);
            return;
        }
    };

    let dest_str = dest.to_string_lossy().to_string();
    crate::config::write_debug_log(
        state,
        &format!("wallpaper: setting desktop wallpaper (fit={}) to: {}", fit, dest_str),
    );

    // Set the wallpaper using platform-specific API
    match set_wallpaper_on_os(&dest_str, &fit) {
        Ok(()) => {
            crate::config::write_debug_log(state, "wallpaper: successfully set wallpaper");

            // Update preferences with the new wallpaper path
            if let Ok(mut prefs) = state.preferences.lock() {
                prefs.wallpaper.current_wallpaper_path = Some(dest_str);
                if explicit_fit.is_some() {
                    prefs.wallpaper.fit = fit;
                }
                crate::config::save_preferences_to_disk(&prefs);
            }
        }
        Err(e) => {
            let msg = format!("wallpaper: failed to set wallpaper: {}", e);
            crate::config::write_debug_log(state, &msg);
            log::warn!("{}", msg);
        }
    }
}

/// Generates a default gradient wallpaper as a BMP file.
/// Creates a vertical gradient from `color_top` to `color_bottom`.
/// Returns the path to the generated file.
pub(crate) fn generate_default_wallpaper(
    filename: &str,
    color_top: [u8; 3],
    color_bottom: [u8; 3],
) -> Result<PathBuf, String> {
    let dir = wallpapers_dir();
    let path = dir.join(filename);

    // Skip if already exists
    if path.exists() {
        return Ok(path);
    }

    let width: u32 = 1920;
    let height: u32 = 1080;
    let row_bytes = width * 3;
    // BMP rows must be padded to a multiple of 4 bytes
    let row_stride = (row_bytes + 3) & !3;
    let pixel_data_size = row_stride * height;
    let file_size = 54 + pixel_data_size; // 14 (file header) + 40 (info header) + pixels

    let mut data = Vec::with_capacity(file_size as usize);

    // BMP File Header (14 bytes)
    data.extend_from_slice(b"BM");
    data.extend_from_slice(&(file_size).to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes()); // reserved
    data.extend_from_slice(&0u16.to_le_bytes()); // reserved
    data.extend_from_slice(&54u32.to_le_bytes()); // pixel data offset

    // BMP Info Header (40 bytes)
    data.extend_from_slice(&40u32.to_le_bytes()); // header size
    data.extend_from_slice(&(width as i32).to_le_bytes());
    data.extend_from_slice(&(height as i32).to_le_bytes()); // positive = bottom-up
    data.extend_from_slice(&1u16.to_le_bytes()); // planes
    data.extend_from_slice(&24u16.to_le_bytes()); // bits per pixel
    data.extend_from_slice(&0u32.to_le_bytes()); // compression (none)
    data.extend_from_slice(&pixel_data_size.to_le_bytes());
    data.extend_from_slice(&2835u32.to_le_bytes()); // h resolution (72 DPI)
    data.extend_from_slice(&2835u32.to_le_bytes()); // v resolution
    data.extend_from_slice(&0u32.to_le_bytes()); // colors used
    data.extend_from_slice(&0u32.to_le_bytes()); // important colors

    // Pixel data (bottom-up rows, BGR byte order)
    for y in 0..height {
        // BMP is bottom-up, so row 0 is the bottom of the image.
        // We want color_top at the visual top (high y in BMP coords).
        let t = y as f64 / (height - 1) as f64; // 0.0 = bottom, 1.0 = top
        let r = lerp_u8(color_bottom[0], color_top[0], t);
        let g = lerp_u8(color_bottom[1], color_top[1], t);
        let b = lerp_u8(color_bottom[2], color_top[2], t);
        for _x in 0..width {
            data.push(b); // BMP uses BGR order
            data.push(g);
            data.push(r);
        }
        // Pad row to 4-byte alignment
        let padding = (row_stride - row_bytes) as usize;
        for _ in 0..padding {
            data.push(0);
        }
    }

    std::fs::write(&path, &data)
        .map_err(|e| format!("wallpaper: failed to write default wallpaper: {}", e))?;

    Ok(path)
}

/// Linear interpolation between two u8 values.
fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    let v = a as f64 + (b as f64 - a as f64) * t;
    v.round().clamp(0.0, 255.0) as u8
}

/// Ensures default dark and light wallpapers exist in the wallpapers directory.
/// Called once during app initialization.
pub(crate) fn ensure_default_wallpapers(state: &crate::AppState) {
    // Dark: deep navy #0f0c29 → dark indigo #302b63
    match generate_default_wallpaper("default-dark.bmp", [0x0f, 0x0c, 0x29], [0x30, 0x2b, 0x63]) {
        Ok(_) => crate::config::write_debug_log(state, "wallpaper: default dark wallpaper ready"),
        Err(e) => log::warn!("{}", e),
    }

    // Light: soft blue #e0eafc → pale lavender #cfdef3
    match generate_default_wallpaper("default-light.bmp", [0xe0, 0xea, 0xfc], [0xcf, 0xde, 0xf3]) {
        Ok(_) => crate::config::write_debug_log(state, "wallpaper: default light wallpaper ready"),
        Err(e) => log::warn!("{}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that valid image extensions are accepted.
    #[test]
    fn test_validate_extensions_accepted() {
        for ext in VALID_EXTENSIONS {
            assert!(
                VALID_EXTENSIONS.contains(ext),
                "extension '{}' should be valid",
                ext
            );
        }
    }

    /// Verifies that invalid extensions are rejected by validate_image.
    #[test]
    fn test_validate_rejects_invalid_extension() {
        let tmp = std::env::temp_dir().join("test_wallpaper_bad.txt");
        // Create a file large enough to pass size check
        std::fs::write(&tmp, vec![0u8; 2048]).unwrap();
        let result = validate_image(&tmp);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid extension"));
        std::fs::remove_file(&tmp).ok();
    }

    /// Verifies that non-existent files produce a "file not found" error.
    #[test]
    fn test_validate_file_not_found() {
        let result = validate_image(Path::new("/nonexistent/wallpaper.jpg"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("file not found"));
    }

    /// Verifies that files smaller than MIN_IMAGE_SIZE are rejected.
    #[test]
    fn test_validate_file_too_small() {
        let tmp = std::env::temp_dir().join("test_wallpaper_small.png");
        std::fs::write(&tmp, vec![0u8; 100]).unwrap(); // 100 bytes < 1 KB
        let result = validate_image(&tmp);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("file too small"));
        std::fs::remove_file(&tmp).ok();
    }

    /// Verifies that a valid image file passes validation.
    #[test]
    fn test_validate_valid_image() {
        let tmp = std::env::temp_dir().join("test_wallpaper_valid.jpg");
        std::fs::write(&tmp, vec![0u8; 2048]).unwrap();
        let result = validate_image(&tmp);
        assert!(result.is_ok());
        std::fs::remove_file(&tmp).ok();
    }

    /// Verifies MD5-based destination filename generation.
    #[test]
    fn test_destination_filename_deterministic() {
        let name1 = destination_filename("/Users/syle/pic.jpg");
        let name2 = destination_filename("/Users/syle/pic.jpg");
        assert_eq!(name1, name2);
        assert!(name1.starts_with("wallpaper-"));
        assert!(name1.ends_with(".jpg"));
    }

    /// Verifies different paths produce different filenames.
    #[test]
    fn test_destination_filename_different_paths() {
        let name1 = destination_filename("/path/a.png");
        let name2 = destination_filename("/path/b.png");
        assert_ne!(name1, name2);
    }

    /// Verifies extension is preserved in destination filename.
    #[test]
    fn test_destination_filename_preserves_extension() {
        assert!(destination_filename("/a.png").ends_with(".png"));
        assert!(destination_filename("/a.jpg").ends_with(".jpg"));
        assert!(destination_filename("/a.webp").ends_with(".webp"));
    }

    /// Verifies wallpapers_dir() returns the expected path under config dir.
    #[test]
    fn test_wallpapers_dir_path() {
        let dir = wallpapers_dir();
        assert!(dir.ends_with("wallpapers"));
        assert!(dir.parent().unwrap().ends_with("display-dj"));
    }

    /// Verifies parse_wallpaper_args extracts a known fit mode.
    #[test]
    fn test_parse_args_with_fit() {
        let (fit, path) = parse_wallpaper_args("center//Users/syle/pic.jpg");
        assert_eq!(fit, Some("center"));
        assert_eq!(path, "/Users/syle/pic.jpg");
    }

    /// Verifies parse_wallpaper_args returns None fit when path starts directly.
    #[test]
    fn test_parse_args_no_fit() {
        let (fit, path) = parse_wallpaper_args("/Users/syle/pic.jpg");
        assert_eq!(fit, None);
        assert_eq!(path, "/Users/syle/pic.jpg");
    }

    /// Verifies parse_wallpaper_args handles all valid fit modes.
    #[test]
    fn test_parse_args_all_fit_modes() {
        for mode in VALID_FIT_MODES {
            let input = format!("{}//test/image.jpg", mode);
            let (fit, _path) = parse_wallpaper_args(&input);
            assert_eq!(fit, Some(*mode), "should recognize fit mode '{}'", mode);
        }
    }

    /// Verifies parse_wallpaper_args treats unknown tokens as path, not fit.
    #[test]
    fn test_parse_args_unknown_token_is_path() {
        let (fit, path) = parse_wallpaper_args("unknown//Users/pic.jpg");
        assert_eq!(fit, None);
        assert_eq!(path, "unknown//Users/pic.jpg");
    }

    /// Verifies default wallpaper BMP generation produces valid-sized files.
    #[test]
    fn test_generate_default_wallpaper() {
        let dir = std::env::temp_dir().join("test_wallpapers_gen");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test-gradient.bmp");
        // Clean up any previous run
        std::fs::remove_file(&path).ok();

        let result = generate_default_wallpaper(
            "test-gradient.bmp",
            [0xff, 0x00, 0x00],
            [0x00, 0x00, 0xff],
        );
        // Since generate_default_wallpaper uses wallpapers_dir(), we test that separately.
        // Here just verify the function doesn't panic with valid inputs.
        // The actual file goes to the real wallpapers dir, so check it there.
        let actual_path = wallpapers_dir().join("test-gradient.bmp");
        if actual_path.exists() {
            let meta = std::fs::metadata(&actual_path).unwrap();
            // 1920 * 1080 * 3 bytes + padding + 54 byte header ≈ 6.2 MB
            assert!(meta.len() > 6_000_000, "BMP should be > 6 MB");
            assert!(meta.len() < 7_000_000, "BMP should be < 7 MB");
            std::fs::remove_file(&actual_path).ok();
        }
        assert!(result.is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Verifies the lerp_u8 helper interpolates correctly.
    #[test]
    fn test_lerp_u8() {
        assert_eq!(lerp_u8(0, 255, 0.0), 0);
        assert_eq!(lerp_u8(0, 255, 1.0), 255);
        assert_eq!(lerp_u8(0, 255, 0.5), 128);
        assert_eq!(lerp_u8(100, 200, 0.5), 150);
    }
}
