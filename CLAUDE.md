# Display DJ v7

## Project Overview

Cross-platform desktop system tray application for controlling monitor brightness, contrast, dark mode, volume, keep-awake (sleep prevention), and **window tiling** (macOS + Windows + Linux/X11). Built with **Tauri v2** (Rust backend) + **React 19** (TypeScript frontend) + **Vite 6**.

All platform code (DDC/CI, gamma, WMI, DisplayServices, dark mode, volume, wallpaper, slideshow) is **vendored in-process** under `src-tauri/src/core/`. There is no sidecar process and no runtime dependency on the display-dj-cli repo — Tauri commands call `core::*` functions directly. See [`VENDORING.md`](VENDORING.md) for the upstream→vendored file map and `./scripts/check-vendor-drift.sh` for drift detection.

For full architecture details, request lifecycle, layer-by-layer breakdown, data flow diagrams, and "where to edit" reference, see **[DEV.md](DEV.md)**.

## Architecture

The Rust backend is split into two layers:

- **`src-tauri/src/core/`** — pure platform implementations, no Tauri types. Modules:
  - `core::mod` — shared types: `DisplayInfo`, `DisplayControl` trait, `Platform` trait. `core::PlatformImpl` is a `cfg`-gated type alias that resolves to the right platform impl at compile time.
  - `core::macos`, `core::windows`, `core::linux` — per-OS display implementations (DDC/CI, gamma, DisplayServices, WMI, brightnessctl/ddcutil).
  - `core::theme` — system dark mode read/write.
  - `core::volume` — system volume get/set.
  - `core::wallpaper` — wallpaper set + slideshow timer/state/cycling (all in-process).
  - `core::display` — high-level helpers (`set_all_brightness`, `set_one_brightness`, contrast variants) that fan out to `PlatformImpl`.
- **`src-tauri/src/{display,dark_mode,volume,wallpaper}.rs`** — thin Tauri-command wrappers around `core::*`. CPU-bound work is wrapped in `tauri::async_runtime::spawn_blocking` because every Tauri command taking `State<'_, AppState>` must be `async fn` (see "macOS Tray Icon Pitfall" below).

## Build Commands

```bash
npm install          # Install frontend dependencies
npm run dev          # Start Vite dev server (frontend only)
npm run build        # Build frontend (tsc + vite build)
npx tauri dev        # Run full app in development mode
npx tauri build      # Production build (binary + .dmg/.exe/.deb/.AppImage)
cargo check          # Check Rust compilation (from src-tauri/)
```

**VSCode debugging:** `.vscode/launch.json` provides launch configs for Tauri dev, Vite dev, Vitest (run/watch), Cargo test, Cargo check, and Tauri build. `.vscode/tasks.json` defines the `ui:dev` background task referenced by the lldb-based Tauri Dev launch entry.

## Local Install from Release

Use the `/install-app` slash command to download and install the latest release for the current platform. It handles all platform-specific steps automatically.

### macOS post-install steps (required)

```bash
xattr -cr "/Applications/Display DJ.app"                       # Strip Apple quarantine (unsigned builds)
tccutil reset Accessibility com.synle.display-dj               # Reset Accessibility (required after each new build for tiling)
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
open "/Applications/Display DJ.app"
```

### Windows post-install steps

Run the `*_x64-setup.exe` installer — it handles everything.

## Versioning

The **single source of truth** is `src-tauri/tauri.conf.json` → `"version"`. This drives:

1. **UI header**: `build.rs` reads `tauri.conf.json` and sets compile-time env vars `APP_VERSION` and `BUILD_DATE` (ISO 8601, computed via `SystemTime` + civil date math). Dev/local builds append `[beta - <short_sha>]`; release builds (CI with `TAURI_RELEASE=true`) show clean version. Tauri commands `get_app_version()` and `get_about_info()` (in `config.rs`) expose this; `Header.tsx` renders it.
2. **Installer/bundle metadata**: Tauri uses this version for `.dmg`, `.exe`, `.deb`, `.AppImage` bundles.

Other version fields (`package.json`, `Cargo.toml`) are pinned at `0.0.0` and unused — the npm package and crate are not published. Release versioning is driven by git tags (`v*` triggers `release-official.yml`).

`build.rs` is `tauri_build::build()` plus the `expose_app_version()` helper that emits `APP_VERSION` and `BUILD_DATE` env vars.

## Testing

```bash
npm test                     # Frontend tests (Vitest)
npm run test:watch           # Frontend tests in watch mode
cd src-tauri && cargo test   # Backend tests (Rust)
```

### Frontend Tests (Vitest + React Testing Library)

- **Setup**: `src/test/setup.ts` — jsdom, jest-dom matchers, Tauri API mocks (`invoke()` and `listen()` mocked globally).
- **Unit tests**: `src/components/*.test.tsx` — one per component (Header, Slider, DarkModeToggle, VolumeControl, AllMonitorsControl, MonitorControl, KeepAwakeToggle).
- **Smoke test**: `src/App.test.tsx` — verifies App renders, fetches initial data, handles backend failures.

### Backend Tests (Rust)

Inline `#[cfg(test)]` modules cover:

