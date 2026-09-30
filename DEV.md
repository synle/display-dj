# display-dj

Cross-platform desktop app for monitor brightness, contrast, dark mode, volume and audio-output selection, and window tiling control. Built with Tauri v2 (Rust backend) + React 19 + TypeScript + Vite 6 (WebView frontend). Platform code (DDC/CI, gamma, WMI, DisplayServices, audio, wallpaper) runs in-process under `src-tauri/src/core/`.

## Quick Start

Prerequisites: Node.js 20+, [Rust toolchain](https://rustup.rs/) (stable), and platform deps (Linux: see CONTRIBUTING).

```bash
npm install
npm run tauri dev       # Full app (Vite + Rust backend)
npm run dev             # Frontend only (localhost:1420)
npm test                # Frontend tests
npm run test:coverage   # Frontend coverage gate
cd src-tauri && cargo test                            # Backend tests
cd src-tauri && cargo llvm-cov --lib --summary-only   # Backend coverage
npm run tauri build     # Production build (.dmg / .exe / .deb / .AppImage)
```

## Development TODO

### Investigate Windows device battery indicators

[HaloBattery](https://github.com/HeyOkay/HaloBattery) demonstrates a feasible Windows-only path
for showing battery levels from wireless mice, headsets, controllers, and Bluetooth devices. Its
MIT-licensed implementation combines vendor-specific HID protocols, Windows.Gaming.Input/XInput,
and Windows Bluetooth PnP properties plus WinRT connection state. Useful parts to study include
provider isolation, transport deduplication, last-known-value handling, and diagnostics.

Do not copy the Python tray-window implementation into Display DJ. A future prototype should keep
device discovery and polling in a Windows-only Rust module under `src-tauri/src/core/`, expose one
normalized device snapshot through Tauri, and render it inside Display DJ's existing popup and tray
menu. Scope must name supported hardware explicitly: most HID battery protocols are vendor- and
model-specific, while generic Bluetooth coverage depends on battery data already exposed by
Windows. Any borrowed code must retain HaloBattery's MIT notice and receive hardware-backed tests
before shipping. No macOS or Linux support is implied by this investigation.

## Coverage thresholds

CI enforces floors that trail main-branch measurement by ~10pp. Actual numbers are NOT mirrored here — read from source of truth:

- **Frontend** → `vite.config.ts` → `test.coverage.thresholds`. Reports in `coverage/`.
- **Backend** → `.github/workflows/build.yml`, `Rust coverage` step (`--fail-under-lines/-functions/-regions`). HTML in `src-tauri/target/llvm-cov-target/html/`.

Raising: measure current %, set floor ~10pp below, update both files. Never lower without keeping the ~10pp gap.

## State, persistence, and scheduling

Platform reads and writes return errors when the OS backend cannot confirm the operation. `fetch_all_state` does not replace failures with empty monitors, light mode, or zero volume; frontend optimistic updates roll back and report the failure through an ARIA live status region. Aggregate brightness and contrast show an exact value only when applicable monitors agree, otherwise they show `Mixed`.

`save_preferences_to_disk` serializes writers, syncs temporary primary and backup files, and atomically replaces each destination. Malformed `preferences.json` files are quarantined as timestamped `.invalid.json` files; a valid `preferences.backup.json` is then recovered before defaults are used. `SettingsPanel` serializes and coalesces debounced snapshots, including a final close/unmount flush. Main-popup slider values are backend-owned `lastKnownValues`: `App.tsx` coalesces changes and calls `record_last_known_values` once after 200ms of quiet (flush on blur/unmount), and `save_preferences` keeps the in-memory copy. `restore_last_known_values()` in `lib.rs` re-applies them after the startup pre-warm.

Volume path: the volume `Slider` throttles live sends to 30ms (brightness/contrast keep a 50ms trailing debounce); `volume::set_volume` serializes writes with a ticket counter so a superseded request is skipped; macOS `core::volume` writes CoreAudio's virtual main volume via `core::audio_output::set_default_volume` (measured 1.0-4.3ms vs ~250ms for `osascript`, 2026-09-28), falling back to `osascript` on error.

Popup show: `tray::show_popup_window` no longer emits `monitors-changed` unconditionally. It calls `refresh_devices_on_show()`, which rescans displays and audio outputs in a background thread (one at a time) and emits change events only when the device list identity (`monitor_list_signature`: uid, name, hidden, brightness/contrast support) or audio state differs. The frontend keeps rendering its current lists until then.

Brightness path: `core::display` keeps enumerated display controls in `CONTROL_CACHE` so a write is one DDC transaction (measured ~55ms external, ~0.1ms built-in, 2026-09-28) instead of enumerate + EDID + 2 DDC reads + write. `list_all()` (startup, hot-plug, Force Refresh) refreshes the cache; a failed write drops it, re-enumerates once, and retries. `set_all_brightness` writes displays in parallel on scoped threads (`SendControl` wraps each control; access is exclusive). `display::set_monitor_brightness` / `set_all_monitors_brightness` serialize writes latest-wins per target and skip an identical value within 2s (`BrightnessTracker`).

Night mode stores the last successfully applied day/night phase in `AppState`. The 60-second poll executes actions only on startup or a phase transition; failed default brightness/theme writes leave the phase pending for the next tick. Schedule times require strict `HH:MM` and brightness values are sanitized to the configured safe range.

## Audio output selection

`src-tauri/src/core/audio_output/` owns the platform layer:

- macOS uses CoreAudio device UIDs, output-stream/alive/default-output capability checks, built-in transport metadata, and the default output/system-output properties. Devices listed in `UNSUPPORTED_OUTPUT_DEVICE_UIDS` (Microsoft Teams Audio and ZoomAudioDevice loopback endpoints) are always hidden and rejected. Shared preference application also forces native endpoint names containing `Steam Streaming` into the hidden group on every platform.
- Windows uses MMDevice for active render endpoints, `IPolicyConfig` for console/multimedia/communications defaults, and `IAudioEndpointVolume` for native volume and mute.
- Linux uses `pactl` sink names, `set-default-sink`, and best-effort migration of active sink inputs.

`src-tauri/src/volume.rs` exposes `get_audio_output_devices`, `set_audio_output_device`, `set_audio_output_device_state`, `save_audio_output_order`, and `rename_audio_output_device`. Enumeration and switching run in `spawn_blocking`; saved labels, `enabled` / `disabled` / `hidden` states, and optional sort positions are overlaid after the OS result. Settings persist in `Preferences.audio_output_configs`, keyed by stable platform ID, while the selected device remains OS-owned. Missing state values from older preference files default to `enabled`; missing sort positions retain the default state/built-in/name order.

`AppState.audio_output_state` caches the effective output snapshot. A backend refresh thread probes every 5 seconds, emits `audio-output-changed`, and rebuilds the tray menu only when that snapshot changes. `App.tsx` keeps output state separate from `fetch_all_state`; its visible-main-panel poll reads the shared cache as a fallback, prevents overlapping requests, and keeps the last successful state on errors.

When debug logging is enabled, `volume.rs` logs the initial snapshot and later changes with every device ID/name/state plus the selected ID. Explicit selection logs include the requested ID, cached before-state, refreshed after-state, active-ID confirmation or mismatch, and post-switch volume. On Windows, `core/audio_output/windows.rs` also logs the console, multimedia, and communications default endpoint IDs before and after `IPolicyConfig::SetDefaultEndpoint`, plus each role's HRESULT outcome. The unchanged 5-second refresh path emits nothing, preventing diagnostic polling from flooding `debug.log`.

`VolumeControl.tsx` owns a speaker expand/collapse chevron independent from the monitor section. Its readonly `All Speakers (<enabled non-hidden count>)` label always appends `- <selected name>` (collapsed and expanded); expanded mode lists visible endpoints with the selected radio checked, padded radio targets, inline alias editing, and reorder arrows. `SettingsPanel.tsx` lists monitors first, then every endpoint on the Monitors & Speakers tab (Night Mode Schedule is on the System tab) with the same arrows and a tri-state selector. Output names share the monitor-name font size. Enabled built-in outputs lead the initial order; `sortOrder` preserves later user ordering, and selection never affects row order. The tray's **Speakers** submenu lists enabled outputs with the selected endpoint marked `●`, then exposes Mute, 50%, and 100% presets through the normal volume command dispatcher.

`src/components/Dropdown.tsx` is the single frontend select primitive. Audio-output state, monitor brightness mode, wallpaper, slideshow, and Exposé strategy controls all use it; `.dropdown` in `App.css` owns their common 32px height, horizontal padding, typography, focus, hover, and disabled states.

Tray popup placement uses the physical mouse position from `TrayIconEvent::Click` to select the target monitor. Tauri can report mixed-DPI monitor origins and sizes in coordinate spaces that overlap (observed: Retina `(0,0) 3456x2234 @2x`, lower display `(-63,1117) 1920x1200 @1x`, click `(1303,1132)`). When one point falls inside multiple reported bounds, the display whose nearest edge is closest to the tray click wins; this works for top, bottom, left, and right tray edges. Exact coincident bounds have no geometric discriminator and retain stable monitor enumeration order. Every click state stores the latest anchor before action filtering because macOS opens its synchronous context menu on right-button down; left-click opening and context-menu **Show Window** then reuse that anchor. The popup starts below the tray, flips fully above when its lower edge would overflow, and clamps every corner inside that same monitor. `AppState` retains the click point and tray rectangle so content-driven resize events repeat the same placement. With Debug Logging enabled, left/right tray button-up events record button, mouse point, selected screen, all screen origins/resolutions with explicit `@Nx` DPI scaling, tray bounds, placement mode, popup rectangle, and final position.

## Platform pitfalls

### macOS — Chromium apps & `AXFocusedApplication` (Brave / Chrome / Edge / Arc)

**Symptom:** all `command/tile/*` shortcuts silently no-op when a Chromium browser is focused; other apps and OSes fine.

**Root cause:** system-wide AX lookup (`AXUIElementCreateSystemWide()` → `AXFocusedApplication` → `AXFocusedWindow`) returns `-25212` (`kAXErrorCannotComplete`) for Chromium — its sandboxed renderers aren't fully wired to the system-wide AX server. Windows (`GetForegroundWindow`) and Linux (`_NET_ACTIVE_WINDOW`) don't traverse AX trees, hence macOS-only.

**Fix (v7.0.24+):**

1. Fallback: on failure, get PID via `NSWorkspace.frontmostApplication`, then read `AXFocusedWindow` from an app-scoped element (`get_focused_window_via_frontmost_app`). Same approach as AeroSpace/Rectangle.
2. `_AXUIElementGetWindow` can return `wid == 0` for Chromium elements — `execute_tile` treats window_id as optional and proceeds without restore-state.
3. All AX failures log code + description via `ax_error_description`; `-25212` is tagged "Chromium-style" so regressions are grep-able.

Code: `src-tauri/src/tiling/macos.rs`. Verify live: focus Brave, hit Cmd+Ctrl+Right, expect a log line like `AXFocusedApplication failed with AXError=-25212 … falling back to NSWorkspace.frontmostApplication`. If you instead see `set AXPosition(…) failed`, the lookup worked but the app refused the move (fullscreen-locked or undecorated window) — different problem.

### Maximized / fullscreen focused-window tiling

Focused-window tiling normalizes OS-managed window states before reading the original bounds or applying the target geometry. Windows checks `IsZoomed` and calls `ShowWindow(SW_RESTORE)` without touching minimized windows. macOS clears native `AXFullScreen`, or sends Escape for browser/video pseudo-fullscreen, waits for the transition, reacquires the focused AX window, and then records its restored bounds. Linux/X11 removes EWMH horizontal maximize, vertical maximize, and fullscreen states, polls `_NET_WM_STATE` for bounded confirmation, and only then records the restored bounds or sends `_NET_MOVERESIZE_WINDOW`; `_NET_WM_STATE_HIDDEN` remains untouched.

Code: `src-tauri/src/tiling/{windows,macos,linux}.rs`. Exposé keeps its broader behavior and may also unminimize windows because every visible window must join the grid.

### Linux — running locally & first-build notes (verified on Mint 22.2 / XFCE / X11)

**Running the binary from a terminal or agent shell:** the app is a tray daemon with no window. A plain `./target/release/display-dj &` dies when the parent shell/session closes (no crash in the log — it just vanishes). Run fully detached:

```bash
cd src-tauri
setsid nohup ./target/release/display-dj > /tmp/display-dj.log 2>&1 < /dev/null &
```

Logs go to stdout (plus `debug.log` in the config dir when enabled). A clean startup ends with `startup probe + cache pre-warm complete` and `register_shortcuts: done — N registered, 0 failed`.

**Tray icon invisible on XFCE:** the panel needs the **Status Notifier/Indicator** plugin (`xfce4-indicator-plugin`, installed by default on Mint). The icon may also land under the notification-area collapse arrow (`^`). Right-click is reliable for the menu; left-click doesn't always fire (AppIndicator limitation, see Known Limitations in CONTRIBUTING).

**GTK GL warning at startup** (`Disabled hardware acceleration because GTK failed to initialize GL`) — common in VMs/remote-desktop sessions. Tauri falls back to software rendering; harmless unless you see visual glitches, in which case try launching with `WEBKIT_DISABLE_COMPOSITING_MODE=1`.

**Z-order self-test on X11:** launch with `DISPLAY_DJ_ZORDER_SELFTEST=1`; five seconds after startup it runs all 6 z-order commands on whatever window is focused and logs each step with a `[zorder-selftest]` prefix. Verified working on XFCE/xfwm4 — this exercises the same focused-window resolution and move dispatch as tiling.

**Generated schemas:** the first Tauri build on each platform regenerates `src-tauri/gen/schemas/*` and creates a platform file (`linux-schema.json` — tracked). Local builds may also reformat `capabilities.json` / `acl-manifests.json` (pretty vs compact) when the local tauri CLI version differs from the last committer's — content-equivalent churn; revert rather than commit formatting-only diffs of those two.

**Brightness paths on Linux (`core::linux.rs`):** built-in panels write `/sys/class/backlight/<device>/brightness` directly, falling back to the `brightnessctl` CLI on permission failure (user needs the `video` group); external monitors shell out to `ddcutil` over `i2c-dev` (needs `ddcutil`, `i2c-tools`, `i2c-dev` loaded + user in `i2c` group); gamma dimming uses `xrandr` on X11. Full setup + verify commands in CONTRIBUTING "Platform Setup".

**Linux verification status (as of v7.2.0):** build (`.deb` + `.AppImage`), global shortcuts, tray icon, z-order commands, and Exposé layout math all pass on Mint 22.2/XFCE/X11 with only a built-in eDP panel. DDC/CI against real external monitors remains untested. Linux Tile Snap now has X11 pointer/geometry polling plus shared WebView overlays; runtime verification on a real XFCE/X11 desktop remains required.

**Arch/KDE super-alpha diagnostics:** the existing Ubuntu 22.04 release jobs
already produce x64 + ARM64 AppImages, so no Arch-specific build is required.
KDE-aware backend selection prevents an installed `gsettings` binary from
shadowing Plasma: theme writes use `plasma-apply-colorscheme`, Plasma 6 reads
prefer `kreadconfig6`, and wallpaper writes use
`plasma-apply-wallpaperimage`. Enabling Debug Logging immediately emits a full
snapshot from `core::LinuxPlatform::debug_info()`: os-release identity,
desktop/session and package format, helper-tool availability, selected feature
backends, bounded command stdout/stderr, backlight state, DDC detection, and
X11/Wayland output probing. Normal log lines include thread, module, and source
line. Plasma Wayland remains diagnostic-only for tiling/global shortcuts and
KWin gamma control.

### Windows Tile Snap

`tiling/windows.rs` installs a `SetWinEventHook` listener for system move/size start and end events. Resize-border gestures are rejected with `WM_NCHITTEST`; title-bar moves poll the physical cursor, use shared zone geometry from `tiling/mod.rs`, and render through one transparent `tiling/snap_overlay.rs` WebView per display. Each WebView subscribes to a display-scoped event name supplied in its page URL, preventing one monitor's zone or preview payload from overwriting another's. Windows passes each monitor's effective DPI scale to the overlay, which converts physical Win32 rectangles into CSS pixels; this keeps bottom zones inside the work-area viewport above the taskbar. Maximized windows receive `SW_RESTORE` before their original drag bounds are captured, matching native Windows unsnap behavior. Lazy WebView creation dispatches to Tauri's main thread while Win32 polling stays on its monitor thread. Releasing inside a zone applies the exact preview rectangle to the original HWND.

`windows-app-manifest.xml` declares `asInvoker` with `uiAccess="false"`, embedded by `build.rs` through `tauri_build::WindowsAttributes`. Keeping the app at normal user integrity lets the global WinEvent monitor observe ordinary application drag events without a UAC prompt. Windows blocks normal-integrity processes from moving elevated windows, so users must explicitly run Display DJ as administrator when that behavior is required. Do not use `uiAccess="true"`: unsigned/current-user bundles do not meet Windows UIAccess signing and secure-install-location requirements. The manifest must retain Tauri's Common Controls v6 dependency.

### Platform shell shortcuts

`command/system/taskView` and `command/system/showDesktop` map to platform shell actions. Their global-shortcut callbacks dispatch when the arrow key is released. macOS sends `com.apple.expose.awake` or `com.apple.showdesktop.awake` directly to the Dock through `CoreDockSendNotification`, with defaults `Ctrl+Super+Shift+Up` and `Ctrl+Super+Shift+Down`. Windows Task View releases the triggering `Ctrl+Alt+Shift` modifiers before sending balanced `Win+Tab` events. Windows Show Desktop instead creates `IShellDispatch4` on a background COM thread and calls the documented `ToggleDesktop()` method, so trigger modifiers cannot alter that action.

Task View must inject key-up events for the triggering Windows modifiers immediately before `Win+Tab`. Waiting for `GetAsyncKeyState` to clear after the global-shortcut release callback timed out and suppressed Task View. Show Desktop avoids input injection entirely through `ToggleDesktop()`. No documented public Windows API opens Task View: public `IVirtualDesktopManager` only queries desktop membership and moves windows. Keep `Win+Tab` rather than adopting undocumented immersive-shell COM interfaces.

Default app z-order shortcuts follow the same platform modifier split: `Shift+Ctrl+Super+Left` / `Right` on macOS and `Shift+Ctrl+Alt+Left` / `Right` on Windows/Linux. `config.rs::default_zorder_keybindings_for_platform` owns this mapping so non-macOS builds never register the operating-system Super key for these actions.

Do not route the macOS actions through `/System/Applications/Mission Control.app/Contents/MacOS/Mission Control`. That private launcher's numeric map is `0` = Mission Control, `1` = Show Desktop, and `2` = App Expose; the old Show Desktop path incorrectly passed `2`. More importantly, process creation only proved that the launcher started: it supplied no reliable signal that the Dock accepted the action, and during manual debugging both launcher-backed shortcuts stopped changing macOS state while Display DJ continued logging successful dispatches. The direct SPI path matches what the launcher itself calls, avoids the ambiguous numeric contract and helper-process lifecycle, and returns a nonzero status when Dock delivery fails. Keep the notification-name assertions in `tray::tests::platform_shell_shortcuts_use_native_actions` when changing this path.

`tiling::get_windows_elevation_status()` queries the current process token only when Settings opens. `get_windows_snap_enabled()` reads `HKCU\Control Panel\Desktop\WindowArrangementActive`, and `open_windows_multitasking_settings()` opens `ms-settings:multitasking`. Optional-query failures log and return `None`; they never gate startup, tiling, or Settings. Settings stays quiet when macOS Accessibility, Windows elevation, and native-Snap requirements are satisfied; failures render a short linked line below the relevant checkbox.

Windows defaults `tileSnapEnabled` to false because native Windows Snap competes for the same edges. Users should disable **Settings → System → Multitasking → Snap windows** before opting in; README carries both UI and registry-command instructions.

Dynamic overlay labels must stay covered by `src-tauri/capabilities/default.json`: `tile-snap-overlay-*` for Tile Snap and `overlay-*` for brightness fallback. Tauri denies `event.listen` when a WebView label matches no capability, which leaves the window visible but unable to draw emitted state.

### Linux/X11 Tile Snap

`tiling/linux.rs` polls `QueryPointer` and `_NET_ACTIVE_WINDOW` geometry on a dedicated thread. Left-button gestures become Tile Snap drags only after 10px of cursor movement and two stable-size position samples; normal-window size changes reject border resizes. Maximized xfwm4 windows may resize once while restoring to normal bounds, so those pre-confirmation changes update the saved restore rectangle instead of cancelling the drag.

The monitor reuses `tiling/snap_overlay.rs` and shared geometry from `tiling/mod.rs`; release applies the exact preview via `_NET_MOVERESIZE_WINDOW`. It starts only with `$DISPLAY` and stays dormant when preferences disable Tile Snap. Wayland-only sessions remain unsupported.

## Crash Logging

Every Rust panic plus every macOS native crash (`.ips` from `~/Library/Logs/DiagnosticReports/`) lands in `{config_dir}/display-dj/crash.log` (same folder as `preferences.json`; "Open App Folder" reveals it). Panics are captured by an in-process `std::panic::set_hook`; native macOS crashes are summarized into the same file by `crash_log.rs::import_macos_native_crashes` at each launch, so both crash modes appear chronologically without Console.app. Native Linux crashes are not intercepted in-process; use the host's system-coredump tooling for those stacks.

Each record carries timestamp, app version, OS/arch, thread, location, payload, backtrace, the last 80 lines of `debug.log`, and a full preferences snapshot (native crashes add incident id, exception type/signal, top 40 frames). `crash_log.rs::rotate_if_needed` trims at the next `==========` boundary past ~2 MB, newest preserved. Never gated on `debug_logging`.

Tauri commands: `get_crash_log` (contents as string), `open_crash_log` (opens in default editor, creates empty file if missing). Used by the About panel.

### Post-mortem: v7.0.26 SIGABRT in `GlobalObserverHandler`

Three field crashes: abort inside the NSEvent global-monitor ObjC block. `catch_unwind` around the Rust handler was inert because `[profile.release].panic = "abort"` — panics skipped the catch and hit `abort()`. **Fix (v7.0.29):** `panic = "unwind"` in release + defensive `state.displays.get()` bounds check.

**Rule:** any Rust closure wrapped in a foreign callback (ObjC block, C fn pointer, Win32 callback) requires `panic = "unwind"` or `catch_unwind` is documentation, not a safety net.

## DPI Scaling (Beta)

Settings chips: `.beta-chip` (orange, beta) and `.beta-chip.platform-chip` (accent, e.g. `Windows`). `Tooltip` takes `ReactNode` text and follows the theme. Settings → Monitors is one section with `.settings-subheader` groups: **Min Brightness** slider, **Options** (Show Contrast Slider, Show DPI Settings), then **Displays** rows.

`core::dpi` changes per-display UI scale. Settings → Monitors holds **Show DPI Settings** with an orange `beta` chip (`showDpiSettings`, default off); the beta warning lives in its tooltip. When on, each monitor row shows name → DPI scale → brightness mode → Hide; DPI displays matching no monitor config render as DPI-only rows (`matchDpiDisplay` in `DpiSettings.tsx`). Settings has no reorder arrows for monitors or speakers. Not shown in the main popup.

- macOS and Windows show exactly the OS-offered scales (`DpiDisplay.continuous = false`); the user band is ignored.
- Linux reports `continuous = true`; `dpiOptionsFor` builds `dpiMinPercent..=dpiMaxPercent` at `dpiStepPercent` (defaults 60/250/5; caps 50–500, step 1–100 via `Preferences::sanitize()`). `set_display_dpi` enforces the band on Linux only. The band inputs render `display: none` — edit `preferences.json`.
- After an apply, on `devicePixelRatio` change, and on `WindowEvent::ScaleFactorChanged`, the popup refits and `reanchor_popup` (`tray.rs`) re-reads the live tray rect and re-places it.

- **macOS**: display-mode switch; scale % = native pixel width / "looks like" width (`CGConfigureDisplayWithDisplayMode`, permanent). Only real modes exist, so arbitrary steps / sub-100% usually unavailable. Built-in Retina defaults read as 200%.
- **Windows**: undocumented `DisplayConfigGetDeviceInfo`/`SetDeviceInfo` packet types -3/-4 over steps 100–500%.
- **Linux**: X11 only, `xrandr --output <name> --scale`; Wayland returns an error.

Commands: `get_dpi_displays`, `set_display_dpi { id, percent }`, `reanchor_popup_window`.
