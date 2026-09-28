//! Linux desktop-environment detection and command helpers shared by theme,
//! wallpaper, and platform diagnostics.

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

/// Desktop environment families with dedicated Display DJ backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinuxDesktop {
    Gnome,
    Kde,
    Xfce,
    Other,
}

impl LinuxDesktop {
    /// Stable lowercase name used in diagnostic output.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Gnome => "gnome-compatible",
            Self::Kde => "kde-plasma",
            Self::Xfce => "xfce",
            Self::Other => "other",
        }
    }
}

/// Detect the active desktop from standard session environment variables.
pub(crate) fn current() -> LinuxDesktop {
    let current_desktop = env_value("XDG_CURRENT_DESKTOP");
    let session_desktop = env_value("XDG_SESSION_DESKTOP");
    let kde_full_session = env_value("KDE_FULL_SESSION")
        .map(|value| value != "0")
        .unwrap_or(false);
    detect(
        current_desktop.as_deref(),
        session_desktop.as_deref(),
        kde_full_session,
    )
}

/// Classify desktop strings without reading process-global environment state.
fn detect(
    current_desktop: Option<&str>,
    session_desktop: Option<&str>,
    kde_full_session: bool,
) -> LinuxDesktop {
    let desktop = [current_desktop, session_desktop]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(":")
        .to_lowercase();

    if kde_full_session || contains_desktop_token(&desktop, &["kde", "plasma"]) {
        return LinuxDesktop::Kde;
    }
    if contains_desktop_token(&desktop, &["xfce", "xfce4"]) {
        return LinuxDesktop::Xfce;
    }
    if contains_desktop_token(
        &desktop,
        &["gnome", "ubuntu", "unity", "cinnamon", "budgie", "pop"],
    ) {
        return LinuxDesktop::Gnome;
    }
    LinuxDesktop::Other
}

/// Check colon-delimited desktop names for a known environment token.
fn contains_desktop_token(value: &str, expected: &[&str]) -> bool {
    value
        .split([':', ';'])
        .map(str::trim)
        .any(|token| expected.iter().any(|candidate| token.contains(candidate)))
}

/// Read one non-empty environment value for diagnostics or backend selection.
pub(crate) fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Return whether an executable can be resolved from the current PATH.
pub(crate) fn command_available(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|directory| {
        let candidate = directory.join(program);
        candidate
            .metadata()
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    })
}

/// Run one desktop command and emit bounded success/failure context through the
/// normal logger. The logger writes this context to `debug.log` only when debug
/// logging is enabled.
pub(crate) fn run_checked(program: &str, args: &[&str], operation: &str) -> bool {
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => {
            log::info!("{}: backend={} ok=true", operation, program);
            true
        }
        Ok(output) => {
            let stdout = bounded_text(&String::from_utf8_lossy(&output.stdout), 512);
            let stderr = bounded_text(&String::from_utf8_lossy(&output.stderr), 512);
            log::warn!(
                "{}: backend={} ok=false exit_code={:?} stdout={:?} stderr={:?}",
                operation,
                program,
                output.status.code(),
                stdout,
                stderr,
            );
            false
        }
        Err(error) => {
            log::warn!(
                "{}: backend={} ok=false spawn_error={}",
                operation,
                program,
                error,
            );
            false
        }
    }
}

/// Bound command diagnostics so one tool failure cannot flood `debug.log`.
fn bounded_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.trim().chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// KDE wins when Plasma appears in a colon-delimited desktop value.
    #[test]
    fn detects_kde_plasma_session() {
        assert_eq!(detect(Some("KDE:Plasma"), None, false), LinuxDesktop::Kde);
    }

    /// KDE_FULL_SESSION identifies Plasma when XDG desktop values are absent.
    #[test]
    fn detects_kde_full_session() {
        assert_eq!(detect(None, None, true), LinuxDesktop::Kde);
    }

    /// Ubuntu GNOME variants use the gsettings-compatible backend.
    #[test]
    fn detects_gnome_compatible_session() {
        assert_eq!(
            detect(Some("ubuntu:GNOME"), None, false),
            LinuxDesktop::Gnome
        );
    }

    /// XFCE sessions route to xfconf instead of opportunistic gsettings calls.
    #[test]
    fn detects_xfce_session() {
        assert_eq!(
            detect(Some("XFCE"), Some("xfce"), false),
            LinuxDesktop::Xfce
        );
    }

    /// Unknown compositors retain fallback probing behavior.
    #[test]
    fn leaves_unknown_session_unclassified() {
        assert_eq!(
            detect(Some("river"), Some("custom"), false),
            LinuxDesktop::Other
        );
    }

    /// Bounded diagnostics retain short text and mark truncated text.
    #[test]
    fn bounds_command_diagnostics() {
        assert_eq!(bounded_text(" short ", 10), "short");
        assert_eq!(bounded_text("abcdefgh", 4), "abcd…");
    }
}
