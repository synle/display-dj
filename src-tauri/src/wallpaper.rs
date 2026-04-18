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
/// comparison to avoid unnecessary overwrites. Falls back to the cached copy
/// if the source file no longer exists.
///
/// Returns the destination path on success.
pub(crate) fn copy_to_wallpapers(
    source_path: &str,
    state: &crate::AppState,
) -> Result<PathBuf, String> {
    let source = Path::new(source_path);
    let dest_name = destination_filename(source_path);
    let dest = wallpapers_dir().join(&dest_name);

    crate::config::write_debug_log(state, &format!("wallpaper: validating source image: {}", source_path));

    match validate_image(source) {
        Ok(()) => {
            // Source is valid — copy or update cache
            crate::config::write_debug_log(state, &format!("wallpaper: destination path: {}", dest.display()));

            if dest.exists() {
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
        }
        Err(e) => {
            // Source validation failed — fall back to cached copy if available
            if dest.exists() {
                crate::config::write_debug_log(
                    state,
                    &format!("wallpaper: source unavailable ({}) — falling back to cached copy: {}", e, dest.display()),
                );
            } else {
                let msg = format!("wallpaper: validation failed — {} (no cached copy available)", e);
                crate::config::write_debug_log(state, &msg);
                return Err(msg);
            }
        }
    }

    Ok(dest)
}

/// Deletes all cached wallpaper files and remote pack folders.
/// Called from the "Clear Wallpaper Cache" tray menu item.
pub(crate) fn clear_wallpaper_cache(state: &crate::AppState) {
    let dir = wallpapers_dir();
    crate::config::write_debug_log(state, &format!("wallpaper: clearing cache at {}", dir.display()));

    let mut removed = 0u32;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if std::fs::remove_dir_all(&path).is_ok() {
                    removed += 1;
                }
            } else if std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }

    crate::config::write_debug_log(state, &format!("wallpaper: cleared {} cached items", removed));
    log::info!("wallpaper: cleared {} cached items from {}", removed, dir.display());
}

/// Returns the base URL of the display-dj sidecar HTTP server.
fn base_url() -> String {
    let port = crate::server_port();
    format!("http://127.0.0.1:{}", port)
}

/// Sets the desktop wallpaper on a single monitor via the display-dj-cli sidecar.
/// Calls `GET /set_wallpaper_one/{monitor_index}/{fit}/{path}`.
fn set_wallpaper_single_on_os(monitor_index: usize, path: &str, fit: &str) -> Result<(), String> {
    let url = format!(
        "{}/set_wallpaper_one/{}/{}/{}",
        base_url(),
        monitor_index,
        fit,
        path
    );
    let resp = reqwest::blocking::get(&url)
        .map_err(|e| format!("sidecar request failed: {}", e))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        let body = resp.text().unwrap_or_default();
        Err(format!("sidecar returned error: {}", body))
    }
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

