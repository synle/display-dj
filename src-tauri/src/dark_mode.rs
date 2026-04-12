#[tauri::command]
pub fn get_dark_mode() -> Result<bool, String> {
    get_system_dark_mode()
}

#[tauri::command]
pub fn set_dark_mode(enabled: bool) -> Result<(), String> {
    set_system_dark_mode(enabled)
}

// ===========================================================================
// macOS: defaults + osascript
// ===========================================================================

#[cfg(target_os = "macos")]
fn get_system_dark_mode() -> Result<bool, String> {
    let output = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map_err(|e| format!("Failed to read dark mode: {}", e))?;

    // "Dark" if dark mode on; command exits non-zero when light mode (key absent)
    Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "Dark")
}

#[cfg(target_os = "macos")]
fn set_system_dark_mode(enabled: bool) -> Result<(), String> {
    let script = format!(
        "tell application \"System Events\" to tell appearance preferences to set dark mode to {}",
        if enabled { "true" } else { "false" }
    );
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| format!("Failed to set dark mode: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

// ===========================================================================
// Windows: Registry (AppsUseLightTheme / SystemUsesLightTheme)
// ===========================================================================

#[cfg(target_os = "windows")]
fn get_system_dark_mode() -> Result<bool, String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        .map_err(|e| format!("Failed to open registry key: {}", e))?;
    let value: u32 = key.get_value("AppsUseLightTheme").unwrap_or(1);
    Ok(value == 0) // 0 = dark, 1 = light
}

#[cfg(target_os = "windows")]
fn set_system_dark_mode(enabled: bool) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
            KEY_WRITE,
        )
        .map_err(|e| format!("Failed to open registry key for writing: {}", e))?;

    let value: u32 = if enabled { 0 } else { 1 };
    key.set_value("AppsUseLightTheme", &value)
        .map_err(|e| format!("Failed to set AppsUseLightTheme: {}", e))?;
    key.set_value("SystemUsesLightTheme", &value)
        .map_err(|e| format!("Failed to set SystemUsesLightTheme: {}", e))?;

    // Broadcast WM_SETTINGCHANGE so running apps update their theme immediately
    broadcast_theme_change();

    Ok(())
}

/// Notify running applications that the theme has changed.
#[cfg(target_os = "windows")]
fn broadcast_theme_change() {
    use std::process::Command;
    // Use PowerShell to broadcast WM_SETTINGCHANGE with "ImmersiveColorSet"
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class ThemeNotify {
    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    public static extern IntPtr SendMessageTimeout(
        IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
        uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
    public static void Notify() {
        UIntPtr result;
        SendMessageTimeout((IntPtr)0xffff, 0x001A, UIntPtr.Zero,
            "ImmersiveColorSet", 0x0002, 5000, out result);
    }
}
'@
[ThemeNotify]::Notify()
"#,
        ])
        .output();
}

// ===========================================================================
// Linux: gsettings (GNOME / GTK desktop environments)
// ===========================================================================

#[cfg(target_os = "linux")]
fn get_system_dark_mode() -> Result<bool, String> {
    // Try the modern GNOME color-scheme setting first
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(stdout.contains("prefer-dark"));
        }
    }

    // Fallback: check the GTK theme name for "dark"
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
        .map_err(|e| format!("gsettings not found: {}. This feature requires GNOME.", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    Ok(stdout.contains("dark"))
}

#[cfg(target_os = "linux")]
fn set_system_dark_mode(enabled: bool) -> Result<(), String> {
    let scheme = if enabled { "prefer-dark" } else { "default" };
    let output = std::process::Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.interface",
            "color-scheme",
            scheme,
        ])
        .output()
        .map_err(|e| format!("gsettings error: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "gsettings set color-scheme failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Also update GTK theme for apps that don't honor color-scheme
    let gtk_theme = if enabled { "Adwaita-dark" } else { "Adwaita" };
    let _ = std::process::Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.interface",
            "gtk-theme",
            gtk_theme,
        ])
        .output();

    Ok(())
}
