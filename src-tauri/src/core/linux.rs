use super::{DisplayControl, DisplayInfo, Platform, BUILTIN_ID, VCP_BRIGHTNESS, VCP_CONTRAST};
use std::fs;
use std::process::Command;

// =========================================================================
// Display server detection — Linux can run X11, Wayland, or neither (TTY).
// This affects which tool we use for gamma (xrandr vs wlr-randr).
// =========================================================================

// #[derive(Debug, Clone, Copy, PartialEq)] — these are all compile-time auto-generated:
//   Debug = printable with {:?}, Clone+Copy = value-type semantics (like a number),
//   PartialEq = supports == comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DisplayServer {
    X11,
    Wayland,
    Unknown,
}

/// Detect whether we're on X11 or Wayland by checking environment variables.
/// XDG_SESSION_TYPE is the most reliable, with WAYLAND_DISPLAY/DISPLAY as fallbacks.
fn detect_display_server() -> DisplayServer {
    // std::env::var returns Result<String, VarError> — Ok if the var exists
    if let Ok(session) = std::env::var("XDG_SESSION_TYPE") {
        match session.to_lowercase().as_str() {
            "wayland" => return DisplayServer::Wayland,
            "x11" => return DisplayServer::X11,
            _ => {} // empty match arm = do nothing, fall through
        }
    }
    // .is_ok() = the env var exists (we don't care about the value)
    if std::env::var("WAYLAND_DISPLAY").is_ok() { return DisplayServer::Wayland; }
    if std::env::var("DISPLAY").is_ok() { return DisplayServer::X11; }
    DisplayServer::Unknown
}

// =========================================================================
// Built-in display — backlight via sysfs (kernel interface) or brightnessctl.
// /sys/class/backlight/ exposes one directory per backlight device.
// =========================================================================

/// Info about a sysfs backlight device (one per built-in panel).
/// The max brightness varies wildly by driver: Intel uses 24000, some AMD use 255.
/// We normalize to 0-100% in the DisplayControl implementation.
struct BacklightInfo {
    device: String, // sysfs device name (e.g., "intel_backlight", "amdgpu_bl0")
    max: u32,       // max brightness value from /sys/class/backlight/<device>/max_brightness
}

/// Find the first working backlight device in /sys/class/backlight/.
/// Returns None on desktops (no backlight).
fn find_backlight() -> Option<BacklightInfo> {
    // .ok()? converts the Result from read_dir to Option, returning None if the dir doesn't exist.
    // .flatten() skips any DirEntry errors.
    for entry in fs::read_dir("/sys/class/backlight").ok()?.flatten() {
        let max_path = entry.path().join("max_brightness");
        if let Some(max) = fs::read_to_string(&max_path).ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
        {
            if max > 0 {
                return Some(BacklightInfo {
                    device: entry.file_name().to_string_lossy().into_owned(),
                    max,
                });
            }
        }
    }
    None
}

/// Built-in (laptop) display controller for Linux.
/// Reads/writes brightness via sysfs (/sys/class/backlight/) with brightnessctl
/// as a fallback when direct sysfs writes require elevated permissions.
struct BuiltinControl {
    backlight: BacklightInfo,
}

/// DisplayControl implementation for the built-in panel.
/// Mode parameter is ignored — built-in always uses sysfs/brightnessctl.
/// Brightness is stored as a raw driver value internally, converted to/from 0-100%.
impl DisplayControl for BuiltinControl {
    fn get_brightness(&mut self) -> Option<u32> {
        // Read the raw value from sysfs and convert to 0-100 percentage
        let path = format!("/sys/class/backlight/{}/brightness", self.backlight.device);
        let val: u32 = fs::read_to_string(&path).ok()?.trim().parse().ok()?;
        Some(((val as f64 / self.backlight.max as f64) * 100.0).round() as u32)
    }

    fn get_contrast(&mut self) -> Option<u32> { None }

