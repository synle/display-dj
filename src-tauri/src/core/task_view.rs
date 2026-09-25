//! Platform shell overview and desktop actions.

#[cfg(target_os = "macos")]
const MACOS_MISSION_CONTROL_LAUNCHER: &str =
    "/System/Applications/Mission Control.app/Contents/MacOS/Mission Control";
#[cfg(target_os = "macos")]
const MACOS_MISSION_CONTROL_ACTION: &str = "0";
#[cfg(target_os = "macos")]
const MACOS_SHOW_DESKTOP_ACTION: &str = "2";

/// Invoke one action through Apple's Mission Control launcher.
#[cfg(target_os = "macos")]
fn run_macos_mission_control_action(action: &'static str) -> Result<(), String> {
    let mut child = std::process::Command::new(MACOS_MISSION_CONTROL_LAUNCHER)
        .arg(action)
        .spawn()
        .map_err(|error| format!("failed to launch Mission Control action {action}: {error}"))?;
    std::thread::spawn(move || {
        if let Err(error) = child.wait() {
            log::warn!("failed to reap Mission Control action {action}: {error}");
        }
    });
    Ok(())
}

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};

/// Release shortcut modifiers and emit one Windows-key chord.
#[cfg(target_os = "windows")]
fn send_windows_chord(key: VIRTUAL_KEY) -> Result<(), String> {
    let key_input = |key, flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let inputs = [
        key_input(VK_CONTROL, KEYEVENTF_KEYUP),
        key_input(VK_MENU, KEYEVENTF_KEYUP),
        key_input(VK_SHIFT, KEYEVENTF_KEYUP),
        key_input(VK_LWIN, Default::default()),
        key_input(key, Default::default()),
        key_input(key, KEYEVENTF_KEYUP),
        key_input(VK_LWIN, KEYEVENTF_KEYUP),
    ];
    let inserted = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if inserted != inputs.len() as u32 {
        return Err(format!(
            "SendInput inserted {inserted} of {} keyboard events: {}",
            inputs.len(),
            std::io::Error::last_os_error()
        ));
    }

    Ok(())
}

/// Open Windows Task View by releasing the triggering modifiers and emitting Win+Tab.
#[cfg(target_os = "windows")]
pub fn open() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_TAB;
    send_windows_chord(VK_TAB)
}

/// Open macOS Mission Control through Apple's launcher.
#[cfg(target_os = "macos")]
pub fn open() -> Result<(), String> {
    run_macos_mission_control_action(MACOS_MISSION_CONTROL_ACTION)
}

/// Report unsupported overview invocation outside Windows and macOS.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn open() -> Result<(), String> {
    Err("not supported on this platform".into())
}

/// Show or restore the Windows desktop by emitting Win+D.
#[cfg(target_os = "windows")]
pub fn show_desktop() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_D;
    send_windows_chord(VK_D)
}

/// Show or restore the macOS desktop through Apple's Mission Control launcher.
#[cfg(target_os = "macos")]
pub fn show_desktop() -> Result<(), String> {
    run_macos_mission_control_action(MACOS_SHOW_DESKTOP_ACTION)
}

/// Report unsupported Show Desktop invocation outside Windows and macOS.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn show_desktop() -> Result<(), String> {
    Err("not supported on this platform".into())
}
