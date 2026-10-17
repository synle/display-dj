//! Platform screen-lock action for Windows and Linux.

/// Stable preference values for Linux lock backends, in fallback order.
#[cfg(any(target_os = "linux", test))]
const LINUX_LOCK_METHODS: [&str; 7] = [
    "loginctl",
    "gnome-screensaver-command",
    "busctl",
    "qdbus",
    "i3lock",
    "xsecurelock",
    "swaylock",
];

/// Lock the current Windows session through the native User32 API.
#[cfg(target_os = "windows")]
pub fn lock(_preferred_method: Option<&str>) -> Result<Option<String>, String> {
    unsafe { windows::Win32::System::Shutdown::LockWorkStation() }
        .map_err(|error| format!("LockWorkStation failed: {error}"))?;
    Ok(None)
}

/// Lock the Linux session, trying the cached backend first and then each fallback.
#[cfg(target_os = "linux")]
pub fn lock(preferred_method: Option<&str>) -> Result<Option<String>, String> {
    let methods = ordered_linux_methods(preferred_method);
    for method in methods {
        if run_linux_method(method) {
            return Ok(Some(method.to_string()));
        }
    }
    Err("no supported Linux screen-lock command succeeded".into())
}

/// Report that lock-screen dispatch is intentionally unavailable on macOS.
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn lock(_preferred_method: Option<&str>) -> Result<Option<String>, String> {
    Err("not supported on this platform".into())
}

/// Put the cached Linux backend first without allowing arbitrary commands.
#[cfg(any(target_os = "linux", test))]
fn ordered_linux_methods(preferred_method: Option<&str>) -> Vec<&'static str> {
    let preferred = preferred_method.and_then(|value| {
        LINUX_LOCK_METHODS
            .iter()
            .copied()
            .find(|method| *method == value)
    });
    preferred
        .into_iter()
        .chain(
            LINUX_LOCK_METHODS
                .iter()
                .copied()
                .filter(|method| Some(*method) != preferred),
        )
        .collect()
}

/// Run one known Linux lock command and require a successful exit status.
#[cfg(target_os = "linux")]
fn run_linux_method(method: &str) -> bool {
    let args: &[&str] = match method {
        "loginctl" => &["lock-session"],
        "gnome-screensaver-command" => &["-l"],
        "busctl" => &[
            "call",
            "org.gnome.ScreenSaver",
            "/org/gnome/ScreenSaver",
            "org.gnome.ScreenSaver",
            "Lock",
        ],
        "qdbus" => &["org.freedesktop.ScreenSaver", "/ScreenSaver", "Lock"],
        "i3lock" | "xsecurelock" | "swaylock" => &[],
        _ => return false,
    };
    crate::core::linux_desktop::run_checked(method, args, "screen_lock")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid cached method runs before the normal Linux fallback sequence.
    #[test]
    fn cached_linux_lock_method_runs_first() {
        let methods = ordered_linux_methods(Some("qdbus"));

        assert_eq!(methods[0], "qdbus");
        assert_eq!(methods.len(), LINUX_LOCK_METHODS.len());
        assert_eq!(methods.iter().filter(|method| **method == "qdbus").count(), 1);
    }

    /// Unknown preference values cannot become executable names.
    #[test]
    fn unknown_linux_lock_method_uses_default_order() {
        assert_eq!(
            ordered_linux_methods(Some("sh")),
            LINUX_LOCK_METHODS.to_vec()
        );
    }
}