    fn set_brightness(&mut self, value: u16, _mode: &str) -> bool {
        // Convert 0-100 percentage to the raw sysfs value
        let raw = ((value as f64 / 100.0) * self.backlight.max as f64).round() as u32;
        let path = format!("/sys/class/backlight/{}/brightness", self.backlight.device);
        // Try direct sysfs write first (needs write permission or udev rule).
        // Fall back to brightnessctl if we don't have permission.
        if fs::write(&path, raw.to_string()).is_ok() { return true; }
        Command::new("brightnessctl")
            .args(["set", &format!("{}%", value)])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn set_contrast(&mut self, _value: u16) -> bool { false }
    fn reset_gamma(&self) {}
}

// =========================================================================
// External monitor — ddcutil CLI for DDC/CI + xrandr/wlr-randr for gamma.
// Unlike macOS/Windows, Linux uses CLI tools instead of native APIs.
// =========================================================================

/// External monitor controller for Linux.
/// Uses ddcutil CLI for DDC/CI (hardware brightness) and xrandr/wlr-randr for
/// gamma (software brightness). Unlike macOS/Windows which use native APIs,
/// Linux relies on external CLI tools for both DDC and gamma.
struct ExternalControl {
    display_num: u32,                // ddcutil's display number (1-based)
    output_name: Option<String>,     // xrandr/wlr-randr output name (e.g., "HDMI-1")
    display_server: DisplayServer,   // X11 or Wayland — determines which gamma tool to use
    ddc_supported: bool,             // true if initial ddcutil VCP read succeeded
}

/// DisplayControl implementation for external monitors on Linux.
/// DDC operations shell out to `ddcutil`. Gamma operations use `xrandr` (X11),
/// `wlr-randr` (Wayland/wlroots), `wl-gammarelay-rs` (Wayland/D-Bus), or
/// XWayland xrandr as fallbacks.
impl DisplayControl for ExternalControl {
    fn get_brightness(&mut self) -> Option<u32> {
        read_ddcutil_vcp(self.display_num, VCP_BRIGHTNESS)
    }

    fn get_contrast(&mut self) -> Option<u32> {
        read_ddcutil_vcp(self.display_num, VCP_CONTRAST)
    }

    fn set_brightness(&mut self, value: u16, mode: &str) -> bool {
        let use_ddc = mode == "ddc" || mode == "force" || (mode == "auto" && self.ddc_supported);
        let use_gamma = mode == "gamma" || mode == "force" || (mode == "auto" && !self.ddc_supported);
        let mut ok = true;

        if use_ddc && self.ddc_supported {
            let ddc_val = if value == 0 { 1 } else { value };
            if !set_ddcutil_vcp(self.display_num, VCP_BRIGHTNESS, ddc_val) {
                ok = false;
            }
        }

        if use_gamma {
            // `ref` borrows the String inside Option without moving it out of self.
            // Without ref, the String would be moved and self.output_name would be invalid.
            if let Some(ref output) = self.output_name {
                if !set_gamma(output, value as u32, self.display_server) {
                    ok = false;
                }
            }
        }

        ok
    }

    fn set_contrast(&mut self, value: u16) -> bool {
        if !self.ddc_supported { return false; }
        set_ddcutil_vcp(self.display_num, VCP_CONTRAST, value)
    }

    fn reset_gamma(&self) {
        if let Some(ref output) = self.output_name {
            let _ = set_gamma(output, 100, self.display_server);
        }
    }
}

// =========================================================================
// ddcutil helpers — shell out to the ddcutil CLI for DDC/CI communication.
// ddcutil talks to monitors over the i2c-dev kernel interface.
// =========================================================================

/// Read a VCP register via ddcutil. Returns 0-100 percentage or None.
fn read_ddcutil_vcp(display_num: u32, vcp_code: u8) -> Option<u32> {
    let output = Command::new("ddcutil")
        .args(["getvcp", &format!("0x{:02x}", vcp_code), "--display", &display_num.to_string(), "--brief"])
        .output().ok()?;
    if !output.status.success() { return None; }
    // Parse ddcutil --brief output format: "VCP code_id C current_val max_val"
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 && parts[0] == "VCP" {
            let current = parts[3].parse::<f64>().ok()?;
            let max = parts[4].parse::<f64>().ok()?;
            if max > 0.0 { return Some(((current / max) * 100.0).round() as u32); }
        }
    }
    None
}

