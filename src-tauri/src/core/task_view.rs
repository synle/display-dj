//! Windows Task View keyboard synthesis.

/// Open Windows Task View by releasing the triggering modifiers and emitting Win+Tab.
#[cfg(target_os = "windows")]
pub fn open() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
        VK_LWIN, VK_MENU, VK_SHIFT, VK_TAB,
    };

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
        key_input(VK_TAB, Default::default()),
        key_input(VK_TAB, KEYEVENTF_KEYUP),
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

/// Report unsupported Task View invocation outside Windows.
#[cfg(not(target_os = "windows"))]
pub fn open() -> Result<(), String> {
    Err("not supported on this platform".into())
}