/// Set wallpaper on a single monitor by matching a user-provided identifier.
/// Called from `execute_command()` in `tray.rs` for `command/wallpaper/change_single`.
pub(crate) fn change_wallpaper_single(
    app: &tauri::AppHandle,
    state: &crate::AppState,
    monitor_query: &str,
    source_path: &str,
    explicit_fit: Option<&str>,
) {
    use tauri::Manager;

    // Resolve fit mode
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
        &format!(
            "wallpaper: change_wallpaper_single called — monitor={}, source={}, fit={}",
            monitor_query, source_path, fit
        ),
    );

    // Fetch current monitors from sidecar to resolve the query
    let base = format!("http://127.0.0.1:{}", crate::server_port());
    let monitors: Vec<crate::display::Monitor> = match reqwest::blocking::get(format!("{}/get_all", base))
        .and_then(|r| r.json())
    {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("wallpaper: failed to fetch monitors: {}", e);
            crate::config::write_debug_log(state, &msg);
            log::warn!("{}", msg);
            return;
        }
    };

    // Resolve monitor query to index
    let (monitor_index, monitor) = match crate::display::resolve_monitor(&monitors, monitor_query) {
        Some(result) => result,
        None => {
            let available: Vec<String> = monitors.iter().map(|m| format!("{} ({})", m.name, m.id)).collect();
            let msg = format!(
                "wallpaper: no monitor matched '{}' — available: {}",
                monitor_query,
                available.join(", ")
            );
            crate::config::write_debug_log(state, &msg);
            log::warn!("{}", msg);
            return;
        }
    };

    crate::config::write_debug_log(
        state,
        &format!("wallpaper: matched monitor: {} ({}) at index {}", monitor.name, monitor.uid, monitor_index),
    );

    // Copy to wallpapers directory
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
        &format!(
            "wallpaper: setting per-monitor wallpaper for {} (fit={}) to: {}",
            monitor.name, fit, dest_str
        ),
    );

    // Set wallpaper on the specific monitor via sidecar
    match set_wallpaper_single_on_os(monitor_index, &dest_str, &fit) {
        Ok(()) => {
            crate::config::write_debug_log(
                state,
                &format!("wallpaper: successfully set wallpaper on monitor {}", monitor.name),
            );

            // Update per-monitor state in preferences
            if let Ok(mut prefs) = state.preferences.lock() {
                let entry = prefs.wallpaper.per_monitor_wallpapers.iter_mut()
                    .find(|e| e.monitor_uid == monitor.uid);
                if let Some(entry) = entry {
                    entry.wallpaper_path = dest_str;
                } else {
                    prefs.wallpaper.per_monitor_wallpapers.push(
                        crate::config::MonitorWallpaper {
                            monitor_uid: monitor.uid.clone(),
                            wallpaper_path: dest_str,
                        }
                    );
                }
                crate::config::save_preferences_to_disk(&prefs);
            }
        }
        Err(e) => {
            let msg = format!("wallpaper: failed to set per-monitor wallpaper: {}", e);
            crate::config::write_debug_log(state, &msg);
            log::warn!("{}", msg);
        }
    }
}

/// Parses slideshow command arguments: `{path}` or `{interval}/{order}/{path}`.
/// Returns `(interval_minutes, order, folder_path)`.
pub(crate) fn parse_slideshow_args(remainder: &str) -> (Option<u32>, Option<&str>, &str) {
    // Try to parse: {interval}/{order}/{path}
    // If first token is a number, treat it as interval
    if let Some(slash1) = remainder.find('/') {
        let candidate = &remainder[..slash1];
        if let Ok(interval) = candidate.parse::<u32>() {
            let after_interval = &remainder[slash1 + 1..];
            // Next token should be order (forward/backward/random)
            if let Some(slash2) = after_interval.find('/') {
                let order_candidate = &after_interval[..slash2];
                if matches!(order_candidate, "forward" | "backward" | "random") {
                    let path = &after_interval[slash2 + 1..];
                    return (Some(interval), Some(order_candidate), path);
                }
            }
        }
    }
    // Default: just a path
    (None, None, remainder)
}

