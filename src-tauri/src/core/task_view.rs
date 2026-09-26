//! Platform shell overview and desktop actions.

#[cfg(target_os = "macos")]
const MACOS_MISSION_CONTROL_NOTIFICATION: &str = "com.apple.expose.awake";
#[cfg(target_os = "macos")]
const MACOS_SHOW_DESKTOP_NOTIFICATION: &str = "com.apple.showdesktop.awake";

/// Send one window-management notification directly to the macOS Dock.
///
/// Do not replace this with the Mission Control app launcher. Its numeric
/// arguments are private and easy to mis-map (`1` is Show Desktop while `2`
/// is App Expose), and a successful child spawn does not prove that the Dock
/// accepted the requested action. Calling the same Dock SPI directly uses
/// semantic notification names and returns an actionable status code.
#[cfg(target_os = "macos")]
fn send_macos_dock_notification(notification: &str) -> Result<(), String> {
    use std::ffi::{c_char, c_int, c_void, CString};
    use std::sync::OnceLock;

    type CoreDockSendNotification = unsafe extern "C" fn(*const c_void, c_int) -> c_int;
    const APPLICATION_SERVICES: &str =
        "/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices";
    const UTF8_ENCODING: u32 = 0x08000100;
    static HANDLE: OnceLock<usize> = OnceLock::new();

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            allocator: *const c_void,
            value: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        fn CFRelease(value: *const c_void);
        fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    let framework = CString::new(APPLICATION_SERVICES).map_err(|error| error.to_string())?;
    let handle = *HANDLE.get_or_init(|| unsafe { dlopen(framework.as_ptr(), 1) as usize });
    if handle == 0 {
        return Err("failed to open ApplicationServices".into());
    }

    let symbol = c"CoreDockSendNotification";
    let function = unsafe { dlsym(handle as *mut c_void, symbol.as_ptr()) };
    if function.is_null() {
        return Err("CoreDockSendNotification is unavailable".into());
    }

    let value = CString::new(notification).map_err(|error| error.to_string())?;
    let name = unsafe { CFStringCreateWithCString(std::ptr::null(), value.as_ptr(), UTF8_ENCODING) };
    if name.is_null() {
        return Err("failed to create Dock notification name".into());
    }

    let send: CoreDockSendNotification = unsafe { std::mem::transmute(function) };
    let result = unsafe { send(name, 0) };
    unsafe { CFRelease(name) };
    if result != 0 {
        return Err(format!("CoreDockSendNotification returned {result}"));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};

#[cfg(target_os = "windows")]
const WINDOWS_MODIFIER_RELEASE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Wait for physical shortcut modifiers to clear before emitting one Windows-key chord.
///
/// `SendInput` does not reset physical keyboard state. Injecting modifier key-up
/// events while the user still holds those keys desynchronizes Windows until the
/// keys are physically released, making the next shortcut appear unregistered.
#[cfg(target_os = "windows")]
fn send_windows_chord(key: VIRTUAL_KEY) -> Result<(), String> {
    let deadline = std::time::Instant::now() + WINDOWS_MODIFIER_RELEASE_TIMEOUT;
    while [VK_CONTROL, VK_MENU, VK_SHIFT]
        .into_iter()
        .any(|modifier| unsafe { GetAsyncKeyState(modifier.0 as i32) } < 0)
    {
        if std::time::Instant::now() >= deadline {
            return Err("timed out waiting for Ctrl, Alt, and Shift to be released".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

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

/// Open Windows Task View after physical modifier release by emitting Win+Tab.
#[cfg(target_os = "windows")]
pub fn open() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_TAB;
    std::thread::spawn(|| {
        if let Err(error) = send_windows_chord(VK_TAB) {
            log::warn!("task_view: {error}");
        }
    });
    Ok(())
}

/// Open macOS Mission Control through the Dock notification API.
#[cfg(target_os = "macos")]
pub fn open() -> Result<(), String> {
    send_macos_dock_notification(MACOS_MISSION_CONTROL_NOTIFICATION)
}

/// Report unsupported overview invocation outside Windows and macOS.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn open() -> Result<(), String> {
    Err("not supported on this platform".into())
}

/// Show or restore the Windows desktop after physical modifier release by emitting Win+D.
#[cfg(target_os = "windows")]
pub fn show_desktop() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_D;
    std::thread::spawn(|| {
        if let Err(error) = send_windows_chord(VK_D) {
            log::warn!("show_desktop: {error}");
        }
    });
    Ok(())
}

/// Show or restore the macOS desktop through the Dock notification API.
#[cfg(target_os = "macos")]
pub fn show_desktop() -> Result<(), String> {
    send_macos_dock_notification(MACOS_SHOW_DESKTOP_NOTIFICATION)
}

/// Report unsupported Show Desktop invocation outside Windows and macOS.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn show_desktop() -> Result<(), String> {
    Err("not supported on this platform".into())
}
