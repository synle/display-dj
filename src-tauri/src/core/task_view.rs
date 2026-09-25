//! Platform shell overview and desktop actions.

/// Post one notification understood by the macOS Mission Control launcher.
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn post_macos_dock_notification(notification: &str) -> Result<(), String> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::CString;

    let notification = CString::new(notification)
        .map_err(|_| "Mission Control notification contains a null byte".to_string())?;
    unsafe {
        let name: *mut Object = msg_send![class!(NSString), alloc];
        let name: *mut Object = msg_send![name, initWithUTF8String: notification.as_ptr()];
        if name.is_null() {
            return Err("failed to create Mission Control notification name".into());
        }
        let center: *mut Object = msg_send![class!(NSDistributedNotificationCenter), defaultCenter];
        if center.is_null() {
            let _: () = msg_send![name, release];
            return Err("distributed notification center unavailable".into());
        }
        let _: () =
            msg_send![center, postNotificationName: name object: std::ptr::null::<Object>()];
        let _: () = msg_send![name, release];
    }
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

/// Open macOS Mission Control through the Dock's distributed notification.
#[cfg(target_os = "macos")]
pub fn open() -> Result<(), String> {
    post_macos_dock_notification("com.apple.expose.awake")
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

/// Show or restore the macOS desktop through the Dock's distributed notification.
#[cfg(target_os = "macos")]
pub fn show_desktop() -> Result<(), String> {
    post_macos_dock_notification("com.apple.showdesktop.awake")
}

/// Report unsupported Show Desktop invocation outside Windows and macOS.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn show_desktop() -> Result<(), String> {
    Err("not supported on this platform".into())
}
