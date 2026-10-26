# Display DJ — Developer Reference

Cross-platform desktop app for monitor brightness, contrast, dark mode, volume and audio-output selection, and window tiling control. Built with Tauri v2 (Rust backend) + React 19 + TypeScript + Vite 8 (WebView frontend). Platform code (DDC/CI, gamma, WMI, DisplayServices, audio, wallpaper) runs in-process under `src-tauri/src/core/`.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the structural map and edit locations, and [AGENTS.md](AGENTS.md) for project-specific rules.

## Quick Start

See [CONTRIBUTING.md](CONTRIBUTING.md#quick-start) for Node/Rust requirements and platform setup. Dependency manifests and lockfiles own exact versions.

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

## Library upgrade log

This log records the 2026-10-26 dependency migration. Frontend libraries precede Tauri and Rust. Final unpushed history is consolidated into one frontend commit and one Tauri/Rust commit.
Versions are registry stable releases, not prereleases. Exact pins and lockfiles make accepted versions reproducible.

Baseline: Node v24.22.0, npm 11.19.1, Rust 1.99.1; build passed, 218 frontend tests passed, and 504 macOS Rust tests passed.
Frontend coverage: 88.88% lines and 84.53% branches. Initial npm audit: 8 affected packages (2 critical, 3 high, 3 moderate).

For each accepted frontend bump, run `npm run build`, `npm test`, `npm run format`, and `npm run format:check`.
For each Rust bump, additionally run `cargo check --locked` and `cargo test --locked` from `src-tauri/`.
Reused Rust results apply only while Rust inputs remain unchanged. Native hardware behavior and Windows/Linux builds require platform verification.
Dependency installs disable lifecycle scripts. Rollback consists of reverting the relevant library commit, then reinstalling from its lockfile.
No preferences or persisted-data migration is intended. Native startup smoke checks are deferred because startup restores real display/audio settings.

### Frontend

| Library                       | Before → accepted             | Breakage, fixes, and backtracking                                                                                                                                                                                                                                                                                           |
| ----------------------------- | ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `oxfmt`                       | 0.61.0 → 0.74.0               | Latest accepted; removes critical `tinypool` advisories. Existing configuration works; only one Tooltip test callback is reflowed. Build and all 218 tests pass; no behavior changes or backtracking.                                                                                                                       |
| `@testing-library/react`      | 16.3.2 → 16.3.3               | Latest patch; no API or test changes required. No backtracking.                                                                                                                                                                                                                                                             |
| `@testing-library/user-event` | 14.6.1 → 14.6.8               | Latest stable; no application API or test changes required. No backtracking.                                                                                                                                                                                                                                                |
| `@vitejs/plugin-react`        | 6.1.0 → 6.1.2                 | Latest stable; no application API or test changes required. No backtracking.                                                                                                                                                                                                                                                |
| `oxlint`                      | 1.79.0 → 1.83.0               | Latest 1.89 and each version down to 1.84 add one `consistent-function-scoping` finding. 1.83 exactly matches the 18 pre-existing lint errors. No source or rule changes. Build and 218 tests pass.                                                                                                                         |
| `@types/react`                | 19.2.14 → 19.3.0              | Latest stable; no application API or test changes required. No backtracking.                                                                                                                                                                                                                                                |
| `@types/react-dom`            | 19.2.3 → 19.3.0               | Latest stable; no application API or test changes required. No backtracking.                                                                                                                                                                                                                                                |
| `vite`                        | 8.2.2 → 8.3.5                 | Latest stable; existing React plugin and build configuration work unchanged. No backtracking.                                                                                                                                                                                                                               |
| `@testing-library/jest-dom`   | 6.9.1 → 6.9.1 (pinned)        | Tried 7.0.1 and 6.10.0; both pass locally but require Node ≥22, conflicting with Node-20 CI. 6.10.0 is also explicitly deprecated as an incorrect breaking minor. Retain 6.9.1; no test changes.                                                                                                                            |
| `jsdom`                       | 29.0.2 → 29.1.1               | Latest 30.1.3 requires Node `^22.22.2 \|\| ^24.15.0 \|\| >=26.0.0`; reject against Node-20 CI. Latest 29.x preserves the existing engine range and DOM tests.                                                                                                                                                               |
| `typescript`                  | 5.9.3 → 6.0.3                 | Tried 7.0.2: strict side-effect import checks report TS2882 for App.css, and native compiler removes `tsserver.js` required by the workspace VS Code SDK setting. Backtrack to latest 6.x to preserve editor tooling. Add existing `vite/client` asset declarations to `types`; no assertions weakened.                     |
| `vitest`                      | 4.1.5 → 4.1.12                | Latest 5.0.5 drops Node 20; accept latest compatible 4.x security backport. npm resolves matching coverage 4.1.12 in the lockfile, preserving separate declaration commits. Initial install warns `ERESOLVE overriding peer dependency`; final peer tree and clean install pass. All 218 tests and coverage pass unchanged. |
| `@vitest/coverage-v8`         | 4.1.5 → 4.1.12 (explicit pin) | Matching lockfile version already moved with Vitest. Pin its declaration separately; latest 5.0.5 requires the Node-22-only runner. Existing thresholds and include/exclude globs stay unchanged.                                                                                                                           |
| `react`                       | 19.2.5 → 19.3.1               | Latest stable; update runtime and renderer together to avoid exact-version runtime mismatch. No new transition, fragment-ref, server-component, or Trusted Types APIs adopted. Existing component tests cover the unchanged popup flows.                                                                                    |
| `react-dom`                   | 19.2.5 → 19.3.1               | Match React runtime; existing `createRoot` entry point remains valid. No application or assertion changes.                                                                                                                                                                                                                  |
| `undici` (transitive)         | 7.25.0 → 7.30.1               | Refresh within jsdom's existing 7.x range to clear remaining high advisory. No new direct dependency or override.                                                                                                                                                                                                           |

| `@tauri-apps/api` | 2.12.1 → 2.12.4 | Latest stable 2.x patch; existing invoke, event, and window APIs compile unchanged. |
| `@tauri-apps/cli` | 2.12.1 → 2.12.4 | Latest stable 2.x tooling patch; no bundling configuration changes. |
| `@tauri-apps/plugin-global-shortcut` | 2.3.1 → 2.4.1 | Latest stable 2.x minor; app registers shortcuts through Rust, with no JS call sites to migrate. Backend plugin upgrade follows separately. |
| `@tauri-apps/plugin-shell` | 2.3.5 → 2.5.0 | Latest stable 2.x minor adds optional process groups; no JS shell call sites or new process behavior enabled. |

React migration notes: https://github.com/react/react/releases/tag/v19.3.0 and https://github.com/react/react/releases/tag/v19.3.1.
TypeScript migration notes: https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/ and https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/.
Vitest security backport: https://github.com/vitest-dev/vitest/releases/tag/v4.1.12.

Formatter release notes: https://github.com/oxc-project/oxc/releases/tag/oxfmt_v0.74.0.

### Frontend validation outcome

`npm run build`, `npm run test:coverage` (218 passed; lines 88.88%, branches 84.53%), formatting, isolated `npm ci --ignore-scripts --prefer-offline`, and `npm ls` pass.
`npm audit` reports zero vulnerabilities after the transitive transport refresh. `npm run lint` still reports the original 18 errors; no new findings remain.
A built-page browser smoke attempt fails before rendering because the standalone browser lacks Tauri's `metadata` and `transformCallback` bridge.
This is not native-app startup verification. Native startup and hardware interactions remain unverified; startup can restore real display/audio/wallpaper state.

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

`fetch_all_state` propagates errors returned by its monitor/theme/volume wrappers; it does not independently substitute light mode or zero volume. This does not guarantee truthful errors from every lower layer: display detection can still return an empty list.

Main sliders deliberately show `lastKnownValues`, defaulting to 50%, rather than live hardware readings. Slider failures keep the requested value without rollback or a status bar. `App.tsx` passes `brightnessMixed={false}` and `contrastMixed={false}`; reusable component support for `Mixed` does not mean the main popup displays it.

`save_preferences_to_disk` serializes writers, syncs temporary primary and backup files, and atomically replaces each destination. Both files receive the same new snapshot; the backup is not a previous-version archive. Malformed primary JSON is quarantined before backup recovery or default fallback.

`SettingsPanel` coalesces full snapshots with a 100ms debounce and one in-flight save. Close/unmount requests a final flush. Failed saves retain the snapshot for a later flush; there is no save-status badge or visible persistence-error message. Initial loading failures separately show Retry.

`App.tsx` coalesces last-known patches for 200ms and flushes on blur/unmount. `save_preferences` preserves backend-owned last-known values, Linux lock-command cache, lock-binding migration marker, and slideshow resume path. Startup restoration re-applies per-monitor brightness, supported contrast, the last enabled/present speaker, and its recorded volume. Tray and shortcut changes do not update these slider records.

Volume path: the volume `Slider` throttles live sends to 30ms (brightness/contrast keep a 50ms trailing debounce); `volume::set_volume` serializes writes with a ticket counter so a superseded request is skipped; macOS `core::volume` writes CoreAudio's virtual main volume via `core::audio_output::set_default_volume` (measured 1.0-4.3ms vs ~250ms for `osascript`, 2026-09-28), falling back to `osascript` on error.

Popup show: `tray::show_popup_window` no longer emits `monitors-changed` unconditionally. It calls `refresh_devices_on_show()`, which rescans displays and audio outputs in a background thread (one at a time) and emits change events only when the device list identity (`monitor_list_signature`: uid, name, hidden, brightness/contrast support) or audio state differs. The frontend keeps rendering its current lists until then.

Brightness path: `core::display::CONTROL_CACHE` reuses enumerated handles, avoiding discovery before every write. Platform writes can still retry and verify DDC values; Windows force mode applies gamma even when DDC succeeds. `list_all()` refreshes handles on startup/rescan; a failed cached write triggers re-enumeration and retry. All-display brightness uses scoped parallel workers. Application-level writes serialize latest-wins per target and deduplicate identical values for 2 seconds. Historical timing figures do not establish current cross-platform latency.

Night mode stores the last applied day/night phase in `AppState`. Its worker sleeps 60 seconds before the first check, then acts on the first enabled check or a phase transition. Nonempty custom phase commands override the selected profile; a missing profile falls back to legacy brightness/theme values. Only that legacy fallback checks write success before recording the phase; custom commands and profiles record dispatch, not confirmed hardware completion. Failed legacy writes remain pending for the next tick. Schedule times require strict `HH:MM`; numeric values pass through `Preferences::sanitize()`.

### Live settings

`save_preferences` re-registers global shortcuts only when key bindings change, syncs autostart only when its value changes, and starts/stops/restarts changed slideshow settings. Tiling and Exposé commands read current preferences at invocation.

Tile Snap monitors start independently of the saved toggle, so enabling an existing monitor needs no restart. macOS can start its monitor after Accessibility recheck; Linux reconnects when X11 appears or drops. Active macOS/Windows drags refresh zone toggles and trigger sizes; Linux also refreshes ratios and gap mid-drag. macOS/Windows capture ratios and gap at drag confirmation/start, so those changes apply on the next drag. Settings refreshes optional Windows status queries on mount.

## Audio output selection

`src-tauri/src/core/audio_output/` owns the platform layer:

- macOS uses CoreAudio device UIDs, output-stream/alive/default-output capability checks, built-in transport metadata, and the default output/system-output properties. Devices listed in `UNSUPPORTED_OUTPUT_DEVICE_UIDS` (Microsoft Teams Audio and ZoomAudioDevice loopback endpoints) are always hidden and rejected. Shared preference application also forces native endpoint names containing `Steam Streaming` into the hidden group on every platform.
- Windows uses MMDevice for active render endpoints, private `IPolicyConfig` for console/multimedia/communications defaults, and `IAudioEndpointVolume` for native volume and mute. Keep the unsupported default-switching interface isolated; no AudioDeviceCmdlets installation is required.
- Linux uses `pactl` sink names, `set-default-sink`, and best-effort migration of active sink inputs.

`src-tauri/src/volume.rs` exposes `get_audio_output_devices`, `set_audio_output_device`, `set_audio_output_device_state`, `save_audio_output_order`, and `rename_audio_output_device`. Enumeration and switching run in `spawn_blocking`; saved labels, `enabled` / `disabled` / `hidden` states, and optional sort positions are overlaid after the OS result. Settings persist in `Preferences.audio_output_configs`, keyed by stable platform ID, while live selection remains OS-owned. `lastKnownValues.selectedSpeakerId` separately records the popup's last selection for startup restoration. Missing state values from older preference files default to `enabled`; missing sort positions retain the default state/built-in/name order.

`AppState.audio_output_state` caches the effective output snapshot. A backend refresh thread probes every 5 seconds, emits `audio-output-changed`, and rebuilds the tray menu only when that snapshot changes. `App.tsx` keeps output state separate from `fetch_all_state`; its visible-main-panel poll reads the shared cache as a fallback, prevents overlapping requests, and keeps the last successful state on errors.

When debug logging is enabled, `volume.rs` logs the initial snapshot and later changes with every device ID/name/state plus the selected ID. Explicit selection logs include the requested ID, cached before-state, refreshed after-state, active-ID confirmation or mismatch, and post-switch volume. On Windows, `core/audio_output/windows.rs` also logs the console, multimedia, and communications default endpoint IDs before and after `IPolicyConfig::SetDefaultEndpoint`, plus each role's HRESULT outcome. The unchanged 5-second refresh path emits nothing, preventing diagnostic polling from flooding `debug.log`.

`VolumeControl.tsx` owns a speaker expand/collapse chevron independent from the monitor section. Its readonly `All Speakers (<enabled non-hidden count>)` label appends `- <selected name>` when known, in collapsed and expanded modes; expanded mode lists visible endpoints with the selected radio checked, padded radio targets, inline alias editing, and reorder arrows. `SettingsPanel.tsx` lists monitors first, then every endpoint on the Monitors & Speakers tab with a tri-state selector. Settings has no rename or reorder controls; the main popup owns both. Hidden endpoints are recoverable only in Settings. Night Mode and the single selected-profile editor live on the System tab. Output names share the monitor-name font size. Enabled built-in outputs lead the initial order; `sortOrder` preserves later user ordering, and selection never affects row order. The tray's **Speakers** submenu lists enabled outputs with the selected endpoint marked `●`, then exposes Mute, 50%, and 100% presets through the normal volume command dispatcher.

`src/components/Dropdown.tsx` is the single frontend select primitive. Audio-output state, monitor brightness mode, wallpaper, slideshow, and Exposé strategy controls all use it; `.dropdown` in `App.css` owns their common 32px height, horizontal padding, typography, focus, hover, and disabled states.

Tray popup placement uses the physical mouse position from `TrayIconEvent::Click` to select the target monitor. Tauri can report mixed-DPI monitor origins and sizes in coordinate spaces that overlap (observed: Retina `(0,0) 3456x2234 @2x`, lower display `(-63,1117) 1920x1200 @1x`, click `(1303,1132)`). When one point falls inside multiple reported bounds, the display whose nearest edge is closest to the tray click wins; this works for top, bottom, left, and right tray edges. Exact coincident bounds have no geometric discriminator and retain stable monitor enumeration order. Every click state stores the latest anchor before action filtering because macOS opens its synchronous context menu on right-button down; left-click opening and context-menu **Show Window** then reuse that anchor. The popup starts below the tray, flips fully above when its lower edge would overflow, and clamps every corner inside that same monitor. `AppState` retains the click point and tray rectangle so content-driven resize events repeat the same placement. With Debug Logging enabled, left/right tray button-up events record button, mouse point, selected screen, all screen origins/resolutions with explicit `@Nx` DPI scaling, tray bounds, placement mode, popup rectangle, and final position.

## Platform pitfalls

### Monitor identity and Settings rows

Monitor UIDs combine runtime API ID and model name, not a physical serial. `display::dedupe_moved_configs` transfers stale same-name metadata to the live UID unless its label is already customized, then removes stale duplicates. Disconnected metadata with no live name match remains saved. Identical-model monitors can still be ambiguous; do not treat this heuristic as stable physical identity.

Settings loads monitors before preferences and filters metadata by live UIDs after a successful nonempty result. An empty or failed probe leaves the filter unset, so saved rows can still appear. DPI pairing claims all hardware-ID matches first, then name/built-in fallbacks, and never assigns one DPI display twice.

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

### Native FFI declarations and platform-only compilation

Core Foundation typedefs must retain their C ABI widths across every Rust declaration. `CFIndex` and `CFNumberType` are pointer-sized, so `tiling/macos.rs` defines both through `isize`; declaring the same symbol with `i64` in one module and `isize` in another triggers `clashing_extern_declarations` and risks an ABI mismatch. Regression test `macos_core_foundation_array_signatures_use_cfindex` pins the array signatures.

The NSEvent callback uses exact-pinned `block2` and an `RcBlock` retained for process lifetime. Do not restore `block` v0.1: Rust reports its `_NSConcreteStackBlock` declaration as future-incompatible. The callback accepts `*mut c_void` at the block boundary and casts to the legacy `objc` object type inside, keeping `block2` encoding separate from `objc` v0.2.

Host compilation cannot validate code hidden behind another platform's `cfg`. Before changing Win32 bindings, read the installed `windows` crate source for the exact module and generated integer types, then require `cargo test --target <windows-target> --no-run` in Windows CI. Treat `clashing_extern_declarations` and Cargo future-incompatibility notices as release defects rather than harmless warning noise.

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

Title-bar clicks and double-click maximize/restore remain native Windows behavior. The former global low-level mouse hook was removed: it ran on the polling thread, whose sleeps and synchronous cross-process hit tests could hold up input delivery even while Tile Snap was disabled. Keep move/size observation separate from the global mouse input path.

Hook removal proves that input dependency is gone, not that every Windows latency cause is fixed. Native pointer latency, first-overlay creation, driver effects, and held-shortcut CPU behavior need Windows measurements. Compare app closed/running, Tile Snap off/on, first/subsequent drags, and the actual running version before selecting another fix.

`tiling/windows.rs` installs a `SetWinEventHook` listener for system move/size start and end events. Resize-border gestures are rejected with `WM_NCHITTEST`; title-bar moves poll the physical cursor, use shared zone geometry from `tiling/mod.rs`, and render through one transparent `tiling/snap_overlay.rs` WebView per display. Each WebView subscribes to a display-scoped event name supplied in its page URL, preventing one monitor's zone or preview payload from overwriting another's. Windows passes each monitor's effective DPI scale to the overlay, which converts physical Win32 rectangles into CSS pixels; this keeps bottom zones inside the work-area viewport above the taskbar. Maximized windows receive `SW_RESTORE` before their original drag bounds are captured, matching native Windows unsnap behavior. Lazy WebView creation dispatches to Tauri's main thread while Win32 polling stays on its monitor thread. Releasing inside a zone applies the exact preview rectangle to the original HWND.

`windows-app-manifest.xml` declares `asInvoker` with `uiAccess="false"`, embedded by `build.rs` through `tauri_build::WindowsAttributes`. Keeping the app at normal user integrity lets the global WinEvent monitor observe ordinary application drag events without a UAC prompt. Windows blocks normal-integrity processes from moving elevated windows, so users must explicitly run Display DJ as administrator when that behavior is required. Do not use `uiAccess="true"`: unsigned/current-user bundles do not meet Windows UIAccess signing and secure-install-location requirements. The manifest must retain Tauri's Common Controls v6 dependency.

### Platform shell shortcuts

`command/system/taskView` and `command/system/showDesktop` map to platform shell actions. Their global-shortcut callbacks dispatch when the arrow key is released. macOS sends `com.apple.expose.awake` or `com.apple.showdesktop.awake` directly to the Dock through `CoreDockSendNotification`, with defaults `Ctrl+Super+Shift+Up` and `Ctrl+Super+Shift+Down`. Windows Task View releases the triggering `Ctrl+Alt+Shift` modifiers before sending balanced `Win+Tab` events. Windows Show Desktop instead creates `IShellDispatch4` on a background COM thread and calls the documented `ToggleDesktop()` method, so trigger modifiers cannot alter that action.

`command/system/lockScreen` locks the current session on Windows and Linux and defaults to `Ctrl+Alt+Q` on those platforms only. Windows calls `LockWorkStation` directly. Linux tries `loginctl`, GNOME screensaver, GNOME D-Bus, KDE D-Bus, `i3lock`, `xsecurelock`, then `swaylock`; the first command with a successful exit is stored in backend-owned `linuxLockCommand` and tried first next time. Unknown hand-edited values are ignored rather than executed.

`Header.tsx` exposes the same action as a logout-style icon left of Settings after `get_about_info` confirms Windows or Linux. The two 32px actions have a 14px gap to reduce accidental locking.

Task View must inject key-up events for the triggering Windows modifiers immediately before `Win+Tab`. Waiting for `GetAsyncKeyState` to clear after the global-shortcut release callback timed out and suppressed Task View. Show Desktop avoids input injection entirely through `ToggleDesktop()`. No documented public Windows API opens Task View: public `IVirtualDesktopManager` only queries desktop membership and moves windows. Keep `Win+Tab` rather than adopting undocumented immersive-shell COM interfaces.

Default app z-order shortcuts follow the same platform modifier split: `Shift+Ctrl+Super+Left` / `Right` on macOS and `Shift+Ctrl+Alt+Left` / `Right` on Windows/Linux. `config.rs::default_zorder_keybindings_for_platform` owns this mapping so non-macOS builds never register the operating-system Super key for these actions.

Do not route the macOS actions through `/System/Applications/Mission Control.app/Contents/MacOS/Mission Control`. That private launcher's numeric map is `0` = Mission Control, `1` = Show Desktop, and `2` = App Expose; the old Show Desktop path incorrectly passed `2`. More importantly, process creation only proved that the launcher started: it supplied no reliable signal that the Dock accepted the action, and during manual debugging both launcher-backed shortcuts stopped changing macOS state while Display DJ continued logging successful dispatches. The direct SPI path matches what the launcher itself calls, avoids the ambiguous numeric contract and helper-process lifecycle, and returns a nonzero status when Dock delivery fails. Keep the notification-name assertions in `tray::tests::platform_shell_shortcuts_use_native_actions` when changing this path.

`tiling::get_windows_elevation_status()` queries the current process token only when Settings opens. `get_windows_snap_enabled()` reads `HKCU\Control Panel\Desktop\WindowArrangementActive`, and `open_windows_multitasking_settings()` opens `ms-settings:multitasking`. Optional-query failures log and return `None`; they never gate startup, tiling, or Settings. Settings stays quiet when macOS Accessibility, Windows elevation, and native-Snap requirements are satisfied; failures render a short linked line below the relevant checkbox.

Windows defaults `tileSnapEnabled` to false because native Windows Snap competes for the same edges. Users should disable **Settings → System → Multitasking → Snap windows** before opting in; README carries both UI and registry-command instructions.

Dynamic overlay labels must stay covered by `src-tauri/capabilities/default.json`: `tile-snap-overlay-*` for Tile Snap and `overlay-*` for brightness fallback. Tauri denies `event.listen` when a WebView label matches no capability, which leaves the window visible but unable to draw emitted state.

### Linux/X11 Tile Snap

`tiling/linux.rs` polls `QueryPointer` and `_NET_ACTIVE_WINDOW` geometry on a dedicated thread. Left-button gestures become Tile Snap drags only after 10px of cursor movement and two stable-size position samples; normal-window size changes reject border resizes. Maximized xfwm4 windows may resize once while restoring to normal bounds, so those pre-confirmation changes update the saved restore rectangle instead of cancelling the drag.

The monitor reuses `tiling/snap_overlay.rs` and shared geometry from `tiling/mod.rs`; release applies the exact preview via `_NET_MOVERESIZE_WINDOW`. The worker starts even without `$DISPLAY`, retries X11 connections, and stays dormant when preferences disable Tile Snap. Wayland-only tiling remains unsupported.

## Crash Logging

Crash logging attempts to persist Rust panics and import matching macOS native reports (`.ips` from `~/Library/Logs/DiagnosticReports/`) into `crash.log` beside `preferences.json`. File-write failures can still prevent a record; this is not guaranteed crash capture. Panics are captured by an in-process `std::panic::set_hook`; native macOS crashes are summarized into the same file by `crash_log.rs::import_macos_native_crashes` at each launch, so both crash modes appear chronologically without Console.app. Native Linux crashes are not intercepted in-process; use the host's system-coredump tooling for those stacks.

Each record carries timestamp, app version, OS/arch, thread, location, payload, backtrace, the last 80 lines of `debug.log`, and a full preferences snapshot (native crashes add incident id, exception type/signal, top 40 frames). `crash_log.rs::rotate_if_needed` trims at the next `==========` boundary past ~2 MB, newest preserved. Never gated on `debug_logging`.

Tauri commands `get_crash_log` and `open_crash_log` are registered, but the current About panel does not invoke them. Open App Folder exposes the file. Crash snapshots contain raw preferences, paths, aliases, commands, and device identifiers; inspect and redact before sharing.

`config::debug_log_path()` selects `debug.log` for official builds and `debug-dev.log` otherwise. Error-level records persist regardless of the Debug Logging toggle, subject to logger filtering; lower levels honor the toggle. The same build-mode flag affects beta artifacts: without `TAURI_RELEASE=true`, `build.rs` emits a DEV timestamp suffix, not a beta-SHA suffix.

About's update check can display “Up to date” when release lookup fails or returns an unknown tag. That badge alone is not proof of a successful check.

### Post-mortem: v7.0.26 SIGABRT in `GlobalObserverHandler`

Three field crashes: abort inside the NSEvent global-monitor ObjC block. `catch_unwind` around the Rust handler was inert because `[profile.release].panic = "abort"` — panics skipped the catch and hit `abort()`. **Fix (v7.0.29):** `panic = "unwind"` in release + defensive `state.displays.get()` bounds check.

**Rule:** any Rust closure wrapped in a foreign callback (ObjC block, C fn pointer, Win32 callback) requires `panic = "unwind"` or `catch_unwind` is documentation, not a safety net.

## DPI Scaling (Beta)

Settings chips: `.beta-chip` (orange, beta) and `.beta-chip.platform-chip` (accent, e.g. `Windows`). `Tooltip` takes `ReactNode` text and follows the theme. Settings → Monitors is one section with `.settings-subheader` groups: **Min Brightness** slider, **Options** (Show Contrast Slider, Show DPI Settings), then **Displays** rows.

`core::dpi` changes per-display UI scale. Settings → Monitors holds **Show DPI Settings** with an orange `beta` chip (`showDpiSettings`, default off); the beta warning lives in its tooltip. When on, each monitor row shows name → DPI scale → brightness mode → Hide; DPI displays matching no monitor config render as DPI-only rows (`pairDpiDisplays` / `matchDpiDisplay` in `DpiSettings.tsx`). Settings has no reorder arrows for monitors or speakers. Not shown in the main popup.

- macOS and Windows show exactly the OS-offered scales (`DpiDisplay.continuous = false`); the user band is ignored.
- Linux reports `continuous = true`; `dpiOptionsFor` builds `dpiMinPercent..=dpiMaxPercent` at `dpiStepPercent` (defaults 60/250/5; caps 50–500, step 1–100 via `Preferences::sanitize()`). `set_display_dpi` enforces the band on Linux only. The band inputs render `display: none` — edit `preferences.json`.
- After an apply, on `devicePixelRatio` change, and on `WindowEvent::ScaleFactorChanged`, the popup refits and `reanchor_popup` (`tray.rs`) re-reads the live tray rect and re-places it.

- **macOS**: display-mode switch; scale % = native pixel width / "looks like" width (`CGConfigureDisplayWithDisplayMode`, permanent). Only real modes exist, so arbitrary steps / sub-100% usually unavailable. Built-in Retina defaults read as 200%.
- **Windows**: undocumented `DisplayConfigGetDeviceInfo`/`SetDeviceInfo` packet types -3/-4 over steps 100–500%.
- **Linux**: X11 only, `xrandr --output <name> --scale`; Wayland returns an error.

Commands: `get_dpi_displays`, `set_display_dpi { id, percent }`, `reanchor_popup_window`.
