# TODO - Feature Ideas

## Quick Wins

- **Scroll-to-adjust brightness on tray icon** — mouse wheel on the tray icon nudges all-monitor brightness by a step. Tauri's `TrayIconEvent` exposes scroll events; call `core::display::set_all_brightness` per tick.
- **Tray tooltip showing current state** — show brightness %, volume %, dark/light mode in the tooltip (`TrayIconBuilder::tooltip()`) on each state change instead of a static label.
- **Per-monitor quick presets** — row of 0/25/50/75/100% pill buttons below each monitor slider for one-tap brightness.
- **Export / Import settings** — backup/restore `preferences.json` via Tauri's dialog plugin. Useful when migrating machines or sharing multi-monitor configs.
- **Volume presets in UI** — mute/25%/50%/75%/100% buttons below the volume slider (shortcuts Shift+F10-F12 exist; UI has no equivalent).

## Medium Effort

- **Tile Snap on Windows** — edge snapping via `SetWinEventHook` or a low-level mouse hook. Windows Aero Snap covers halves/quarters only; ours would add thirds/custom ratios.
- **Scheduled profiles** — time-of-day schedule for any profile ("Focus" at 9 AM...). Generalizes night mode; the 60s timer loop in `lib.rs` already checks time.
- **Idle-based dimming** — auto-dim after inactivity, restore on input. macOS: `CGEventSourceSecondsSinceLastEventType`; Windows: `GetLastInputInfo`; Linux: `xprintidle`.
- **Battery-aware brightness** — reduce brightness on battery / low charge (`battery` crate). Could trigger a profile.
- **Brightness fade transitions** — animated fade between brightness levels via small DDC steps on a backend timer; frontend slider stays instant.
- **Monitor grouping** — group monitors under one slider (e.g., two side-by-side externals). Groups stored in `preferences.monitorConfigs`.
- **Notification on profile/schedule activation** — system notification so scheduled changes aren't mistaken for bugs. Tauri notification plugin.
- **Do Not Disturb / Focus Mode** — see research notes below.

## Larger Features

- **Monitor input switching** — DDC/CI VCP `0x60` write for HDMI1/DP1/USB-C switching; KVM-like shortcut workflows. Add to `core::DisplayControl`.
- **Wayland tiling support** — no universal protocol. Priority: KDE (D-Bus/KWin scripts via `zbus`), GNOME (shell extension, hardest), wlroots (`wlr-foreign-toplevel-management`). Sway/Hyprland users already have tiling.
- **Per-app dark mode rules** — toggle theme by foreground app. Poll `NSWorkspace.frontmostApplication` / `GetForegroundWindow` / `_NET_ACTIVE_WINDOW`; rules as `(app_name, dark)` pairs in preferences.
- **Remote control via local web UI** — phone/tablet control. v7+ has no HTTP server, so this means standing one up inside `lib.rs` calling `core::*`.
- **CLI companion commands** — terminal control of the running app. Options: second-instance argv routing via `tauri-plugin-single-instance`, or an in-process Unix socket / named pipe IPC.
- **Display hot-plug handling** — auto-detect connect/disconnect, re-apply saved configs, emit `monitors-changed`. Currently requires reopening the popup.

## Research Notes

### Do Not Disturb / Focus Mode

Feasible everywhere, no unified API. Orthogonal to Keep Awake (sleep prevention) — DND silences notifications only.

| Platform           | Approach                                                                                                                    | Feasibility | Permissions |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------- | ----------- | ----------- |
| macOS              | `shortcuts run "Toggle DND"` CLI (recommended); private `DoNotDisturbKit` as opt-in                                         | Moderate    | Automation  |
| Windows            | `winreg` write `HKCU\...\Notifications\Settings\NOC_GLOBAL_SETTING_TOASTS_ENABLED` (0=DND on), broadcast `WM_SETTINGCHANGE` | Excellent   | None        |
| Linux GNOME        | `gsettings set org.gnome.desktop.notifications show-banners false`                                                          | Excellent   | None        |
| Linux KDE          | `qdbus ... Notifications.Inhibit` or `kwriteconfig5`                                                                        | Good        | None        |
| Linux Dunst/others | `dunstctl set-paused toggle`, `makoctl`, `swaync-client --toggle-dnd`                                                       | Varies      | None        |

Implementation follows the Keep Awake pattern: new `dnd.rs` module, `AppState.dnd_active: Mutex<bool>`, `get_dnd`/`set_dnd` commands, `command/changeDND/{on,off,toggle}` routed through `tray::execute_command`, `DndToggle.tsx` next to `KeepAwakeToggle.tsx`. Detect support at startup; hide toggle if unsupported. On macOS, verify state after setting (Shortcuts can fail silently).

### Ambient Light Adaptation

Feasible but fragile on macOS, solid-but-rare on Windows, weakest on Linux. No cross-platform Rust crate. Camera fallback is a necessity on all platforms.

- **macOS**: IORegistry `CurrentLux` property scan via `IOServiceGetMatchingServices` + `io-kit-sys`. Works on MacBooks/iMacs; undocumented private surface. No sensor on Mini/Studio/Pro.
- **Windows**: WinRT `LightSensor::GetDefault()` (`windows` crate, `Devices_Sensors` feature). Clean API, but ALS hardware is optional (~50% of laptops, 0% desktops). Null return is expected.
- **Linux**: `/sys/bus/iio/devices/iio:device*/in_illuminance_raw` × `in_illuminance_scale`, or `iio-sensor-proxy` D-Bus. Driver support spotty (~10-20% of laptops); most auto-brightness projects here went webcam-first (Clight, wluma).
- **Camera fallback**: capture one frame at app startup and popup open (no background polling), compute average luminance in memory, drop buffer immediately — nothing stored or transmitted. Requires camera permission; skip gracefully if busy.

Implementation: strictly opt-in; try ALS → camera → night-mode schedule; detect availability at startup and hide if absent; poll ALS every ~2-5s.