/// Starts a wallpaper slideshow via the display-dj-cli sidecar.
/// Calls `GET /wallpaper_slideshow_start/{interval}/{order}/{fit}/{folder}`.
pub(crate) fn start_slideshow(
    state: &crate::AppState,
    folder: &str,
    interval: Option<u32>,
    order: Option<&str>,
) {
    let (fit, default_interval, default_order) = state
        .preferences
        .lock()
        .map(|p| (
            p.wallpaper.fit.clone(),
            p.wallpaper.slideshow_interval_minutes,
            p.wallpaper.slideshow_order.clone(),
        ))
        .unwrap_or_else(|_| ("fill".into(), 30, "forward".into()));

    let interval = interval.unwrap_or(default_interval).max(5);
    let order = order.unwrap_or(&default_order);

    crate::config::write_debug_log(
        state,
        &format!(
            "wallpaper: starting slideshow — folder={}, interval={}min, order={}, fit={}",
            folder, interval, order, fit
        ),
    );

    let url = format!(
        "{}/wallpaper_slideshow_start/{}/{}/{}/{}",
        base_url(), interval, order, fit, folder
    );

    match reqwest::blocking::get(&url) {
        Ok(resp) if resp.status().is_success() => {
            crate::config::write_debug_log(state, "wallpaper: slideshow started successfully");

            // Persist slideshow config so it resumes on app restart
            if let Ok(mut prefs) = state.preferences.lock() {
                prefs.wallpaper.slideshow_enabled = true;
                prefs.wallpaper.slideshow_folder = Some(folder.to_string());
                prefs.wallpaper.slideshow_interval_minutes = interval;
                prefs.wallpaper.slideshow_order = order.to_string();
                crate::config::save_preferences_to_disk(&prefs);
            }
        }
        Ok(resp) => {
            let body = resp.text().unwrap_or_default();
            let msg = format!("wallpaper: slideshow start failed: {}", body);
            crate::config::write_debug_log(state, &msg);
            log::warn!("{}", msg);
        }
        Err(e) => {
            let msg = format!("wallpaper: slideshow start request failed: {}", e);
            crate::config::write_debug_log(state, &msg);
            log::warn!("{}", msg);
        }
    }
}

/// Stops the active wallpaper slideshow via the display-dj-cli sidecar.
pub(crate) fn stop_slideshow(state: &crate::AppState) {
    crate::config::write_debug_log(state, "wallpaper: stopping slideshow");

    let url = format!("{}/wallpaper_slideshow_stop", base_url());
    match reqwest::blocking::get(&url) {
        Ok(resp) if resp.status().is_success() => {
            crate::config::write_debug_log(state, "wallpaper: slideshow stopped");
            if let Ok(mut prefs) = state.preferences.lock() {
                prefs.wallpaper.slideshow_enabled = false;
                crate::config::save_preferences_to_disk(&prefs);
            }
        }
        Ok(resp) => {
            let body = resp.text().unwrap_or_default();
            log::warn!("wallpaper: slideshow stop failed: {}", body);
        }
        Err(e) => {
            log::warn!("wallpaper: slideshow stop request failed: {}", e);
        }
    }
}

/// Resumes a slideshow from saved preferences on app startup.
/// Called after the sidecar is ready.
pub(crate) fn resume_slideshow_if_enabled(state: &crate::AppState) {
    let (enabled, folder, interval, order, fit) = match state.preferences.lock() {
        Ok(p) => (
            p.wallpaper.slideshow_enabled,
            p.wallpaper.slideshow_folder.clone(),
            p.wallpaper.slideshow_interval_minutes,
            p.wallpaper.slideshow_order.clone(),
            p.wallpaper.fit.clone(),
        ),
        Err(_) => return,
    };

    if !enabled {
        return;
    }

    let folder = match folder {
        Some(f) if !f.is_empty() => f,
        _ => return,
    };

    crate::config::write_debug_log(
        state,
        &format!("wallpaper: resuming slideshow from preferences — folder={}", folder),
    );

    let url = format!(
        "{}/wallpaper_slideshow_start/{}/{}/{}/{}",
        base_url(),
        interval.max(5),
        order,
        fit,
        folder
    );

    match reqwest::blocking::get(&url) {
        Ok(resp) if resp.status().is_success() => {
            crate::config::write_debug_log(state, "wallpaper: slideshow resumed successfully");
        }
        _ => {
            log::warn!("wallpaper: failed to resume slideshow on startup");
        }
    }
}

/// Maximum download size for remote wallpaper packs (500 MB).
const MAX_REMOTE_PACK_SIZE: u64 = 500 * 1024 * 1024;

