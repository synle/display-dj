# TODO - Feature Ideas

These are proposals, not shipped features or verified API contracts. Historical research below needs fresh platform/source checks before implementation.

Current behavior and constraints live in [ARCHITECTURE.md](ARCHITECTURE.md), [FEATURES.md](FEATURES.md), [DEV.md](DEV.md), and [AGENTS.md](AGENTS.md).

## Quick Wins

- **Scroll-to-adjust brightness on tray icon** — investigate whether the pinned tray backend exposes usable scroll events on each platform (unverified). Coalesce supported events through existing brightness routing; respect per-monitor modes and the brightness floor.
- **Tray tooltip showing current state** — show brightness %, volume %, dark/light mode in the tooltip (`TrayIconBuilder::tooltip()`) on each state change instead of a static label.
- **Per-monitor quick presets** — row of effective-minimum/25/50/75/100% pill buttons below each monitor slider for one-tap brightness.
- **Export / Import settings** — backup/restore `preferences.json` via Tauri's dialog plugin. Useful when migrating machines or sharing multi-monitor configs.
- **Volume presets in UI** — mute/25%/50%/75%/100% buttons below the volume slider (shortcuts Shift+F10-F12 exist; UI has no equivalent).

## Medium Effort

- **Additional profile schedules** — extend the existing night/day profile pair to more times or weekdays. The current 60-second worker already dispatches selected profiles on phase changes; this proposal is not the initial profile integration.
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
- **Event-driven display hot-plug handling** — add native topology notifications beyond current popup-show and explicit refresh rescans. Stable physical identity must precede automatic reapplication; current composite UIDs and same-name reconciliation are not sufficient for identical monitors.

## Existing gaps worth investigating

- **Clean lint and CI output** — address the migration's recorded lint findings, React async-test warnings, incomplete platform mocks, and Linux/Windows unused-code warnings. See the dated CI snapshot in `DEV.md`; successful builds are not warning-free builds. Consider a CI lint gate only after establishing an accepted clean baseline.
- **Physical monitor identity** — replace ambiguous runtime-ID/model-name matching with verified physical identity and a compatibility migration.
- **macOS/Linux brightness overlays** — populate monitor geometry; current missing geometry returns an error rather than dimming.
- **Truthful feedback** — main sliders retain requested values on hardware failure, Settings has no visible save-error badge, and About can misreport failed update checks.
- **Windows performance evidence** — the global mouse hook is gone; measure remaining input/CPU/overlay/driver hypotheses on Windows before changing another path.
- **Privacy-safe support output** — current crash logs include raw preferences and identifiers; inspect/redact manually until an export surface exists.

## Research Notes

The API names and feasibility estimates below are historical candidates (unverified), not implementation instructions or support guarantees.

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

Investigate optional hardware ambient-light sensors per platform. Availability and adapter contracts remain unverified; a camera fallback is an optional privacy-sensitive design, not a requirement.

- **macOS**: IORegistry `CurrentLux` property scan via `IOServiceGetMatchingServices` + `io-kit-sys`. Works on MacBooks/iMacs; undocumented private surface. No sensor on Mini/Studio/Pro.
- **Windows**: WinRT `LightSensor::GetDefault()` (`windows` crate, `Devices_Sensors` feature). Hardware availability varies; absence must leave manual controls usable.
- **Linux**: `/sys/bus/iio/devices/iio:device*/in_illuminance_raw` × `in_illuminance_scale`, or `iio-sensor-proxy` D-Bus. Driver/device support needs a named hardware matrix; no coverage percentage is established here.
- **Camera fallback**: capture one frame at app startup and popup open (no background polling), compute average luminance in memory, drop buffer immediately — nothing stored or transmitted. Requires camera permission; skip gracefully if busy.

Implementation: strictly opt-in; try ALS → camera → night-mode schedule; detect availability at startup and hide if absent; poll ALS every ~2-5s.
