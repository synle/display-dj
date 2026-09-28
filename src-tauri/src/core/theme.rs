// =========================================================================
// Dark mode / light mode — per-platform implementations.
// Vendored from display-dj-cli main.rs.
// =========================================================================

#[cfg(target_os = "windows")]
use super::win_cmd::hidden_command;

// --- macOS dark mode: AppleScript via osascript ---

/// Set dark/light mode on macOS via System Events AppleScript.
/// Toggles the system-wide appearance preference.
#[cfg(target_os = "macos")]
pub fn set_dark_mode(dark: bool) -> bool {
    let val = if dark { "true" } else { "false" };
    let script = format!(
        "tell application \"System Events\" to tell appearance preferences to set dark mode to {}",
        val
    );
    // .map() transforms Ok(output) -> Ok(bool), .unwrap_or(false) handles Err case
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get current dark mode state on macOS. Returns Some(true) for dark, Some(false)
/// for light, None if detection fails.
#[cfg(target_os = "macos")]
pub fn get_dark_mode() -> Option<bool> {
    let output = std::process::Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to tell appearance preferences to get dark mode"])
        .output()
        .ok()?; // .ok() converts Result->Option, ? returns None early on failure
    if !output.status.success() { return None; }
    let val = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
    Some(val == "true")
}

// --- Windows dark mode: registry keys + WM_SETTINGCHANGE broadcast ---

/// Set dark/light mode on Windows by writing to the Personalize registry keys.
/// Sets both AppsUseLightTheme (app chrome) and SystemUsesLightTheme (taskbar/start menu).
/// Broadcasts WM_SETTINGCHANGE so already-open windows refresh their title bars.
#[cfg(target_os = "windows")]
pub fn set_dark_mode(dark: bool) -> bool {
    // Windows uses 0=dark, 1=light (inverted from what you'd expect)
    let val = if dark { "0" } else { "1" };
    // Must set both keys — AppsUseLightTheme for app chrome, SystemUsesLightTheme for taskbar
    let app = hidden_command("reg")
        .args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
               "/v", "AppsUseLightTheme", "/t", "REG_DWORD", "/d", val, "/f"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let sys = hidden_command("reg")
        .args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
               "/v", "SystemUsesLightTheme", "/t", "REG_DWORD", "/d", val, "/f"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if app && sys {
        // Broadcast WM_SETTINGCHANGE so existing windows refresh their title bars
        let _ = hidden_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", r#"
                Add-Type -TypeDefinition @'
                using System;
                using System.Runtime.InteropServices;
                public class ThemeBroadcast {
                    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
                    public static extern IntPtr SendMessageTimeout(
                        IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
                        uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
                    public static void Broadcast() {
                        UIntPtr result;
                        SendMessageTimeout((IntPtr)0xffff, 0x001A, UIntPtr.Zero,
                            "ImmersiveColorSet", 0x0002, 5000, out result);
                    }
                }
'@
                [ThemeBroadcast]::Broadcast()
            "#])
            .output();
        true
    } else {
        false
    }
}

/// Get current dark mode state on Windows by reading the registry.
/// AppsUseLightTheme: 0 = dark mode ON, 1 = light mode (note: inverted naming).
#[cfg(target_os = "windows")]
pub fn get_dark_mode() -> Option<bool> {
    let output = hidden_command("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
               "/v", "AppsUseLightTheme"])
        .output()
        .ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("0x0") {
        Some(true)  // 0 = dark mode ON
    } else if stdout.contains("0x1") {
        Some(false) // 1 = light mode
    } else {
        None
    }
}

// --- Linux dark mode: desktop-aware GNOME / KDE / XFCE routing ---

/// Set GNOME-compatible dark/light appearance through gsettings.
#[cfg(target_os = "linux")]
fn set_gnome_dark_mode(dark: bool) -> bool {
    let gtk_theme = if dark { "Adwaita-dark" } else { "Adwaita" };
    let color_scheme = if dark { "prefer-dark" } else { "prefer-light" };
    if !super::linux_desktop::run_checked(
        "gsettings",
        &[
            "set",
            "org.gnome.desktop.interface",
            "color-scheme",
            color_scheme,
        ],
        "set dark mode",
    ) {
        return false;
    }
    let _ = super::linux_desktop::run_checked(
        "gsettings",
        &["set", "org.gnome.desktop.interface", "gtk-theme", gtk_theme],
        "set GTK theme",
    );
    true
}

/// Set KDE Plasma's active color scheme.
#[cfg(target_os = "linux")]
fn set_kde_dark_mode(dark: bool) -> bool {
    super::linux_desktop::run_checked(
        "plasma-apply-colorscheme",
        &[if dark { "BreezeDark" } else { "BreezeLight" }],
        "set dark mode",
    )
}

/// Set XFCE's GTK theme through xfconf.
#[cfg(target_os = "linux")]
fn set_xfce_dark_mode(dark: bool) -> bool {
    super::linux_desktop::run_checked(
        "xfconf-query",
        &[
            "-c",
            "xsettings",
            "-p",
            "/Net/ThemeName",
            "-s",
            if dark { "Adwaita-dark" } else { "Adwaita" },
        ],
        "set dark mode",
    )
}

/// Set dark/light mode on Linux through the active desktop's native backend.
/// Unknown desktops retain best-effort probing for backward compatibility.
#[cfg(target_os = "linux")]
pub fn set_dark_mode(dark: bool) -> bool {
    match super::linux_desktop::current() {
        super::linux_desktop::LinuxDesktop::Gnome => set_gnome_dark_mode(dark),
        super::linux_desktop::LinuxDesktop::Kde => set_kde_dark_mode(dark),
        super::linux_desktop::LinuxDesktop::Xfce => set_xfce_dark_mode(dark),
        super::linux_desktop::LinuxDesktop::Other => {
            set_gnome_dark_mode(dark) || set_kde_dark_mode(dark) || set_xfce_dark_mode(dark)
        }
    }
}

/// Read GNOME-compatible dark mode from color-scheme or GTK theme.
#[cfg(target_os = "linux")]
fn get_gnome_dark_mode() -> Option<bool> {
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            if val.contains("dark") { return Some(true); }
            if val.contains("light") || val.contains("default") { return Some(false); }
        }
    }

    // GNOME fallback: check the GTK theme name for "dark" substring
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            return Some(val.contains("dark"));
        }
    }
    None
}

