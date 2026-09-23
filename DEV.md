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

### Replace the temporary macOS 27 tray patch with Tauri 2.12.0

Display DJ currently patches the published Tauri 2.11.5 crate so it can use `tray-icon` 0.25.1
and `muda` 0.20.0. This fixes macOS 27 left-click events while keeping the existing Tauri tray
and menu integration.

Upstream status as of 2026-09-22:

- The Tauri issue is closed: https://github.com/tauri-apps/tauri/issues/16035.
- The integration PR remains open, requires maintainer review, and has two failing Linux jobs:
  https://github.com/tauri-apps/tauri/pull/16088.
- Tauri release automation assigns this change to Tauri 2.12.0.
- Latest stable Tauri 2.11.6 still depends on `tray-icon` 0.24 and `muda` 0.19.
- No official merge or release date is published. If review and Linux fixes move promptly, merge
  may take days to a few weeks. Earliest plausible release is October 2026; use October-November
  2026 as the safer planning window. This is an estimate, not an upstream commitment.

Do not replace this with Tauri 3 alpha or a direct second `tray-icon` dependency. Tauri 3 is an
unstable major upgrade, while a direct dependency would not replace the version used internally by
Tauri 2.

Removal trigger: `cargo info tauri@2.12.0 --verbose` must show `tray-icon@0.25.1` or newer. Then:

1. Pin normal and dev Tauri dependencies to the published 2.12.0 release.
2. Align `@tauri-apps/api` and `@tauri-apps/cli` with the published 2.12 release.
3. Remove `[patch.crates-io]` and delete `src-tauri/vendor/tauri/`.
4. Remove vendor-specific formatter, linter, `.gitattributes`, and vendoring-documentation entries.
5. Regenerate Cargo/npm lockfiles and Tauri schemas.
6. Run the complete frontend, Rust, release-build, and tray-click validation gates.

## Coverage thresholds

CI enforces floors that trail main-branch measurement by ~10pp. Actual numbers are NOT mirrored here — read from source of truth:

- **Frontend** → `vite.config.ts` → `test.coverage.thresholds`. Reports in `coverage/`.
- **Backend** → `.github/workflows/build.yml`, `Rust coverage` step (`--fail-under-lines/-functions/-regions`). HTML in `src-tauri/target/llvm-cov-target/html/`.

Raising: measure current %, set floor ~10pp below, update both files. Never lower without keeping the ~10pp gap.

## Audio output selection

`src-tauri/src/core/audio_output/` owns the platform layer:

- macOS uses CoreAudio device UIDs, output-stream/alive/default-output capability checks, built-in transport metadata, and the default output/system-output properties. Devices listed in `UNSUPPORTED_OUTPUT_DEVICE_UIDS` (Microsoft Teams Audio and ZoomAudioDevice loopback endpoints) are always hidden and rejected. Shared preference application also forces native endpoint names containing `Steam Streaming` into the hidden group on every platform.
- Windows uses MMDevice for active render endpoints, `IPolicyConfig` for console/multimedia/communications defaults, and `IAudioEndpointVolume` for native volume and mute.
- Linux uses `pactl` sink names, `set-default-sink`, and best-effort migration of active sink inputs.

`src-tauri/src/volume.rs` exposes `get_audio_output_devices`, `set_audio_output_device`, `set_audio_output_device_state`, and `rename_audio_output_device`. Enumeration and switching run in `spawn_blocking`; saved labels and `enabled` / `disabled` / `hidden` states are overlaid after the OS result. Settings persist in `Preferences.audio_output_configs`, keyed by stable platform ID, while the selected device remains OS-owned. Missing state values from older preference files default to `enabled`.

`AppState.audio_output_state` caches the effective output snapshot. A backend refresh thread probes every 5 seconds, emits `audio-output-changed`, and rebuilds the tray menu only when that snapshot changes. `App.tsx` keeps output state separate from `fetch_all_state`; its visible-main-panel poll reads the shared cache as a fallback, prevents overlapping requests, and keeps the last successful state on errors.

`VolumeControl.tsx` renders a readonly `All Speakers (<enabled non-hidden count>)` section label (collapsed appends `- <selected name>`); expanded mode shows the selected name below the count as an editable blue link, then lists only other endpoints with padded radio targets, inline alias editing, and a tri-state selector. Output names share the monitor-name font size. Enabled devices sort first with built-in outputs leading, disabled devices follow, and hidden devices appear only after clicking "Show hidden outputs". The tray's **Output Device** submenu lists enabled outputs only and marks the selected endpoint with `●`.

## Platform pitfalls

### macOS — Chromium apps & `AXFocusedApplication` (Brave / Chrome / Edge / Arc)

**Symptom:** all `command/tile/*` shortcuts silently no-op when a Chromium browser is focused; other apps and OSes fine.

