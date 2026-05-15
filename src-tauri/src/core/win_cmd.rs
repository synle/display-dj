//! Spawn helpers for Windows subprocesses (`powershell.exe`, `reg.exe`, etc.)
//! that must run silently without flashing a visible console window.
//!
//! display-dj's parent binary is compiled with `windows_subsystem = "windows"`
//! and therefore has no console of its own. Without `CREATE_NO_WINDOW` (the
//! Win32 process-creation flag `0x08000000`), every short-lived `powershell`
//! / `reg` spawn briefly allocates its own console window — a visible black
//! flash on every brightness change, volume change, theme toggle, or
//! wallpaper write.
//!
//! Always use `hidden_command(...)` instead of `std::process::Command::new(...)`
//! when shelling out from Windows-only code paths.

#![cfg(target_os = "windows")]

use std::os::windows::process::CommandExt;
use std::process::Command;

/// Win32 [`CREATE_NO_WINDOW`](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)
/// process-creation flag — instructs the OS to not create a console window
/// for a console-subsystem child process.
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Build a `Command` for `program` with `CREATE_NO_WINDOW` already applied,
/// so the spawned child runs without flashing a console window.
pub fn hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}