/// Read KDE Plasma's color scheme, preferring Plasma 6's kreadconfig6.
#[cfg(target_os = "linux")]
fn get_kde_dark_mode() -> Option<bool> {
    for program in ["kreadconfig6", "kreadconfig5"] {
        if let Ok(output) = std::process::Command::new(program)
            .args(["--group", "General", "--key", "ColorScheme"])
            .output()
        {
            if output.status.success() {
                let val = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
                return Some(val.contains("dark"));
            }
        }
    }
    None
}

/// Read XFCE's configured GTK theme.
#[cfg(target_os = "linux")]
fn get_xfce_dark_mode() -> Option<bool> {
    let output = std::process::Command::new("xfconf-query")
        .args(["-c", "xsettings", "-p", "/Net/ThemeName"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_lowercase()
            .contains("dark"),
    )
}

/// Get current Linux dark mode through the active desktop's native backend.
#[cfg(target_os = "linux")]
pub fn get_dark_mode() -> Option<bool> {
    match super::linux_desktop::current() {
        super::linux_desktop::LinuxDesktop::Gnome => get_gnome_dark_mode(),
        super::linux_desktop::LinuxDesktop::Kde => get_kde_dark_mode(),
        super::linux_desktop::LinuxDesktop::Xfce => get_xfce_dark_mode(),
        super::linux_desktop::LinuxDesktop::Other => get_gnome_dark_mode()
            .or_else(get_kde_dark_mode)
            .or_else(get_xfce_dark_mode),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: get_dark_mode must not panic; returns Some(bool) on supported
    /// systems, None if detection fails. Both outcomes are acceptable in tests.
    #[test]
    fn test_get_dark_mode_does_not_panic() {
        let _ = get_dark_mode();
    }

    /// Smoke test: set_dark_mode must not panic for either value. Whether it
    /// actually flips the system theme depends on permissions and DE; we just
    /// verify the function returns without crashing. Restores the original
    /// state at the end so we don't leave the dev's system flipped.
    #[test]
    fn test_set_dark_mode_restores_state() {
        let original = get_dark_mode();
        let _ = set_dark_mode(true);
        let _ = set_dark_mode(false);
        if let Some(was_dark) = original {
            let _ = set_dark_mode(was_dark);
        }
    }
}