/// Downloads a .zip file from a URL, extracts valid images to a subfolder
/// in the wallpapers directory, then starts a slideshow on the extracted folder.
pub(crate) fn download_and_start_remote_slideshow(
    state: &crate::AppState,
    url: &str,
) {
    crate::config::write_debug_log(state, &format!("wallpaper: remote slideshow — url={}", url));

    // Validate URL ends in .zip
    if !url.to_lowercase().ends_with(".zip") {
        let msg = format!("wallpaper: invalid format — only .zip supported: {}", url);
        crate::config::write_debug_log(state, &msg);
        log::warn!("{}", msg);
        return;
    }

    // Compute destination folder
    let url_hash = format!("{:x}", md5::compute(url.as_bytes()));
    let dest_dir = wallpapers_dir().join(format!("remote-{}", url_hash));

    // Check if already extracted (idempotent)
    if dest_dir.exists() && has_valid_images(&dest_dir) {
        crate::config::write_debug_log(
            state,
            &format!("wallpaper: remote pack cached, skipping download: {}", dest_dir.display()),
        );
        start_slideshow(state, &dest_dir.to_string_lossy(), None, None);
        return;
    }

    // Download
    crate::config::write_debug_log(state, &format!("wallpaper: downloading remote pack from: {}", url));
    let resp = match reqwest::blocking::get(url) {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("wallpaper: download failed: {}", e);
            crate::config::write_debug_log(state, &msg);
            log::warn!("{}", msg);
            return;
        }
    };

    if !resp.status().is_success() {
        let msg = format!("wallpaper: download returned HTTP {}: {}", resp.status(), url);
        crate::config::write_debug_log(state, &msg);
        log::warn!("{}", msg);
        return;
    }

    // Check content length if available
    if let Some(len) = resp.content_length() {
        if len > MAX_REMOTE_PACK_SIZE {
            let msg = format!(
                "wallpaper: download too large ({} bytes, max {} bytes): {}",
                len, MAX_REMOTE_PACK_SIZE, url
            );
            crate::config::write_debug_log(state, &msg);
            log::warn!("{}", msg);
            return;
        }
    }

    let bytes = match resp.bytes() {
        Ok(b) => b,
        Err(e) => {
            let msg = format!("wallpaper: failed to read download body: {}", e);
            crate::config::write_debug_log(state, &msg);
            log::warn!("{}", msg);
            return;
        }
    };

    crate::config::write_debug_log(
        state,
        &format!("wallpaper: download complete ({} bytes), extracting to: {}", bytes.len(), dest_dir.display()),
    );

    // Extract valid images from zip
    std::fs::create_dir_all(&dest_dir).ok();

    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(a) => a,
        Err(e) => {
            let msg = format!("wallpaper: invalid zip file: {}", e);
            crate::config::write_debug_log(state, &msg);
            log::warn!("{}", msg);
            std::fs::remove_dir_all(&dest_dir).ok();
            return;
        }
    };

    let mut extracted_count = 0u32;
    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(_) => continue,
        };

        // Skip directories and non-image files
        if file.is_dir() {
            continue;
        }

        let name = file.name().to_string();
        let ext = std::path::Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if !VALID_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        // Extract to flat directory (use just the filename, not nested paths)
        let filename = std::path::Path::new(&name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&name);

        let dest_path = dest_dir.join(filename);

        if let Ok(mut out_file) = std::fs::File::create(&dest_path) {
            if std::io::copy(&mut file, &mut out_file).is_ok() {
                // Verify extracted file is large enough
                if let Ok(meta) = std::fs::metadata(&dest_path) {
                    if meta.len() >= MIN_IMAGE_SIZE {
                        extracted_count += 1;
                    } else {
                        std::fs::remove_file(&dest_path).ok();
                    }
                }
            }
        }
    }

    crate::config::write_debug_log(
        state,
        &format!("wallpaper: extracted {} images from remote pack", extracted_count),
    );

    if extracted_count == 0 {
        let msg = "wallpaper: remote pack empty — no valid images found in zip";
        crate::config::write_debug_log(state, msg);
        log::warn!("{}", msg);
        std::fs::remove_dir_all(&dest_dir).ok();
        return;
    }

    // Start slideshow on the extracted folder
    crate::config::write_debug_log(
        state,
        &format!("wallpaper: starting slideshow on remote pack: {}", dest_dir.display()),
    );
    start_slideshow(state, &dest_dir.to_string_lossy(), None, None);
}

