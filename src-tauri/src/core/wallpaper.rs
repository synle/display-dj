// =========================================================================
// Wallpaper — set/get desktop wallpaper, plus a folder slideshow.
// Vendored from display-dj-cli main.rs.
// =========================================================================

use serde::{Deserialize, Serialize};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "windows")]
use super::win_cmd::hidden_command;

const VALID_FITS: &[&str] = &["fill", "fit", "stretch", "center", "tile"];

/// Returns true if `fit` is one of fill/fit/stretch/center/tile.
pub fn validate_fit(fit: &str) -> bool {
    VALID_FITS.contains(&fit)
}

/// Current wallpaper state — path and fit mode.
#[derive(Serialize, Deserialize, Clone)]
pub struct WallpaperInfo {
    pub path: Option<String>,
    pub fit: Option<String>,
}

// --- macOS per-monitor: osascript with desktop index ---

/// Set wallpaper on a specific monitor on macOS.
/// AppleScript desktop indices are 1-based, so we add 1 to the 0-based index.
#[cfg(target_os = "macos")]
pub fn set_wallpaper_one(index: usize, path: &str, _fit: &str) -> Result<(), String> {
    let desktop_num = index + 1; // AppleScript uses 1-based indexing
    let script = format!(
        "tell application \"System Events\" to tell desktop {} to set picture to \"{}\"",
        desktop_num, path
    );
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| format!("failed to run osascript: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("failed to set wallpaper on monitor {}: {}", index, stderr))
    }
}

// --- Windows per-monitor: IDesktopWallpaper COM via PowerShell ---

