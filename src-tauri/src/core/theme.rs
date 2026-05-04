// =========================================================================
// Dark mode / light mode — per-platform implementations.
// Vendored from display-dj-cli main.rs.
// =========================================================================

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
    let app = std::process::Command::new("reg")
        .args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
               "/v", "AppsUseLightTheme", "/t", "REG_DWORD", "/d", val, "/f"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let sys = std::process::Command::new("reg")
        .args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
               "/v", "SystemUsesLightTheme", "/t", "REG_DWORD", "/d", val, "/f"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if app && sys {
        // Broadcast WM_SETTINGCHANGE so existing windows refresh their title bars
        let _ = std::process::Command::new("powershell")
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
    let output = std::process::Command::new("reg")
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

// --- Linux dark mode: tries desktop environments in order (GNOME -> KDE -> XFCE) ---

/// Set dark/light mode on Linux. Tries GNOME (gsettings color-scheme + gtk-theme),
/// KDE (plasma-apply-colorscheme), and XFCE (xfconf-query) in order.
/// Returns true on first success, false if no supported DE was found.
#[cfg(target_os = "linux")]
pub fn set_dark_mode(dark: bool) -> bool {
    let gtk_theme = if dark { "Adwaita-dark" } else { "Adwaita" };
    let color_scheme = if dark { "prefer-dark" } else { "prefer-light" };

    // GNOME 42+ uses color-scheme (the modern way)
    if std::process::Command::new("gsettings")
        .args(["set", "org.gnome.desktop.interface", "color-scheme", color_scheme])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        // Also set gtk-theme for older GTK3 apps that don't read color-scheme
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.interface", "gtk-theme", gtk_theme])
            .output();
        return true;
    }

    // KDE Plasma
    if std::process::Command::new("plasma-apply-colorscheme")
        .arg(if dark { "BreezeDark" } else { "BreezeLight" })
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return true;
    }

    // XFCE — uses xfconf for settings
    let xfce_theme = if dark { "Adwaita-dark" } else { "Adwaita" };
    if std::process::Command::new("xfconf-query")
        .args(["-c", "xsettings", "-p", "/Net/ThemeName", "-s", xfce_theme])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return true;
    }

    false // no supported desktop environment found
}

/// Get current dark mode state on Linux. Tries GNOME color-scheme, GNOME gtk-theme
/// (fallback), and KDE color scheme in order. Returns None if no DE detected.
#[cfg(target_os = "linux")]
pub fn get_dark_mode() -> Option<bool> {
    // GNOME: check color-scheme first (more reliable than theme name)
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

    // KDE: read the color scheme name
    if let Ok(output) = std::process::Command::new("kreadconfig5")
        .args(["--group", "General", "--key", "ColorScheme"])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            return Some(val.contains("dark"));
        }
    }

    None // couldn't detect theme on any DE
}