- `config.rs`: serde roundtrips, defaults, camelCase, `CommandValue` enum, `MonitorMetadata`, effective min brightness, backward-compatible deserialization, layout presets, night-mode schedules, `WallpaperPreferences`.
- `display.rs`: `DjDisplay` → `Monitor` conversion (incl. uid). `DjDisplay` is the local conversion struct in `display.rs`; its fields are populated from `core::DisplayInfo`. Also: `merge_with_configs`, `reconcile_migrated_configs`, `ensure_metadata_for_monitors`, `resolve_monitor` (id/uid/substring).
- `keep_awake.rs`: `KeepAwake` guard creation, `Mutex<Option<KeepAwake>>` enable/disable cycle.
- `tray_icon.rs`: %-to-px conversion, icon generation across all state combos (dark/light × keep-awake × muted), filled rect/thick line drawing.
- `tray.rs`: `build_command_url()` always returns `None` (every command dispatches in-process to `core::*`); brightness clamping; contrast capping.
- `tiling/mod.rs` (shared): `TilingLayout` parsing, all 17 layouts, gap/padding math, `layout_across_displays` overflow + min cell size + DPI scaling, layout preset resolution + rule matching, `plan_expose` / `plan_expose_app` / `plan_layout_preset`, smart-restore helpers (`is_rect_oversized`, `calculate_smart_restore_rect`, `calculate_smart_restore_rect_at_cursor`), grid-aligned oversized placement (`find_free_cell`, `find_free_block`, `mark_block`), `parse_zorder_command` (all 6 variants), `is_window_at_front` pure helper.
- `tiling/macos.rs` (macOS only): `is_window_move`, `build_snap_zones` / `detect_snap_zone_macos`, `get_display_full_frames`, `is_pseudo_fullscreen` / `send_escape_key`, `get_all_gui_app_pids`, `move_all_windows_to_current_space`.
- `tiling/windows.rs` (Windows only): `should_skip_system_window`, DPI border correction, `dbg_log`, expose debounce.
- `tiling/linux.rs` (Linux only): X11 availability check, strut-to-work-area math, process name resolution.
- `wallpaper.rs`: image validation, MD5 path hashing, content hash comparison, fit/slideshow arg parsing, `WallpaperPreferences` serde, remote pack URL/folder validation.

**Smoke test**: `src-tauri/tests/smoke.rs` verifies the crate compiles, links, and exposes `AppState` and `run`.

### CI

GitHub Actions (`build.yml`) runs `npm test` and `cargo test` on macOS ARM/Intel, Windows, and Linux for every push and PR. PRs get a comment with per-platform build artifact links. A single rolled-up check named **`Main Build`** (job id `main_build`) aggregates the 4-platform matrix + coverage into one green/red status on the commit — require this name in branch protection instead of every matrix permutation.

## Formatting

After modifying frontend code (`src/`), config, or docs, always run `npx prettier --write` on the changed files. The prettier hook in `.claude/settings.json` automates this for Edit/Write tool calls — run it manually for non-Tool changes.

## Required Steps for Every Feature Change

1. **Tests**: Frontend → `*.test.tsx`, Rust → `#[cfg(test)]` modules. Mock at the API boundary if hardware/platform/external. Run `npm test` and `cd src-tauri && cargo test` before finishing.
2. **Formatting**: `npx prettier --write` on changed `src/`, `*.ts`, `*.tsx`, `*.json`, `*.md`, `*.yml`.
3. **Documentation**: Update `CLAUDE.md`, `README.md`, and `CONTRIBUTING.md` for new commands, preferences, UI components, architecture changes.
4. **Method comments**: `///` for Rust, `/** */` JSDoc for TS/React. Every public function, Tauri command, React component, non-trivial helper, and test must have one.

## Windows Console-Flash Pitfall (Critical, Windows-only)

**Every Windows child spawn from a `#[cfg(target_os = "windows")]` code path
must go through `core::win_cmd::hidden_command(...)`.** That helper returns a
`std::process::Command` with the Win32 `CREATE_NO_WINDOW` (`0x08000000`)
creation flag pre-applied, so the short-lived `powershell` / `reg` child does
not allocate and immediately tear down a console window of its own — which
otherwise reads as a visible black flash on every brightness change, volume
change, theme toggle, and wallpaper write (the GUI parent has
`windows_subsystem = "windows"` and therefore no console to share).

- **Where this matters**: `core/{windows,volume,theme,wallpaper}.rs`. Every
  `Command::new("powershell")` and `Command::new("reg")` call in those files
  is routed through `hidden_command(...)`. A regression test in `lib.rs`
  (`no_bare_powershell_or_reg_spawns_in_core`) fails the build if a bare spawn
  drifts back in. See the v7.0.9 fix.
- **Where this does NOT matter**: macOS (osascript) and Linux (gsettings /
  feh / xfconf-query / plasma-apply-colorscheme / xrandr / etc.) — neither OS
  has a "Win32 PE subsystem" concept, and neither auto-allocates a terminal
  for a child process. A GUI parent launched from `.desktop` / Finder has no
  controlling terminal; child stdio inherits null fds and no window appears.
  `CREATE_NO_WINDOW` and `windows_subsystem` are Win32-only abstractions.