/// Write a VCP register via ddcutil. value is the raw DDC value (not percentage).
fn set_ddcutil_vcp(display_num: u32, vcp_code: u8, value: u16) -> bool {
    Command::new("ddcutil")
        .args(["setvcp", &format!("0x{:02x}", vcp_code), &value.to_string(), "--display", &display_num.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `ddcutil detect --brief` and parse the output into display info structs.
/// Each display gets a ddcutil display number and optional output name for gamma.
fn enumerate_ddcutil(display_server: DisplayServer) -> Vec<(DisplayInfo, ExternalControl)> {
    // Match guard: Ok(o) if condition — matches Ok AND checks the condition.
    // If either fails, falls through to _ which returns an empty vec.
    let output = match Command::new("ddcutil").args(["detect", "--brief"]).output() {
        Ok(o) if o.status.success() => o,
        _ => return vec![], // vec![] = empty Vec literal (like [] in JS)
    };

    // Parse ddcutil's line-based output format:
    //   "Display 1" starts a new display block
    //   "Monitor:" or "Model:" lines contain the display name
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut monitors = Vec::new();
    let mut current_num: Option<u32> = None;
    let mut current_name = String::new();

    for line in stdout.lines() {
        let line = line.trim();
        // .strip_prefix() returns Option<&str> — the rest of the string after the prefix,
        // or None if it doesn't match. Combined with if-let, checks and extracts in one step.
        if let Some(rest) = line.strip_prefix("Display ") {
            // Emit previous monitor before starting a new one
            if let Some(num) = current_num {
                push_monitor(&mut monitors, num, &current_name, display_server);
            }
            current_num = rest.trim().parse().ok();
            current_name = format!("External Display {}", rest.trim());
        }
        if line.starts_with("Monitor:") || line.starts_with("Model:") {
            // .splitn(2, ':') splits into at most 2 parts at the first colon
            if let Some(name) = line.splitn(2, ':').nth(1) {
                let name = name.trim();
                if !name.is_empty() { current_name = name.to_string(); }
            }
        }
    }
    // Don't forget the last monitor (no "Display N" line follows it)
    if let Some(num) = current_num {
        push_monitor(&mut monitors, num, &current_name, display_server);
    }

    monitors
}

/// Build a DisplayInfo + ExternalControl pair and push to the monitors vec.
fn push_monitor(
    monitors: &mut Vec<(DisplayInfo, ExternalControl)>,
    display_num: u32,
    name: &str,
    display_server: DisplayServer,
) {
    let brightness = read_ddcutil_vcp(display_num, VCP_BRIGHTNESS);
    let contrast = read_ddcutil_vcp(display_num, VCP_CONTRAST);
    let ddc_supported = brightness.is_some();
    let output_name = get_output_name(display_num, display_server);

    let info = DisplayInfo {
        id: display_num.to_string(),
        name: name.to_string(),
        display_type: "external".into(),
        brightness,
        contrast,
        ddc_supported,
    };
    let ctrl = ExternalControl { display_num, output_name, display_server, ddc_supported };
    monitors.push((info, ctrl));
}

// =========================================================================
// Gamma control — different tools for X11 and Wayland.
// xrandr is the standard X11 tool, wlr-randr works on wlroots-based
// compositors (Sway, Hyprland). Fallbacks exist for other setups.
// =========================================================================

/// Map a ddcutil display number to an X11/Wayland output name (e.g., "HDMI-1").
/// We filter out built-in outputs (eDP = embedded DisplayPort, LVDS = legacy laptop)
/// so external display numbering matches between ddcutil and xrandr/wlr-randr.
/// This mapping is needed because ddcutil and xrandr use different enumeration orders.
fn get_output_name(display_num: u32, server: DisplayServer) -> Option<String> {
    let outputs = match server {
        DisplayServer::X11 => get_xrandr_external_outputs(),
        DisplayServer::Wayland => get_wayland_external_outputs(),
        DisplayServer::Unknown => return None,
    };
    // .saturating_sub(1) converts 1-based ddcutil number to 0-based index (clamped to 0)
    outputs.into_iter().nth(display_num.saturating_sub(1) as usize)
}

/// Get external output names from xrandr (X11).
fn get_xrandr_external_outputs() -> Vec<String> {
    let output = match Command::new("xrandr").arg("--listactivemonitors").output() {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };
    // Parse xrandr output: skip header line, extract last word (output name) from each line.
    // Filter out eDP (embedded DisplayPort) and LVDS (laptop panels).
    String::from_utf8_lossy(&output.stdout)
        .lines().skip(1) // skip header "Monitors: N"
        .filter_map(|line| {
            // filter_map = map + filter combined — return Some(x) to keep, None to skip.
            // The ? operator returns None early if .last() is None.
            let name = line.split_whitespace().last()?;
            if name.starts_with("eDP") || name.starts_with("LVDS") { None }
            else { Some(name.to_string()) }
        })
        .collect()
}

/// Get external output names from Wayland. Tries wlr-randr first, then swaymsg.
fn get_wayland_external_outputs() -> Vec<String> {
    // wlr-randr works on Sway, Hyprland, and other wlroots-based compositors
    if let Ok(o) = Command::new("wlr-randr").output() {
        if o.status.success() {
            // wlr-randr output: each line that doesn't start with space is an output name
            let outputs: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.starts_with(' ') && !l.is_empty())
                .filter_map(|l| {
                    let name = l.split_whitespace().next()?;
                    if name.starts_with("eDP") || name.starts_with("LVDS") { None }
                    else { Some(name.to_string()) }
                })
                .collect();
            if !outputs.is_empty() { return outputs; }
        }
    }
    // Fallback: swaymsg (Sway only) — parse JSON-ish output for "name" fields
    if let Ok(o) = Command::new("swaymsg").args(["-t", "get_outputs", "-r"]).output() {
        if o.status.success() {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let outputs: Vec<String> = stdout.split("\"name\"")
                .skip(1)
                .filter_map(|s| {
                    let rest = s.strip_prefix(':')?.trim().trim_start_matches('"');
                    let name = rest.split('"').next()?;
                    if name.starts_with("eDP") || name.starts_with("LVDS") || name.is_empty() { None }
                    else { Some(name.to_string()) }
                })
                .collect();
            if !outputs.is_empty() { return outputs; }
        }
    }
    vec![]
}

/// Set gamma (software brightness) for an output. Tries multiple tools in order.
/// Returns true if any tool succeeded.
fn set_gamma(output: &str, brightness: u32, server: DisplayServer) -> bool {
    let val = format!("{:.2}", brightness.min(100) as f64 / 100.0); // e.g., "0.50"
    match server {
        DisplayServer::X11 => {
            // xrandr --brightness sets the software brightness multiplier
            Command::new("xrandr").args(["--output", output, "--brightness", &val])
                .output().map(|o| o.status.success()).unwrap_or(false)
        }
        DisplayServer::Wayland => {
            // Try wlr-randr first (most common Wayland compositor tool)
            if Command::new("wlr-randr").args(["--output", output, "--brightness", &val])
                .output().map(|o| o.status.success()).unwrap_or(false)
            { return true; }
            // Try wl-gammarelay-rs via D-Bus (works on any Wayland compositor)
            if Command::new("busctl").args([
                "--user", "set-property", "rs.wl-gammarelay", "/",
                "rs.wl.gammarelay", "Brightness", "d",
                &(brightness as f64 / 100.0).to_string(),
            ]).output().map(|o| o.status.success()).unwrap_or(false)
            { return true; }
            // Last resort: try xrandr via XWayland (GNOME Wayland has XWayland)
            if std::env::var("DISPLAY").is_ok() {
                return Command::new("xrandr").args(["--output", output, "--brightness", &val])
                    .output().map(|o| o.status.success()).unwrap_or(false);
            }
            false
        }
        DisplayServer::Unknown => false,
    }
}

/// Reset gamma on all external outputs to 100% (no dimming).
fn reset_gamma_all(server: DisplayServer) {
    let outputs = match server {
        DisplayServer::X11 => get_xrandr_external_outputs(),
        DisplayServer::Wayland => get_wayland_external_outputs(),
        DisplayServer::Unknown => return,
    };
    for output in outputs {
        let _ = set_gamma(&output, 100, server); // let _ = ignore Result
    }
}

// =========================================================================
// Platform implementation — discovers all displays on Linux
// =========================================================================

/// Linux platform implementation.
/// Discovers displays by combining sysfs backlight (built-in) with ddcutil (external).
/// No dedup needed — sysfs and ddcutil enumerate from completely different sources
/// (kernel backlight interface vs I2C bus), and eDP/LVDS outputs are filtered out
/// of the xrandr/wlr-randr external output list.
pub struct LinuxPlatform;

impl Platform for LinuxPlatform {
    /// Enumerate all displays on this Linux machine.
    /// Built-in: /sys/class/backlight/ sysfs interface.
    /// External: ddcutil CLI via i2c-dev kernel module.
    fn enumerate() -> Vec<(DisplayInfo, Box<dyn DisplayControl>)> {
        let mut result: Vec<(DisplayInfo, Box<dyn DisplayControl>)> = Vec::new();
        let display_server = detect_display_server();

        // --- Built-in display ---
        if let Some(backlight) = find_backlight() {
            let mut ctrl = BuiltinControl { backlight };
            let brightness = ctrl.get_brightness();
            let info = DisplayInfo {
                id: BUILTIN_ID.into(),
                // Include the sysfs device name for clarity (e.g., "intel_backlight")
                name: format!("Built-in Display ({})", ctrl.backlight.device),
                display_type: "builtin".into(),
                brightness,
                contrast: None,
                ddc_supported: false,
            };
            result.push((info, Box::new(ctrl)));
        }

        // --- External displays ---
        // enumerate_ddcutil runs `ddcutil detect` and parses the output
        for (info, ctrl) in enumerate_ddcutil(display_server) {
            result.push((info, Box::new(ctrl)));
        }

        result
    }

    /// Reset gamma on all external outputs to 100% (no dimming).
    /// Unlike macOS (single CoreGraphics call), Linux must reset each output individually
    /// via xrandr or wlr-randr.
    fn reset_all_gamma() {
        reset_gamma_all(detect_display_server());
    }

    /// Dump raw platform diagnostics for the `debug` command.
    /// Returns display server type, sysfs backlight info, ddcutil version and detect
    /// output, external output names, and raw xrandr/wlr-randr output.
    fn debug_info() -> serde_json::Value {
        let display_server = detect_display_server();

        // --- Backlight (built-in) ---
        let backlight = find_backlight().map(|bl| {
            let current_raw = fs::read_to_string(format!("/sys/class/backlight/{}/brightness", bl.device))
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok());
            serde_json::json!({
                "device": bl.device,
                "max_brightness": bl.max,
                "current_raw": current_raw,
            })
        });

        // --- ddcutil version ---
        let ddcutil_version = Command::new("ddcutil").arg("--version").output().ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(|s| s.to_string()));

        // --- ddcutil detect (full output) ---
        let ddcutil_detect = Command::new("ddcutil").args(["detect"]).output().ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

        // --- External output names ---
        let external_outputs = match display_server {
            DisplayServer::X11 => get_xrandr_external_outputs(),
            DisplayServer::Wayland => get_wayland_external_outputs(),
            DisplayServer::Unknown => vec![],
        };

        // --- Raw xrandr/wlr-randr output ---
        let randr_output = match display_server {
            DisplayServer::X11 => Command::new("xrandr").arg("--query").output().ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
            DisplayServer::Wayland => Command::new("wlr-randr").output().ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
            DisplayServer::Unknown => None,
        };

        serde_json::json!({
            "display_server": format!("{:?}", display_server),
            "backlight": backlight,
            "ddcutil_version": ddcutil_version,
            "ddcutil_detect_raw": ddcutil_detect,
            "external_output_names": external_outputs,
            "randr_output_raw": randr_output,
        })
    }
}