**Root cause:** system-wide AX lookup (`AXUIElementCreateSystemWide()` → `AXFocusedApplication` → `AXFocusedWindow`) returns `-25212` (`kAXErrorCannotComplete`) for Chromium — its sandboxed renderers aren't fully wired to the system-wide AX server. Windows (`GetForegroundWindow`) and Linux (`_NET_ACTIVE_WINDOW`) don't traverse AX trees, hence macOS-only.

**Fix (v7.0.24+):**

1. Fallback: on failure, get PID via `NSWorkspace.frontmostApplication`, then read `AXFocusedWindow` from an app-scoped element (`get_focused_window_via_frontmost_app`). Same approach as AeroSpace/Rectangle.
2. `_AXUIElementGetWindow` can return `wid == 0` for Chromium elements — `execute_tile` treats window_id as optional and proceeds without restore-state.
3. All AX failures log code + description via `ax_error_description`; `-25212` is tagged "Chromium-style" so regressions are grep-able.

Code: `src-tauri/src/tiling/macos.rs`. Verify live: focus Brave, hit Ctrl+Shift+Right, expect a log line like `AXFocusedApplication failed with AXError=-25212 … falling back to NSWorkspace.frontmostApplication`. If you instead see `set AXPosition(…) failed`, the lookup worked but the app refused the move (fullscreen-locked or undecorated window) — different problem.

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

### Windows Tile Snap

`tiling/windows.rs` installs a `SetWinEventHook` listener for system move/size start and end events. Resize-border gestures are rejected with `WM_NCHITTEST`; title-bar moves poll the physical cursor, use shared zone geometry from `tiling/mod.rs`, and render through one transparent `tiling/snap_overlay.rs` WebView per display. Lazy WebView creation dispatches to Tauri's main thread while Win32 polling stays on its monitor thread. Releasing inside a zone applies the exact preview rectangle to the original HWND.

`windows-app-manifest.xml` declares `asInvoker` with `uiAccess="false"`, embedded by `build.rs` through `tauri_build::WindowsAttributes`. Keeping the app at normal user integrity lets the global WinEvent monitor observe ordinary application drag events without a UAC prompt. Windows blocks normal-integrity processes from moving elevated windows, so users must explicitly run Display DJ as administrator when that behavior is required. Do not use `uiAccess="true"`: unsigned/current-user bundles do not meet Windows UIAccess signing and secure-install-location requirements. The manifest must retain Tauri's Common Controls v6 dependency.

Windows defaults `tileSnapEnabled` to false because native Windows Snap competes for the same edges. Users should disable **Settings → System → Multitasking → Snap windows** before opting in; README carries both UI and registry-command instructions.

Dynamic overlay labels must stay covered by `src-tauri/capabilities/default.json`: `tile-snap-overlay-*` for Tile Snap and `overlay-*` for brightness fallback. Tauri denies `event.listen` when a WebView label matches no capability, which leaves the window visible but unable to draw emitted state.

### Linux/X11 Tile Snap

`tiling/linux.rs` polls `QueryPointer` and `_NET_ACTIVE_WINDOW` geometry on a dedicated thread. Left-button gestures become Tile Snap drags only after 10px of cursor movement and two stable-size position samples; normal-window size changes reject border resizes. Maximized xfwm4 windows may resize once while restoring to normal bounds, so those pre-confirmation changes update the saved restore rectangle instead of cancelling the drag.

The monitor reuses `tiling/snap_overlay.rs` and shared geometry from `tiling/mod.rs`; release applies the exact preview via `_NET_MOVERESIZE_WINDOW`. It starts only with `$DISPLAY` and stays dormant when preferences disable Tile Snap. Wayland-only sessions remain unsupported.

## Crash Logging

Every Rust panic plus every macOS native crash (`.ips` from `~/Library/Logs/DiagnosticReports/`) lands in `{config_dir}/display-dj/crash.log` (same folder as `preferences.json`; "Open App Folder" reveals it). Panics are captured by an in-process `std::panic::set_hook`; native crashes are summarized into the same file by `crash_log.rs::import_macos_native_crashes` at each launch, so both crash modes appear chronologically without Console.app.

Each record carries timestamp, app version, OS/arch, thread, location, payload, backtrace, the last 80 lines of `debug.log`, and a full preferences snapshot (native crashes add incident id, exception type/signal, top 40 frames). `crash_log.rs::rotate_if_needed` trims at the next `==========` boundary past ~2 MB, newest preserved. Never gated on `debug_logging`.

Tauri commands: `get_crash_log` (contents as string), `open_crash_log` (opens in default editor, creates empty file if missing). Used by the About panel.

### Post-mortem: v7.0.26 SIGABRT in `GlobalObserverHandler`

Three field crashes: abort inside the NSEvent global-monitor ObjC block. `catch_unwind` around the Rust handler was inert because `[profile.release].panic = "abort"` — panics skipped the catch and hit `abort()`. **Fix (v7.0.29):** `panic = "unwind"` in release + defensive `state.displays.get()` bounds check.

**Rule:** any Rust closure wrapped in a foreign callback (ObjC block, C fn pointer, Win32 callback) requires `panic = "unwind"` or `catch_unwind` is documentation, not a safety net.