- **The `windows_subsystem` attribute itself**: must live on the binary root
  (`src-tauri/src/main.rs`), never on `lib.rs`. The inner attribute is
  silently ignored on `lib.rs` and the release `.exe` then ships as a
  console-subsystem program — pops a console for the _parent_ process, which
  `CREATE_NO_WINDOW` on children cannot fix. (`main.rs:1` has it correctly.)
  This burned `sqlui-native` at v3.1.9 — same trap, same fix.
- **Cross-apply to upstream**: `core/*` is vendored from `display-dj-cli`,
  which is itself a CLI (console app) and does not need any of this. The
  `hidden_command(...)` substitution is a display-dj-local patch. On the next
  vendor refresh, re-apply it (or run the substitution and let the test
  enforce the rule).

## macOS Tray Icon Pitfall (Critical)

Two patterns in Tauri command handlers break the system tray icon (left- and right-click stop working):

1. **Sync Tauri commands accessing `AppState`**: `pub fn` (sync) handlers run on a blocking thread that starves the macOS main-thread run-loop, preventing `on_tray_icon_event` from firing. All commands that take `State<'_, AppState>` must be `async`.
2. **`write_debug_log()` in frequent sync commands**: it locks `state.preferences`. Calling it from `get_preferences` (invoked on every render) creates enough mutex contention to starve the run-loop. Use `log::info!` in sync commands; `write_debug_log()` is safe in async/infrequent commands.

Inline WARNING comments in `config.rs` document this.

## Tile Snap Event Monitoring (NSEvent Global Monitor)

Tile Snap uses `NSEvent.addGlobalMonitorForEvents(matching:handler:)` to observe mouse events globally. This replaced `CGEventTap`, which silently failed in production `.app` bundles (macOS Sequoia rejects `CGEventTapEnable` for ad-hoc signed bundles).

**Why NSEvent:**

- Higher-level Cocoa API; macOS trusts it from `.app` bundles without special signing.
- Listen-only, which is all Tile Snap needs.
- Handler runs on the main thread automatically.

**Handler rules (main thread):**

1. Keep it fast — defer AX API calls (`get_focused_window`, `get_window_rect`) until the 10px drag threshold is crossed.
2. Use `try_lock()`, never `lock()`.
3. Wrap with `catch_unwind` — Rust panics cannot unwind through ObjC blocks (would abort).
4. Use raw `objc_msgSend` with `Sel::register("type")` for `[event type]` — `type` is a Rust keyword; `msg_send![event, r#type]` raises an ObjC exception.
5. Convert `[NSEvent mouseLocation]` (Cocoa: Y up from bottom-left) → CG coords (Y down from top-left): `primary_h - cocoa_y`.
6. Use the `block` crate (v0.1) for ObjC blocks. The block must stay alive (`.copy()` to heap) for the monitor's lifetime.

## Soft-Overlay Brightness Fallback (v7.0.19+)

Some panels — most prominently the Samsung Smart Monitor M7/M8 family over USB-C on Intel Iris Xe — ignore DDC/CI writes and have their `SetDeviceGammaRamp` calls silently rejected by the Intel iGPU driver. There is no hardware path that can dim them. The industry workaround (Twinkle Tray, Lunar, Win10_BrightnessSlider) is a software overlay: a transparent, always-on-top, click-through window per monitor whose opacity rises as brightness falls. The OS compositor blends it with everything underneath, so it works on any GPU/driver.

### Per-monitor `brightnessMode` preference

Stored on `MonitorMetadata` in `preferences.monitor_configs`. Four values:

- **`"auto"`** (default) — try DDC, then gamma, then fall back to the overlay if both hardware paths failed.
- **`"ddc"`** — DDC/CI only, no overlay fallback.
- **`"gamma"`** — `SetDeviceGammaRamp` only, no overlay fallback.
- **`"overlay"`** — skip hardware entirely; dim with the overlay window. The only mode that works on the failing-panel scenario above.

The dropdown is rendered next to the existing "Hide" button in Settings (`SettingsPanel.tsx`); built-in displays don't get the dropdown (they dim natively via DisplayServices / WMI / sysfs backlight).

### Overlay module (`src-tauri/src/overlay.rs`)

One Tauri `WebviewWindow` per external monitor, labeled `overlay-{monitor_id}`. Created lazily on the first overlay request, positioned to the monitor's `DisplayInfo.monitor_rect`, made click-through with `set_ignore_cursor_events(true)`. The content is `public/overlay.html` — a single full-viewport black div listening for `set-overlay-alpha` events. Alpha is `1.0 - brightness/100`, clamped to `[0.0, 0.9]` so the user can never make the panel fully opaque (which would prevent recovery via the slider).

Public API:

- `overlay::set_overlay_brightness(app, monitor_id, monitor_rect, brightness_pct)` — ensure window, show, emit alpha. Hides instead of showing when `brightness_pct >= 100`.
- `overlay::destroy_overlay(app, monitor_id)` — close the overlay (called on unplug or when switching back to a hardware-only mode).

### Routing (`display::set_brightness` / `display::set_all_brightness`)