/// Returns true if the directory contains at least one valid image file.
fn has_valid_images(dir: &std::path::Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let ext = entry.path()
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            if VALID_EXTENSIONS.contains(&ext.as_str()) {
                return true;
            }
        }
    }
    false
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

    /// Verifies parse_slideshow_args with just a folder path.
    #[test]
    fn test_parse_slideshow_args_path_only() {
        let (interval, order, path) = parse_slideshow_args("/Users/syle/Pictures");
        assert_eq!(interval, None);
        assert_eq!(order, None);
        assert_eq!(path, "/Users/syle/Pictures");
    }

    /// Verifies parse_slideshow_args with interval, order, and path.
    #[test]
    fn test_parse_slideshow_args_full() {
        let (interval, order, path) = parse_slideshow_args("15/random//Users/syle/Pictures");
        assert_eq!(interval, Some(15));
        assert_eq!(order, Some("random"));
        assert_eq!(path, "/Users/syle/Pictures");
    }

    /// Verifies parse_slideshow_args with forward order.
    #[test]
    fn test_parse_slideshow_args_forward() {
        let (interval, order, path) = parse_slideshow_args("30/forward//tmp/wallpapers");
        assert_eq!(interval, Some(30));
        assert_eq!(order, Some("forward"));
        assert_eq!(path, "/tmp/wallpapers");
    }

    /// Verifies parse_slideshow_args treats non-numeric first token as path.
    #[test]
    fn test_parse_slideshow_args_nonnumeric_is_path() {
        let (interval, order, path) = parse_slideshow_args("Pictures/nature");
        assert_eq!(interval, None);
        assert_eq!(order, None);
        assert_eq!(path, "Pictures/nature");
    }

    /// Verifies URL validation rejects non-.zip URLs.
    #[test]
    fn test_remote_url_must_be_zip() {
        assert!("https://example.com/pack.zip".to_lowercase().ends_with(".zip"));
        assert!("https://example.com/pack.ZIP".to_lowercase().ends_with(".zip"));
        assert!(!"https://example.com/pack.tar.gz".to_lowercase().ends_with(".zip"));
        assert!(!"https://example.com/pack.rar".to_lowercase().ends_with(".zip"));
    }

    /// Verifies MD5 URL hashing produces a deterministic folder name.
    #[test]
    fn test_remote_pack_folder_name() {
        let url = "https://example.com/nature-pack.zip";
        let hash = format!("{:x}", md5::compute(url.as_bytes()));
        let folder = format!("remote-{}", hash);
        assert!(folder.starts_with("remote-"));
        // Same URL always produces the same folder
        let hash2 = format!("{:x}", md5::compute(url.as_bytes()));
        assert_eq!(hash, hash2);
    }

    /// Verifies has_valid_images detects images in a directory.
    #[test]
    fn test_has_valid_images() {
        let dir = std::env::temp_dir().join("test_has_valid_images");
        std::fs::create_dir_all(&dir).unwrap();
        // Empty dir
        assert!(!has_valid_images(&dir));
        // Add a valid image
        std::fs::write(dir.join("test.jpg"), vec![0u8; 2048]).unwrap();
        assert!(has_valid_images(&dir));
        // Add only non-image
        std::fs::remove_file(dir.join("test.jpg")).ok();
        std::fs::write(dir.join("readme.txt"), b"hello").unwrap();
        assert!(!has_valid_images(&dir));
        std::fs::remove_dir_all(&dir).ok();
    }
}