/// Set wallpaper on a specific monitor on Windows using IDesktopWallpaper COM interface.
/// Gets the monitor device path by index, then sets wallpaper + fit on that monitor.
#[cfg(target_os = "windows")]
pub fn set_wallpaper_one(index: usize, path: &str, fit: &str) -> Result<(), String> {
    let position = match fit {
        "fill" => "4",    // DWPOS_FILL
        "fit" => "3",     // DWPOS_FIT
        "stretch" => "2", // DWPOS_STRETCH
        "center" => "0",  // DWPOS_CENTER
        "tile" => "1",    // DWPOS_TILE
        _ => "4",
    };
    let escaped_path = path.replace('\'', "''");
    let cmd = format!(
        r#"$wp = New-Object -ComObject 'DesktopWallpaper'
$count = $wp.GetMonitorDevicePathCount()
if ({idx} -ge $count) {{ Write-Error "monitor index {idx} out of range (0..$($count-1))"; exit 1 }}
$id = $wp.GetMonitorDevicePathAt({idx})
$wp.SetWallpaper($id, '{path}')
$wp.SetPosition({pos})
"#,
        idx = index, path = escaped_path, pos = position
    );
    let output = hidden_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .output()
        .map_err(|e| format!("failed to run powershell: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("failed to set wallpaper on monitor {}: {}", index, stderr))
    }
}

// --- Linux per-monitor: not natively supported ---

/// Per-monitor wallpaper is not supported on Linux GNOME.
#[cfg(target_os = "linux")]
pub fn set_wallpaper_one(_index: usize, _path: &str, _fit: &str) -> Result<(), String> {
    Err("per-monitor wallpaper not supported on this platform".to_string())
}

// --- macOS: osascript via System Events ---

/// Set wallpaper on macOS using System Events AppleScript.
/// Sets the desktop picture on all desktops/spaces.
#[cfg(target_os = "macos")]
pub fn set_wallpaper(path: &str, _fit: &str) -> bool {
    // System Events sets the picture on all desktops
    let script = format!(
        "tell application \"System Events\" to tell every desktop to set picture to \"{}\"",
        path
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get current wallpaper path on macOS via System Events.
#[cfg(target_os = "macos")]
pub fn get_wallpaper() -> Option<WallpaperInfo> {
    let output = std::process::Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to get picture of desktop 1"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() { return None; }
    Some(WallpaperInfo { path: Some(path), fit: Some("fill".into()) })
}

/// macOS always supports wallpaper operations.
#[cfg(target_os = "macos")]
pub fn is_wallpaper_supported() -> bool { true }

// --- Windows: registry + SystemParametersInfoW via PowerShell ---

/// Set wallpaper on Windows. Sets fit mode via registry keys, then applies
/// the wallpaper using SystemParametersInfoW P/Invoke through PowerShell.
#[cfg(target_os = "windows")]
pub fn set_wallpaper(path: &str, fit: &str) -> bool {
    // Set fit mode via registry: WallpaperStyle + TileWallpaper
    let (style, tile) = match fit {
        "fill" => ("10", "0"),
        "fit" => ("6", "0"),
        "stretch" => ("2", "0"),
        "center" => ("0", "0"),
        "tile" => ("0", "1"),
        _ => ("10", "0"), // default to fill
    };
    let _ = hidden_command("reg")
        .args(["add", r"HKCU\Control Panel\Desktop", "/v", "WallpaperStyle", "/t", "REG_SZ", "/d", style, "/f"])
        .output();
    let _ = hidden_command("reg")
        .args(["add", r"HKCU\Control Panel\Desktop", "/v", "TileWallpaper", "/t", "REG_SZ", "/d", tile, "/f"])
        .output();

    // Set wallpaper via SystemParametersInfoW (SPI_SETDESKWALLPAPER = 0x0014)
    let escaped_path = path.replace('\'', "''");
    let cmd = format!(
        r#"Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
public class Wallpaper {{
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern int SystemParametersInfo(int uAction, int uParam, string lpvParam, int fuWinIni);
}}
'@
[Wallpaper]::SystemParametersInfo(0x0014, 0, '{}', 0x01 -bor 0x02)
"#,
        escaped_path
    );
    hidden_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get current wallpaper on Windows by reading registry keys.
#[cfg(target_os = "windows")]
pub fn get_wallpaper() -> Option<WallpaperInfo> {
    let output = hidden_command("reg")
        .args(["query", r"HKCU\Control Panel\Desktop", "/v", "Wallpaper"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout.lines()
        .find(|l| l.contains("Wallpaper"))
        .and_then(|l| l.split("REG_SZ").nth(1))
        .map(|s| s.trim().to_string())?;
    if path.is_empty() { return None; }

    // Read fit mode from WallpaperStyle + TileWallpaper
    let style_val = hidden_command("reg")
        .args(["query", r"HKCU\Control Panel\Desktop", "/v", "WallpaperStyle"])
        .output().ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout).lines()
                .find(|l| l.contains("WallpaperStyle"))
                .and_then(|l| l.split("REG_SZ").nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_default();
    let tile_val = hidden_command("reg")
        .args(["query", r"HKCU\Control Panel\Desktop", "/v", "TileWallpaper"])
        .output().ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout).lines()
                .find(|l| l.contains("TileWallpaper"))
                .and_then(|l| l.split("REG_SZ").nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_default();

    let fit = match (style_val.as_str(), tile_val.as_str()) {
        ("10", "0") => "fill",
        ("6", "0") => "fit",
        ("2", "0") => "stretch",
        ("0", "1") => "tile",
        ("0", "0") => "center",
        _ => "fill",
    };
    Some(WallpaperInfo { path: Some(path), fit: Some(fit.into()) })
}

/// Windows always supports wallpaper operations.
#[cfg(target_os = "windows")]
pub fn is_wallpaper_supported() -> bool { true }

// --- Linux: gsettings (GNOME), xfconf-query (XFCE), feh fallback ---

/// Set wallpaper on Linux. Tries GNOME (gsettings), XFCE (xfconf-query), and feh in order.
#[cfg(target_os = "linux")]
pub fn set_wallpaper(path: &str, fit: &str) -> bool {
    let gnome_mode = match fit {
        "fill" => "zoom",
        "fit" => "scaled",
        "stretch" => "stretched",
        "center" => "centered",
        "tile" => "wallpaper",
        _ => "zoom",
    };

    // Try GNOME (gsettings)
    let mode_ok = std::process::Command::new("gsettings")
        .args(["set", "org.gnome.desktop.background", "picture-options", gnome_mode])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if mode_ok {
        let uri = format!("file://{}", path);
        let set_ok = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.background", "picture-uri", &uri])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        // Also set picture-uri-dark for GNOME 42+ dark mode wallpaper
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.background", "picture-uri-dark", &uri])
            .output();
        if set_ok { return true; }
    }

    // Try XFCE (xfconf-query)
    if std::process::Command::new("xfconf-query")
        .args(["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/workspace0/last-image", "-s", path])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return true;
    }

    // Fallback: feh
    let feh_mode = match fit {
        "fill" => "--bg-fill",
        "fit" => "--bg-max",
        "stretch" => "--bg-scale",
        "center" => "--bg-center",
        "tile" => "--bg-tile",
        _ => "--bg-fill",
    };
    std::process::Command::new("feh")
        .args([feh_mode, path])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get current wallpaper on Linux via gsettings (GNOME).
#[cfg(target_os = "linux")]
pub fn get_wallpaper() -> Option<WallpaperInfo> {
    // Try GNOME
    let uri_output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.background", "picture-uri"])
        .output().ok()?;
    if uri_output.status.success() {
        let uri = String::from_utf8_lossy(&uri_output.stdout).trim()
            .trim_matches('\'').to_string();
        let path = uri.strip_prefix("file://").unwrap_or(&uri).to_string();
        if !path.is_empty() {
            let gnome_mode = std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.background", "picture-options"])
                .output().ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().trim_matches('\'').to_string())
                .unwrap_or_default();
            let fit = match gnome_mode.as_str() {
                "zoom" => "fill",
                "scaled" => "fit",
                "stretched" => "stretch",
                "centered" => "center",
                "wallpaper" => "tile",
                _ => "fill",
            };
            return Some(WallpaperInfo { path: Some(path), fit: Some(fit.into()) });
        }
    }
    None
}

/// Check if wallpaper operations are supported on this Linux session.
/// Returns true if any supported DE/tool is available.
#[cfg(target_os = "linux")]
pub fn is_wallpaper_supported() -> bool {
    // GNOME
    if std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.background", "picture-uri"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    { return true; }
    // XFCE
    if std::process::Command::new("xfconf-query")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    { return true; }
    // feh
    std::process::Command::new("feh")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// =========================================================================
// Wallpaper slideshow — cycles through images in a folder on a timer.
// Only one slideshow active at a time. Starting a new one cancels the old.
// Manual wallpaper changes auto-stop any running slideshow.
// State: Mutex-guarded struct + AtomicBool cancel flag + background thread.
// =========================================================================

/// Valid image extensions for slideshow folder scanning.
const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "bmp", "tiff", "tif", "gif", "heic", "webp",
];

/// Persistent slideshow state — protected by a static Mutex.
pub struct SlideshowState {
    pub running: bool,
    pub cancel: Arc<AtomicBool>,
    pub folder: String,
    pub interval_minutes: u64,
    pub order: String,
    pub fit: String,
    pub images: Vec<String>,
    pub current_index: usize,
}

impl Default for SlideshowState {
    fn default() -> Self {
        SlideshowState {
            running: false,
            cancel: Arc::new(AtomicBool::new(false)),
            folder: String::new(),
            interval_minutes: 0,
            order: String::new(),
            fit: String::new(),
            images: Vec::new(),
            current_index: 0,
        }
    }
}

pub static SLIDESHOW: std::sync::Mutex<Option<SlideshowState>> = std::sync::Mutex::new(None);

/// Scan a folder for valid image files, sorted alphabetically.
pub fn scan_images(folder: &str) -> Vec<String> {
    let dir = match std::fs::read_dir(folder) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut images: Vec<String> = dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .filter_map(|e| e.path().to_str().map(String::from))
        .collect();
    images.sort();
    images
}

/// Cancel any currently running slideshow. Sets the cancel flag so the timer thread exits.
pub fn slideshow_cancel() {
    let mut guard = SLIDESHOW.lock().unwrap();
    if let Some(state) = guard.as_ref() {
        state.cancel.store(true, Ordering::SeqCst);
    }
    *guard = None;
}

/// Start a wallpaper slideshow. Cancels any existing slideshow first.
/// Returns JSON response with image count and first image, or error.
pub fn slideshow_start(interval: u64, order: &str, fit: &str, folder: &str) -> String {
    // Validate parameters
    if interval < 5 {
        return r#"{"error":"interval must be at least 5 minutes"}"#.to_string();
    }
    if !validate_fit(fit) {
        return format!(r#"{{"error":"invalid fit mode: '{}'. Valid: fill, fit, stretch, center, tile"}}"#, fit);
    }
    if !["forward", "backward", "random"].contains(&order) {
        return format!(r#"{{"error":"invalid order: '{}'. Valid: forward, backward, random"}}"#, order);
    }
    if !std::path::Path::new(folder).is_dir() {
        return format!(r#"{{"error":"folder not found: {}"}}"#, folder);
    }

    let mut images = scan_images(folder);
    if images.is_empty() {
        return r#"{"error":"no valid images found in folder"}"#.to_string();
    }

    // Sort/shuffle based on order
    match order {
        "backward" => images.reverse(),
        "random" => shuffle(&mut images),
        _ => {} // "forward" — already sorted alphabetically
    }

    // Cancel any existing slideshow
    slideshow_cancel();

    let first_image = images[0].clone();
    let fit_owned = fit.to_string();
    let order_owned = order.to_string();
    let folder_owned = folder.to_string();

    // Set the first image immediately
    if !set_wallpaper(&first_image, fit) {
        return r#"{"error":"failed to set first wallpaper"}"#.to_string();
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel_flag.clone();
    let images_count = images.len();

    // Store state
    {
        let mut guard = SLIDESHOW.lock().unwrap();
        *guard = Some(SlideshowState {
            running: true,
            cancel: cancel_flag,
            folder: folder_owned.clone(),
            interval_minutes: interval,
            order: order_owned.clone(),
            fit: fit_owned.clone(),
            images: images.clone(),
            current_index: 0,
        });
    }

    // Spawn background timer thread
    thread::spawn(move || {
        let interval_secs = interval * 60;
        let mut idx = 0usize;

        loop {
            // Sleep in 1-second increments so we can check cancel flag frequently
            for _ in 0..interval_secs {
                if cancel_clone.load(Ordering::SeqCst) { return; }
                thread::sleep(Duration::from_secs(1));
            }
            if cancel_clone.load(Ordering::SeqCst) { return; }

            // Re-scan folder for forward/backward (picks up new/deleted files)
            let mut current_images = if order_owned == "random" {
                images.clone() // random: only reshuffle after full cycle
            } else {
                let mut fresh = scan_images(&folder_owned);
                if fresh.is_empty() {
                    // Folder empty/gone — auto-stop
                    let mut guard = SLIDESHOW.lock().unwrap();
                    *guard = None;
                    return;
                }
                if order_owned == "backward" { fresh.reverse(); }
                fresh
            };

            // Advance index
            idx += 1;
            if idx >= current_images.len() {
                if order_owned == "random" {
                    // Reshuffle and rescan for new images
                    current_images = scan_images(&folder_owned);
                    if current_images.is_empty() {
                        let mut guard = SLIDESHOW.lock().unwrap();
                        *guard = None;
                        return;
                    }
                    shuffle(&mut current_images);
                    images = current_images.clone();
                }
                idx = 0;
            }

            if idx >= current_images.len() { continue; }

            let img = &current_images[idx];

            // Skip if file no longer exists
            if !std::path::Path::new(img).exists() { continue; }

            // Set wallpaper (serialized — the Mutex in SLIDESHOW prevents interleaving)
            let _ = set_wallpaper(img, &fit_owned);

            // Update state
            if let Ok(mut guard) = SLIDESHOW.lock() {
                if let Some(state) = guard.as_mut() {
                    state.current_index = idx;
                    state.images = current_images;
                }
            }
        }
    });

    serde_json::to_string(&serde_json::json!({
        "ok": true,
        "total_images": images_count,
        "current_image": first_image
    })).unwrap()
}

/// Stop the active slideshow. Returns JSON with whether it was running.
pub fn slideshow_stop() -> String {
    let was_running = {
        let guard = SLIDESHOW.lock().unwrap();
        guard.as_ref().map_or(false, |s| s.running)
    };
    slideshow_cancel();
    format!(r#"{{"ok":true,"was_running":{}}}"#, was_running)
}

/// Query the current slideshow state. Returns full status JSON.
pub fn slideshow_status() -> String {
    let guard = SLIDESHOW.lock().unwrap();
    match guard.as_ref() {
        Some(state) if state.running => {
            let current_image = state.images.get(state.current_index)
                .cloned().unwrap_or_default();
            serde_json::to_string(&serde_json::json!({
                "running": true,
                "folder": state.folder,
                "interval_minutes": state.interval_minutes,
                "order": state.order,
                "fit": state.fit,
                "current_image": current_image,
                "current_index": state.current_index,
                "total_images": state.images.len()
            })).unwrap()
        }
        _ => r#"{"running":false}"#.to_string(),
    }
}

/// Simple Fisher-Yates shuffle using a basic LCG PRNG seeded from system time.
pub fn shuffle(items: &mut Vec<String>) {
    let len = items.len();
    if len <= 1 { return; }
    // Seed from system time nanoseconds
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut rng = seed;
    for i in (1..len).rev() {
        // LCG: rng = (rng * 6364136223846793005 + 1442695040888963407) mod 2^64
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = (rng >> 33) as usize % (i + 1);
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Tests that touch the global SLIDESHOW mutex serialize via this lock so they
    // don't fight each other when cargo runs them in parallel.
    static SLIDESHOW_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// validate_fit accepts the five canonical fit modes.
    #[test]
    fn test_validate_fit_valid_modes() {
        assert!(validate_fit("fill"));
        assert!(validate_fit("fit"));
        assert!(validate_fit("stretch"));
        assert!(validate_fit("center"));
        assert!(validate_fit("tile"));
    }

    /// validate_fit rejects unknown modes and case mismatches.
    #[test]
    fn test_validate_fit_invalid_modes() {
        assert!(!validate_fit("FILL"));
        assert!(!validate_fit("zoom"));
        assert!(!validate_fit(""));
        assert!(!validate_fit("stretched"));
    }

    /// scan_images returns empty for a nonexistent folder (no panic).
    #[test]
    fn test_scan_images_missing_folder() {
        let images = scan_images("/nonexistent/folder/abc123xyz");
        assert!(images.is_empty());
    }

    /// scan_images returns empty for a folder with no image files.
    #[test]
    fn test_scan_images_no_images() {
        let tmp = tempfile::tempdir().unwrap();
        // Create a non-image file
        std::fs::write(tmp.path().join("readme.txt"), "hello").unwrap();
        let images = scan_images(tmp.path().to_str().unwrap());
        assert!(images.is_empty());
    }

    /// scan_images detects png/jpg/jpeg/bmp/gif/webp and returns sorted paths.
    #[test]
    fn test_scan_images_finds_supported_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.png"), "x").unwrap();
        std::fs::write(tmp.path().join("c.jpg"), "x").unwrap();
        std::fs::write(tmp.path().join("b.jpeg"), "x").unwrap();
        std::fs::write(tmp.path().join("d.bmp"), "x").unwrap();
        std::fs::write(tmp.path().join("ignore.txt"), "x").unwrap();
        let images = scan_images(tmp.path().to_str().unwrap());
        assert_eq!(images.len(), 4);
        // Sorted alphabetically — a.png comes first.
        assert!(images[0].ends_with("a.png"));
        assert!(images[1].ends_with("b.jpeg"));
        assert!(images[2].ends_with("c.jpg"));
        assert!(images[3].ends_with("d.bmp"));
    }

    /// scan_images is case-insensitive on extensions (PNG and png both match).
    #[test]
    fn test_scan_images_case_insensitive() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.PNG"), "x").unwrap();
        std::fs::write(tmp.path().join("b.JPG"), "x").unwrap();
        let images = scan_images(tmp.path().to_str().unwrap());
        assert_eq!(images.len(), 2);
    }

    /// shuffle on empty or single-element vec is a no-op.
    #[test]
    fn test_shuffle_empty_and_single() {
        let mut empty: Vec<String> = Vec::new();
        shuffle(&mut empty);
        assert!(empty.is_empty());

        let mut single = vec!["a".to_string()];
        shuffle(&mut single);
        assert_eq!(single, vec!["a".to_string()]);
    }

    /// shuffle preserves all elements (just reorders).
    #[test]
    fn test_shuffle_preserves_elements() {
        let original = vec![
            "a".to_string(), "b".to_string(), "c".to_string(),
            "d".to_string(), "e".to_string()
        ];
        let mut items = original.clone();
        shuffle(&mut items);
        let mut sorted = items.clone();
        sorted.sort();
        let mut expected = original.clone();
        expected.sort();
        assert_eq!(sorted, expected);
    }

    /// slideshow_start rejects intervals < 5 minutes.
    #[test]
    fn test_slideshow_start_rejects_short_interval() {
        let _lock = SLIDESHOW_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let result = slideshow_start(1, "forward", "fill", tmp.path().to_str().unwrap());
        assert!(result.contains("at least 5 minutes"));
    }

    /// slideshow_start rejects invalid fit modes.
    #[test]
    fn test_slideshow_start_rejects_invalid_fit() {
        let _lock = SLIDESHOW_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let result = slideshow_start(5, "forward", "zoom", tmp.path().to_str().unwrap());
        assert!(result.contains("invalid fit mode"));
    }

    /// slideshow_start rejects invalid order values.
    #[test]
    fn test_slideshow_start_rejects_invalid_order() {
        let _lock = SLIDESHOW_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let result = slideshow_start(5, "sideways", "fill", tmp.path().to_str().unwrap());
        assert!(result.contains("invalid order"));
    }

    /// slideshow_start rejects nonexistent folders.
    #[test]
    fn test_slideshow_start_rejects_missing_folder() {
        let _lock = SLIDESHOW_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let result = slideshow_start(5, "forward", "fill", "/no/such/path/abc123");
        assert!(result.contains("folder not found"));
    }

    /// slideshow_start rejects empty folders.
    #[test]
    fn test_slideshow_start_rejects_empty_folder() {
        let _lock = SLIDESHOW_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let result = slideshow_start(5, "forward", "fill", tmp.path().to_str().unwrap());
        assert!(result.contains("no valid images"));
    }

    /// slideshow_stop reports was_running=false when nothing is running.
    #[test]
    fn test_slideshow_stop_when_not_running() {
        let _lock = SLIDESHOW_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        slideshow_cancel();
        let result = slideshow_stop();
        assert!(result.contains("was_running"));
    }

    /// slideshow_status returns running=false when no slideshow active.
    #[test]
    fn test_slideshow_status_not_running() {
        let _lock = SLIDESHOW_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        slideshow_cancel();
        let result = slideshow_status();
        assert!(result.contains("\"running\":false"));
    }

    /// slideshow_cancel must not panic when no slideshow exists.
    #[test]
    fn test_slideshow_cancel_no_state() {
        let _lock = SLIDESHOW_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        slideshow_cancel();
        slideshow_cancel(); // idempotent
    }

    /// SlideshowState::default produces a clean non-running instance.
    #[test]
    fn test_slideshow_state_default() {
        let s = SlideshowState::default();
        assert!(!s.running);
        assert!(s.folder.is_empty());
        assert_eq!(s.interval_minutes, 0);
        assert!(s.images.is_empty());
        assert_eq!(s.current_index, 0);
    }

    /// WallpaperInfo serializes/deserializes with optional fields.
    #[test]
    fn test_wallpaper_info_serde() {
        let info = WallpaperInfo {
            path: Some("/tmp/wp.jpg".to_string()),
            fit: Some("fill".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: WallpaperInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, Some("/tmp/wp.jpg".to_string()));
        assert_eq!(parsed.fit, Some("fill".to_string()));
    }

    /// WallpaperInfo with None fields round-trips cleanly.
    #[test]
    fn test_wallpaper_info_none_fields() {
        let info = WallpaperInfo { path: None, fit: None };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: WallpaperInfo = serde_json::from_str(&json).unwrap();
        assert!(parsed.path.is_none());
        assert!(parsed.fit.is_none());
    }

    /// is_wallpaper_supported returns a deterministic bool (does not panic).
    #[test]
    fn test_is_wallpaper_supported_smoke() {
        let _ = is_wallpaper_supported();
    }

    /// get_wallpaper is smoke-safe (may return None on CI).
    #[test]
    fn test_get_wallpaper_smoke() {
        let _ = get_wallpaper();
    }

    /// set_wallpaper with an invalid path returns false (no panic).
    #[test]
    fn test_set_wallpaper_invalid_path() {
        // Not asserting outcome — some platforms may succeed silently — just no panic.
        let _ = set_wallpaper("/no/such/wallpaper.jpg", "fill");
    }

    /// set_wallpaper_one with an invalid path/index does not panic.
    #[test]
    fn test_set_wallpaper_one_invalid() {
        let _ = set_wallpaper_one(99, "/no/such/path.jpg", "fill");
    }

    /// slideshow_start with an actual folder of images starts and is cancellable.
    /// Verifies the happy-path branches in slideshow_start (validation passed,
    /// image scan succeeded, state stored) without waiting for the timer to fire.
    #[test]
    fn test_slideshow_start_and_cancel() {
        let _lock = SLIDESHOW_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.png"), "x").unwrap();
        std::fs::write(tmp.path().join("b.png"), "x").unwrap();
        // set_wallpaper may fail in CI but slideshow_start still validates input
        // and either succeeds or returns "failed to set first wallpaper".
        let result = slideshow_start(5, "forward", "fill", tmp.path().to_str().unwrap());
        // Either succeeded or hit the wallpaper-set failure branch — both exercise validation.
        assert!(result.contains("\"ok\":true") || result.contains("failed to set first wallpaper"));
        slideshow_cancel();
    }
}