Each Tauri brightness command snapshots `min_brightness`, the per-monitor `brightnessMode`, and the cached `monitor_rect` _before_ awaiting (so the preferences mutex isn't held across `.await` — see CLAUDE.md macOS Tray Icon Pitfall). It then dispatches through `display::route_for_mode(...)`:

- `BrightnessRoute::DdcOnly` → `core::display::set_one_brightness(id, value, "ddc")`; `destroy_overlay` first.
- `BrightnessRoute::GammaOnly` → `core::display::set_one_brightness(id, value, "gamma")`; `destroy_overlay` first.
- `BrightnessRoute::OverlayOnly` → `overlay::set_overlay_brightness(...)`; no hardware call.
- `BrightnessRoute::AutoWithOverlayFallback` → `core::display::set_one_brightness(id, value, "force")`; on success `destroy_overlay`; on failure `set_overlay_brightness`.

`set_all_brightness` keeps a fast path for the common "every monitor in auto" case (single bulk `set_all_brightness("force")` call + per-monitor overlay touch-up). When any monitor has a non-auto mode it iterates per monitor.

The same routing is used by `tray::execute_command` for the `command/changeBrightness/{value}` and `command/changeBrightness/{monitor_id}/{value}` keyboard-shortcut paths via `dispatch_brightness_for_one` / `dispatch_brightness_for_all` helpers.

### Platform status

- **Windows**: fully functional. `DisplayInfo.monitor_rect` is populated from `MONITORINFOEXW.rcMonitor` for external displays.
- **macOS / Linux**: the Tauri overlay window itself spawns, but `monitor_rect` is currently `None` on both platforms (see TODOs in `core::macos` and `core::linux`). `brightnessMode = "overlay"` is selectable in the UI but no-ops on those platforms until the rect is filled in. The auto path keeps working on macOS/Linux because DDC and DisplayServices/ddcutil paths are unaffected.

## Key Conventions

- All Rust structs sent to frontend use `#[serde(rename_all = "camelCase")]`.
- Tauri commands are snake_case in Rust, called with snake_case strings from the frontend.
- Frontend parameter objects use camelCase (Serde converts).
- `CommandValue` uses `#[serde(untagged)]` so keybindings accept both `"string"` and `["array"]`.
- Preferences use `#[serde(default)]` for forward-compatible config loading.
- Brightness is clamped to `[effective_min_brightness(), 100]` (absolute floor of 5).
- Contrast is DDC-only (`Option<u32>` / `number | null`); the slider is hidden by default and toggled via `showContrast` in Settings.
- Keep Awake uses the `keepawake` crate (v0.6) — guard stored as `Mutex<Option<KeepAwake>>` in `AppState`. Creating enables; dropping (set to `None`) releases. Works on macOS (IOKit), Windows (`SetThreadExecutionState`), Linux (D-Bus). The `set_keep_awake` command is `async` (tray pitfall).

## Dynamic Tray Icon (`tray_icon.rs`)

Drawn programmatically at 128x128 from percentage-based layout constants (no PNG assets). Reflects three states:

- **Dark/light mode**: border color swaps (white on dark menu bar, black on light) with inverse default fill.
- **Keep-awake**: fill becomes blue (deep blue on dark, sky blue on light).
- **Muted (volume=0)**: red X drawn over the icon.

States are cached on `AppState` (`is_dark_mode`, `is_muted`); `update_tray_icon()` regenerates on change. Initial state is fetched in-process from `core::theme` and `core::volume` at startup via `fetch_initial_tray_state()`.

## Settings & About

- **Settings Panel**: auto-saves preferences (300ms debounce) — no Save/Cancel buttons. `SettingsPanel` uses `useCallback` + `setTimeout` to debounce `save_preferences`, and triggers `onPreferencesSaved` to refresh the parent UI.
- **About Panel** (`AboutPanel.tsx`): tray menu "About Display DJ" → emits `show-about` → frontend shows panel. Displays version (`get_about_info`), latest version (GitHub `releases/latest`), engine, platform+arch, build date (`BUILD_DATE`), homepage. Shows "Up to date" / "Update available" badge. macOS shows `xattr -cr` quarantine and Accessibility commands in selectable code blocks.

## Window Tiling (macOS + Windows + Linux/X11)

Module: `tiling/`. Moves/resizes the focused window into tiled layouts. **19 layouts** (halves, thirds, two-thirds incl. vertical, quarters, maximize) plus restore.

### Architecture

- `tiling/mod.rs` — shared types, layout math, `TilingLayout`, AND orchestration (`plan_expose`, `plan_expose_app`, `plan_layout_preset`) that compute placements without OS calls.
- `tiling/{macos,windows,linux}.rs` — thin wrappers that call the shared plan functions, then apply `Placement { window_id, owner_pid, target }` via platform APIs.

This means layout bugs are fixed once in `mod.rs`, not three times.

### Smart Restore

When a window's saved original rect is oversized (≥ 85% of display in both width and height — e.g., from maximize/fullscreen), restore uses a smart size instead: 60% of the smallest connected display, no smaller than the app's `AXMinimumSize` (macOS), centered on the window's display. Helpers `is_rect_oversized()` and `calculate_smart_restore_rect()` in `tiling/mod.rs` are shared across platforms.

On macOS, Tile Snap also smart-shrinks oversized windows on drag start (≥ 85% threshold) via `calculate_smart_restore_rect_at_cursor()` so the user can see snap zones while dragging.

### Exposé

- **Exposé** (`command/tile/expose`, Shift+Ctrl+E / Ctrl+Up): all on-screen windows into a deterministic alphabetical grid.
- **App Exposé** (`command/tile/exposeApp`, Shift+Ctrl+A / Ctrl+Down): only frontmost app's windows.

Both **normalize** first (unminimize + exit native fullscreen + Escape browser/video pseudo-fullscreen + collapse virtual desktops/Spaces). **Fill-first overflow**: fill display 1 to `exposeColumns × exposeRows`, overflow to display 2, etc. Windows with min sizes (Steam, Chrome) that exceed grid cells overflow to subsequent displays; the last display uses grid-aligned placement (oversized windows consume `ceil`'d cells, snapped to grid boundaries with no gaps). Resizable windows placed first, oversized fill remaining slots. Layout is deterministic (sorted alphabetically by app, then `window_id`); each invocation re-lays out (no toggle/restore).

The shared `layout_across_displays` in `tiling/mod.rs` handles overflow for all platforms.

### Platform Implementations

- **macOS**: AX API (`AXUIElement`) for move/resize. Requires Accessibility. State per CGWindowID in `AppState.tiling_state`. Uses `_AXUIElementGetWindow` (private but stable since 10.6) to bridge AX → CGWindowID. NSScreen visible frames for display bounds — must use `screens[0]` (primary) not `mainScreen` for coord conversion. Tile Snap via `NSEvent.addGlobalMonitorForEvents` (10px move threshold + position-change check before activation; drop-zone overlays via simple rect hit-tests in `build_snap_zones` / `detect_snap_zone_macos` (per display: 4 corner quarters, top-edge maximize, left/right-edge halves, three 1/3 markers on the bottom row at 25%/50%/75%, and two double-width 2/3 markers on the same bottom row at 12.5%/87.5%); on mouse_up, move to pre-calculated target — no re-detection). Crates: `objc` v0.2 + `block` v0.1; AX is raw `extern "C"`.
- **Windows**: Win32 (`GetForegroundWindow`, `SetWindowPos`, `EnumDisplayMonitors`, `EnumWindows`) via `windows` crate v0.58. No special permissions. All 19 layouts + restore + Exposé + App Exposé. Tile Snap not implemented.
- **Linux/X11**: `x11rb` (pure Rust X11 client) + EWMH. Focused window via `_NET_ACTIVE_WINDOW`; move/resize via `_NET_MOVERESIZE_WINDOW`; enumeration via `_NET_CLIENT_LIST`; geometry via XRandr. Panels respected via `_NET_WM_STRUT_PARTIAL` / `_NET_WM_STRUT` with `_NET_WORKAREA` fallback. Frame decoration via `_NET_FRAME_EXTENTS`. Runtime-gated: `get_tiling_supported` checks `$DISPLAY` (false on Wayland-only). Tile Snap not implemented. Tested on Linux Mint with XFCE (xfwm4).

### Preferences

`tiling.{enabled, halfRatio=50, thirdRatio=33, gap=0, sideEdgeTrigger=18, topEdgeTrigger=18, cornerTrigger=50, exposeEnabled=true, exposeColumns=3, exposeRows=3, exposeMinWidth=400, exposeMinHeight=300}`. `exposeMinWidth/Height` are logical pixels — DPI-scaled on Windows.

### UI

- **Tiling submenu** in tray (gated on platform).
- **Exposé submenu** is separate, top-level — Enable/Disable, Exposé/App Exposé, grid presets (2x2, 2x3, 3x3, 3x4, 4x4, 5x5).
- Settings: separate "Enable Exposé" checkbox; grid sliders (Cols/Rows) shown only when enabled. Accessibility warning on macOS when tiling is enabled but permission not granted.

## Window Z-Order Control (macOS + Windows + Linux/X11)

Lives in the `tiling/` module — shares focused-window resolution and AX/Win32/X11 plumbing. Two scopes (focused window vs. all windows of focused app) × three actions (front, back, toggle):

- `command/window/moveToFront`, `command/app/moveToFront`
- `command/window/moveToBack`, `command/app/moveToBack`
- `command/window/toggleFrontBack`, `command/app/toggleFrontBack`

**Default keybindings**: `Shift+Ctrl+Super+Left` = `command/app/moveToBack`, `Shift+Ctrl+Super+Right` = `command/app/moveToFront` (mnemonic: Left = back/away, Right = front/toward you; `Super` = Cmd/Win/Super). App-scope rather than window-scope so the visible behavior is symmetric on macOS — see "Back" below for why single-window scope can't visibly lower the active app's only window.

**Self-test (debug aid)**: Set `DISPLAY_DJ_ZORDER_SELFTEST=1` before launching. Five seconds after startup, `tiling::run_zorder_selftest()` runs all 6 z-order commands on whatever window is currently focused, with state snapshots between steps. Logs everything with a `[zorder-selftest]` prefix. Off by default — running on every launch would manipulate the user's focused window.

Parsing centralized in `tiling::parse_zorder_command()`; dispatch in `tiling::execute_zorder()` which forwards to platform impls. Dispatched in-process from `tray.rs::execute_command()` on a background thread, so `build_command_url()` returns `None`.

### Front

- **macOS**: `move_window_to_front` → `activateWithOptions: 2` (`NSApplicationActivateIgnoringOtherApps`) + `AXRaise`. `move_app_to_front` → `activateWithOptions: 3` (`...|NSApplicationActivateAllWindows`) + iterate AXWindows via `get_all_ax_windows_for_pid()` and `AXRaise` each (some macOS versions don't fully respect `NSApplicationActivateAllWindows`); originally focused window raised last so it stays topmost. PID via public `AXUIElementGetPid`.
- **Windows**: `SetWindowPos(HWND_TOP, …, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)` + `BringWindowToTop` + `SetForegroundWindow`. App scope iterates HWNDs (`EnumWindows` filtered by `GetWindowThreadProcessId`) and re-foregrounds the focused window after the raise loop.
- **Linux**: `_NET_ACTIVE_WINDOW` client message via existing `raise_window()`. App scope filters `_NET_CLIENT_LIST` by `_NET_WM_PID`.

### Back

- **macOS**: no public AX API to lower — uses private `CGSOrderWindow(cid, wid, -1, 0)` (CoreGraphics SkyLight, `kCGSOrderBelow`), the standard approach in yabai/Rectangle/AeroSpace. CGS extern declared next to existing `CGSGetActiveSpace`/`CGSMoveWindowsToManagedSpace` in `tiling/macos.rs`. `move_window_to_back` resolves AX → CGWindowID via `_AXUIElementGetWindow`. `move_app_to_back` iterates AXWindows front-first, lowering each — preserves within-app relative order at the bottom of the stack. **Critical: `CGSOrderWindow` alone is invisible when the lowered window's app is the active app** — every window of the active app sits above every window of every inactive app on macOS, regardless of within-app z-order. After lowering, `activate_next_app_excluding_pid()` activates the frontmost window's PID from `get_all_windows()` whose owner differs from the lowered app, dropping the lowered app into the inactive layer. The lowered PID is then stored in the module-local `LAST_BACKED_PID: Mutex<Option<i32>>` so a subsequent `move_window_to_front` / `move_app_to_front` can bring that PID back even though focus has shifted to the app we activated. The remembered PID is consumed (cleared) on the first front call after a back, giving natural back/front-pair undo semantics; once consumed, front falls back to "currently focused window." Windows and Linux do not need this trick (no active-app grouping at the WM level), so the remembered-PID logic is macOS-only.
- **Windows**: `SetWindowPos(HWND_BOTTOM, …, SWP_NOACTIVATE)` (Windows transfers focus automatically). App scope iterates HWNDs front-to-back so each `HWND_BOTTOM` drops one window to the absolute bottom.
- **Linux**: `ConfigureWindow(stack_mode = BELOW)` via `lower_window()` — honored by Mutter/KWin/xfwm4.

### Toggle

Stateless. Each call checks live z-order via platform `is_focused_window_at_front()`:

- **macOS**: `CGWindowListCopyWindowInfo`.
- **Windows**: `EnumWindows` filtered by `IsWindowVisible` + skip-list system windows.
- **Linux**: `_NET_CLIENT_LIST_STACKING` reversed (front-to-back); falls back to `_NET_CLIENT_LIST` if WM doesn't expose stacking.

The shared pure function `tiling::is_window_at_front(focused_id, &front_to_back_z_order)` compares against the first entry — testable on all platforms without a window server. If at front → dispatch to `move_to_back`; else → `move_to_front`. App-scope toggle uses the same check but dispatches to the app-scope variants.

## Wallpaper (`wallpaper.rs`)

Sets desktop wallpaper via:

- `command/wallpaper/change/{path}` — all monitors, default fit
- `command/wallpaper/change/{fit}/{path}` — all monitors, explicit fit
- `command/wallpaper/change_single/{monitor}/{path}` — single monitor, default fit
- `command/wallpaper/change_single/{monitor}/{fit}/{path}` — single monitor, explicit fit

Images are validated (existing path, valid extension, > 1 KB), then copied to `{config_dir}/display-dj/wallpapers/wallpaper-{md5(source_path)}.{ext}`. On repeat calls, content MD5 is compared to skip unnecessary overwrites. The cached copy (not the original) is applied via `core::wallpaper::set_wallpaper(fit, path)` / `core::wallpaper::set_wallpaper_one(index, fit, path)`, which call OS APIs directly (NSWorkspace on macOS, `SystemParametersInfoW` + `IDesktopWallpaper` on Windows, `gsettings`/feh on Linux).

**Fit modes**: `fill` (default), `fit`, `stretch`, `center`, `tile` — all platforms (`core::wallpaper` handles per-OS quirks; per-monitor is macOS + Windows only, Linux falls back to global).

`{monitor}` is resolved via `resolve_monitor()` in `display.rs`: exact `id`, then exact `uid`, then case-insensitive substring on name/original_name.

### Slideshow

Managed in-process by `core::wallpaper` (timer thread, state, cycling). GUI starts/stops via `core::wallpaper::start_slideshow(interval, order, fit, folder)` / `core::wallpaper::stop_slideshow()`. On startup, `resume_slideshow_if_enabled()` resumes if `slideshowEnabled`. Manual `command/wallpaper/change` auto-stops the running slideshow.

**Remote packs**: `command/wallpaper/slideshow_remote/{url}` downloads a `.zip` via `reqwest::blocking::get`, extracts valid images to `wallpapers/remote-{md5(url)}/`, then starts a slideshow there. Only `.zip`; max 500 MB; idempotent (skips download if folder exists with images). Uses the `zip` crate.

### Preferences

`wallpaper.{fit="fill", currentWallpaperPath, perMonitorWallpapers, slideshowEnabled=false, slideshowFolder, slideshowIntervalMinutes=30 (min 5), slideshowOrder=forward|backward|random}`.

Settings UI: Wallpaper Fit dropdown, Enable Slideshow checkbox, folder path, interval (hours + minutes, min 5), order dropdown.

## Command Reference

Commands are strings dispatched by `execute_command()` in `tray.rs`. Bindable to keyboard shortcuts in `keyBindings`, usable in profiles, and selectable from the tray menu. Every command dispatches in-process to a `core::*` function or a tiling/wallpaper helper. `build_command_url()` always returns `None` (covered by unit tests; kept as a typed signal that no command currently maps to an external URL).

| Command                                                   | In-process dispatch                                         | Description                                    |
| --------------------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------- |
| `command/changeBrightness/{value}`                        | `core::display::set_all_brightness(value)`                  | Set brightness on all monitors                 |
| `command/changeBrightness/{monitor_id}/{value}`           | `core::display::set_one_brightness(id, value)`              | Set brightness on a single monitor             |
| `command/changeContrast/{value}`                          | `core::display::set_all_contrast(value)`                    | Set contrast on all monitors                   |
| `command/changeContrast/{monitor_id}/{value}`             | `core::display::set_one_contrast(id, value)`                | Set contrast on a single monitor               |
| `command/changeVolume/{value}`                            | `core::volume::set_volume(value)`                           | Set volume (0-100)                             |
| `command/changeDarkMode/dark`                             | `core::theme::set_dark_mode(true)`                          | Switch to dark mode                            |
| `command/changeDarkMode/light`                            | `core::theme::set_dark_mode(false)`                         | Switch to light mode                           |
| `command/changeDarkMode/toggle`                           | `core::theme::get_dark_mode()` then `set_dark_mode(!cur)`   | Toggle dark/light mode                         |
| `command/changeProfile/{index}`                           | (recursively dispatches profile commands)                   | Run all commands in a saved profile            |
| `command/tile/{layoutName}`                               | `tiling::execute_tile(...)`                                 | Tile the focused window                        |
| `command/layout/{name_or_index}`                          | `tiling::execute_layout_preset(...)`                        | Apply a layout preset by name or 0-based index |
| `command/window/moveToFront`                              | `tiling::execute_zorder(WindowToFront)`                     | Raise the focused window above all others      |
| `command/app/moveToFront`                                 | `tiling::execute_zorder(AppToFront)`                        | Raise all windows of the focused app           |
| `command/window/moveToBack`                               | `tiling::execute_zorder(WindowToBack)`                      | Lower the focused window below all others      |
| `command/app/moveToBack`                                  | `tiling::execute_zorder(AppToBack)`                         | Lower all windows of the focused app           |
| `command/window/toggleFrontBack`                          | `tiling::execute_zorder(WindowToggle)`                      | Toggle focused window between front and back   |
| `command/app/toggleFrontBack`                             | `tiling::execute_zorder(AppToggle)`                         | Toggle the focused app between front and back  |
| `command/wallpaper/change/{path}`                         | `core::wallpaper::set_wallpaper(default_fit, path)`         | Set wallpaper on all monitors (default fit)    |
| `command/wallpaper/change/{fit}/{path}`                   | `core::wallpaper::set_wallpaper(fit, path)`                 | Set wallpaper with explicit fit mode           |
| `command/wallpaper/change_single/{monitor}/{path}`        | `core::wallpaper::set_wallpaper_one(idx, default_fit, p)`   | Set wallpaper on a single monitor              |
| `command/wallpaper/change_single/{monitor}/{fit}/{path}`  | `core::wallpaper::set_wallpaper_one(idx, fit, path)`        | Per-monitor wallpaper with explicit fit        |
| `command/wallpaper/slideshow/{folder_path}`               | `core::wallpaper::start_slideshow(default…, folder)`        | Start slideshow (default interval/order)       |
| `command/wallpaper/slideshow/{interval}/{order}/{folder}` | `core::wallpaper::start_slideshow(interval, order, …)`      | Start slideshow with explicit interval/order   |
| `command/wallpaper/slideshow_stop`                        | `core::wallpaper::stop_slideshow()`                         | Stop the active slideshow                      |
| `command/wallpaper/slideshow_remote/{url_to_zip}`         | download via `reqwest`, then `start_slideshow` on extracted | Download zip, extract images, start slideshow  |

Monitor IDs are the `id` field on `core::DisplayInfo` (e.g. `"1"`, `"2"`, `"builtin"`). Brightness clamped to `[effective_min_brightness, 100]`; contrast clamped to `[0, 100]`. Wallpaper monitor matching uses `resolve_monitor()` (id → uid → substring).

## Other Features

- **Night Mode Schedule** (`NightModeSchedule` in `config.rs`): optional `nightCommands` and `dayCommands` arrays of command strings. When non-empty, replace the default brightness + dark/light behavior and execute via `tray::execute_command()` (allows volume changes, profile activation, per-monitor brightness, etc.). When empty (default), falls back to legacy `nightBrightness` / `dayBrightness` + dark/light. Backward-compatible.
- **Window Layout Presets**: named presets in `preferences.layoutPresets`. Each `LayoutPreset` has a `name` and `rules` array; each `LayoutRule` has `appMatch` (case-insensitive substring), `layout` (camelCase TilingLayout), optional `displayIndex` (0-based). Triggered via `command/layout/{name_or_index}` from keybindings, profiles, or the "Layout Presets" tray submenu (only shown when configured). One window per rule (first match wins); duplicate rules to tile multiple windows of the same app. Configured by editing `preferences.json` (Open App Preferences) or browsing the config dir (Open App Folder).

## Related Projects

- **[display-dj-cli](https://github.com/synle/display-dj-cli)** — the **upstream** of the platform code now vendored at `src-tauri/src/core/`. Local checkout at `/Users/syle/git/display-dj-cli`. Display DJ has **no runtime dependency** on this repo: builds and releases do not download or bundle anything from it. It remains useful as a standalone CLI and as a place to land platform-code changes that should be cross-applied; when fixing display/wallpaper/theme bugs, consider whether the fix belongs in both repos. See [`VENDORING.md`](VENDORING.md) for per-file provenance (upstream path + last-synced SHA) and run `./scripts/check-vendor-drift.sh` to detect upstream drift.

## Dependencies

The vendored `core::*` modules call OS APIs directly. Cargo deps:

- `ddc` v0.2 (top-level) — DDC/CI protocol shared types.
- `ddc-macos`, `libc` (macOS-only) — DDC/CI over IOKit.
- `ddc-winapi`, `winapi` (Windows-only) — DDC/CI over Win32. The existing `windows` crate gained the `Win32_Devices_Display` feature for `IDesktopWallpaper` + display enumeration.
- `keepawake` v0.6 — sleep prevention (macOS IOKit, Windows `SetThreadExecutionState`, Linux D-Bus).
- `tauri-plugin-dialog` — native OS confirmation dialogs (used by Reset to Default).
- `md5` v0.7 — wallpaper filename generation and content comparison.
- `zip` v2 — extracts remote wallpaper packs.
- `reqwest` (blocking) — only used to download user-supplied remote wallpaper-pack zips. Not used for any internal IPC.
- `windows` v0.58 (Windows-only) — Win32 window tiling APIs + display/wallpaper APIs.
- `x11rb` v0.13 (Linux-only) — pure Rust X11/EWMH window tiling.

### Linux (additional system packages)

```bash
sudo apt install ddcutil brightnessctl i2c-tools
sudo modprobe i2c-dev
sudo usermod -aG i2c $USER
```

## CI Workflows

- **`build.yml`** — tests + builds on all platforms for every push/PR. PR comment with artifact download links. A rolled-up `Main Build` check (job `main_build`, `needs: [build, coverage]`, `if: always()`) collapses the 4-platform matrix + coverage into one green/red commit status — use this name in branch protection.
- **`release-official.yml`** — triggered by `v*` tags or manual `workflow_dispatch`. Uses `synle/workflows/actions/release/` shared actions (`begin-release` → Tauri matrix build → `end-release`). `begin-release` resolves the tag, cleans existing release, creates a draft. Build uploads assets. `end-release` generates changelog (top 10 commits since last tag, diff link, platform support from `.github/release-body-static.md`) and finalizes flags. Custom notes via `release_notes` input. Sets `TAURI_RELEASE=true` (clean version). Use `/release-official` for interactive triggering.
- **`release-beta.yml`** — manual `workflow_dispatch` only. Same flow with `mode: beta`. Optional `sha` (defaults to HEAD) and `notes` inputs. Creates a draft prerelease tagged `release-beta-<date>-<sha>`. Does not set `TAURI_RELEASE`, so builds show the `[beta - <sha>]` suffix. Use `/release-beta` for interactive triggering.

## GitHub Raw File URLs

Always use the `?raw=1` blob URL format:

```
https://github.com/{owner}/{repo}/blob/head/{path}?raw=1
```

Do **not** use:

- `https://api.github.com/repos/{owner}/{repo}/contents/{path}` (Contents API)
- `https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}`

## Git / PR Merge Policy

- Always **squash and merge** PRs. Never merge commits or rebase merges. One commit per PR.
- **Always rebase before pushing** (`git pull --rebase` before `git push`).
- You may `git merge origin/main` or `git merge origin/master` locally to sync, but PR merges must be squash.
