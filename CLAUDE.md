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
  - `core::keyboard_backlight` — built-in laptop keyboard backlight (beta). macOS via private IOHIDEventSystem, Windows via Lenovo+Dell WMI. Linux unsupported.
  - `core::wallpaper` — wallpaper set + slideshow timer/state/cycling (all in-process).
  - `core::display` — high-level helpers (`set_all_brightness`, `set_one_brightness`, contrast variants) that fan out to `PlatformImpl`.
- **`src-tauri/src/{display,dark_mode,volume,keyboard_backlight,wallpaper}.rs`** — thin Tauri-command wrappers around `core::*`. CPU-bound work is wrapped in `tauri::async_runtime::spawn_blocking` because every Tauri command taking `State<'_, AppState>` must be `async fn` (see "macOS Tray Icon Pitfall" below).
- **`src-tauri/src/sidecar_cache.rs`** — 5-minute TTL cache wrapping `core::PlatformImpl::enumerate()`, `core::theme::get_dark_mode()`, `core::volume::get_volume()`, and `core::keyboard_backlight::get_keyboard_backlight()`. Writes and Force Refresh invalidate. Name retained from the v6.x HTTP-sidecar era; v7+ is pure in-process.
- **`src-tauri/src/overlay.rs`** — per-monitor Tauri `WebviewWindow` for soft-overlay brightness fallback (see "Soft-Overlay Brightness Fallback" below).

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

Use the `/install-app` slash command — handles platform-specific steps. macOS post-install (unsigned builds): `xattr -cr "/Applications/Display DJ.app"`, then `tccutil reset Accessibility com.synle.display-dj` and grant Accessibility for tiling. Windows: run the `*_x64-setup.exe` installer.

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
- **Unit tests**: `src/components/*.test.tsx` — one per component (Header, Slider, DarkModeToggle, VolumeControl, KeyboardBacklightControl, AllMonitorsControl, MonitorControl, KeepAwakeToggle, AboutPanel, ProfileButtons, SettingsPanel).
- **Smoke test**: `src/App.test.tsx` — verifies App renders, fetches initial data, handles backend failures.

### Backend Tests (Rust)

Inline `#[cfg(test)]` modules cover:

- `config.rs`: serde roundtrips, defaults, camelCase, `CommandValue` enum, `MonitorMetadata`, effective min brightness, backward-compatible deserialization, layout presets, night-mode schedules, `WallpaperPreferences`, `KeyboardBacklightPreferences` + `Shift+F2` combo binding.
- `core::keyboard_backlight`: `snap_to_25` rounds to nearest 25 + clamps at 100; cross-platform `get` / `set` / `is_supported` smoke tests; back-compat default (`enabled = true`).
- `keyboard_backlight.rs` (Tauri commands): cache hit/miss, snap-on-set, supported-bool smoke.
- `display.rs`: `DjDisplay` → `Monitor` conversion (incl. uid). `DjDisplay` is the local conversion struct in `display.rs`; its fields are populated from `core::DisplayInfo`. Also: `merge_with_configs`, `reconcile_migrated_configs`, `ensure_metadata_for_monitors`, `resolve_monitor` (id/uid/substring).
- `keep_awake.rs`: `KeepAwake` guard creation, `Mutex<Option<KeepAwake>>` enable/disable cycle.
- `tray_icon.rs`: %-to-px conversion, icon generation across all state combos (dark/light × keep-awake × muted), filled rect/thick line drawing.
- `tray.rs`: `build_command_url()` always returns `None` (every command dispatches in-process to `core::*`); brightness clamping; contrast capping.
- `tiling/mod.rs` (shared): `TilingLayout` parsing, all 19 layouts, gap/padding math, `layout_across_displays` overflow + min cell size + DPI scaling, layout preset resolution + rule matching, `plan_expose` / `plan_expose_app` / `plan_layout_preset`, smart-restore helpers (`is_rect_oversized`, `calculate_smart_restore_rect`, `calculate_smart_restore_rect_at_cursor`), grid-aligned oversized placement (`find_free_cell`, `find_free_block`, `mark_block`), `parse_zorder_command` (all 6 variants), `is_window_at_front` pure helper.
- `tiling/macos.rs` (macOS only): `is_window_move`, `build_snap_zones` / `detect_snap_zone_macos`, `get_display_full_frames`, `is_pseudo_fullscreen` / `send_escape_key`, `get_all_gui_app_pids`, `move_all_windows_to_current_space`, `ax_error_description`.
- `tiling/windows.rs` (Windows only): `should_skip_system_window`, DPI border correction, `dbg_log`, expose debounce.
- `tiling/linux.rs` (Linux only): X11 availability check, strut-to-work-area math, process name resolution.
- `wallpaper.rs`: image validation, MD5 path hashing, content hash comparison, fit/slideshow arg parsing, `WallpaperPreferences` serde, remote pack URL/folder validation.
- `overlay.rs`: brightness-to-alpha math, label generation, monitor-rect → window-position math.
- `sidecar_cache.rs`: TTL freshness, write/refresh invalidation, per-entry race-free reads.

**Smoke test**: `src-tauri/tests/smoke.rs` verifies the crate compiles, links, and exposes `AppState` and `run`.

### CI

GitHub Actions (`build.yml`) runs `npm test` and `cargo test` on macOS ARM/Intel, Windows, and Linux for every push and PR. PRs get a comment with per-platform build artifact links. A single rolled-up check named **`Main Build`** (job id `main_build`) aggregates the 4-platform matrix + coverage into one green/red status on the commit — require this name in branch protection instead of every matrix permutation.

### Coverage thresholds (where they live)

Coverage floors trail the current main-branch measurement by ~10pp (per CLAUDE.md global rule 40). The actual numbers are NOT mirrored in this doc — read them from the source of truth before changing:

- **Frontend (Vitest, v8 provider)** — thresholds in `vite.config.ts` under `test.coverage.thresholds` (`lines`, `statements`, `branches`, `functions`). Run locally via `npm run test:coverage`; CI step `Frontend coverage (Vitest)` fails the build when any axis dips below its floor. Reports land in `coverage/`.
- **Backend (cargo-llvm-cov)** — thresholds inline on the `Rust coverage (cargo-llvm-cov)` step in `.github/workflows/build.yml` as `--fail-under-lines`, `--fail-under-functions`, `--fail-under-regions` flags. Run locally with `cd src-tauri && cargo llvm-cov --lib --summary-only`. HTML report lands in `src-tauri/target/llvm-cov-target/html/`.

When raising thresholds: measure current %, set floor ~10pp below, update both files, commit. Never lower without leaving the same ~10pp safety gap (the comments above the flags track the history).

## Formatting

After modifying frontend code (`src/`), config, or docs, always run `npx prettier --write` on the changed files. The prettier hook in `.claude/settings.json` automates this for Edit/Write tool calls — run it manually for non-Tool changes.

## Required Steps for Every Feature Change

1. **Tests**: Frontend → `*.test.tsx`, Rust → `#[cfg(test)]` modules. Mock at the API boundary if hardware/platform/external. Run `npm test` and `cd src-tauri && cargo test` before finishing.
2. **Formatting**: `npx prettier --write` on changed `src/`, `*.ts`, `*.tsx`, `*.json`, `*.md`, `*.yml`.
3. **Documentation**: Update `CLAUDE.md`, `README.md`, and `CONTRIBUTING.md` for new commands, preferences, UI components, architecture changes.
4. **Method comments**: `///` for Rust, `/** */` JSDoc for TS/React. Every public function, Tauri command, React component, non-trivial helper, and test must have one.

## Windows Console-Flash Pitfall (Critical, Windows-only)

**Every Windows child spawn from `#[cfg(target_os = "windows")]` must go through `core::win_cmd::hidden_command(...)`.** That helper pre-applies `CREATE_NO_WINDOW` (`0x08000000`) so the short-lived `powershell` / `reg` child doesn't allocate a console (visible black flash on every brightness / volume / theme / wallpaper / keyboard-backlight change — the GUI parent has `windows_subsystem = "windows"` and no console to share).

- **Where this matters**: `core/{windows,volume,theme,wallpaper,keyboard_backlight}.rs`. Regression test `no_bare_powershell_or_reg_spawns_in_core` in `lib.rs` fails the build if a bare spawn drifts back in.
- **Not relevant on macOS / Linux** — no Win32-PE-subsystem concept; GUI parents launched from `.desktop` / Finder inherit null stdio.
- **The `windows_subsystem` attribute** must live on `main.rs`, never `lib.rs` (silently ignored there — release `.exe` then ships as console-subsystem, which `CREATE_NO_WINDOW` on children cannot fix).
- **Vendor refresh**: `core/*` is vendored from `display-dj-cli` (a CLI — doesn't need this). The `hidden_command` substitution is a display-dj-local patch; re-apply on refresh. Regression test enforces it.

## macOS Tray Icon Pitfall (Critical)

Two patterns in Tauri command handlers break the system tray icon (left- and right-click stop working):

1. **Sync Tauri commands accessing `AppState`**: `pub fn` (sync) handlers run on a blocking thread that starves the macOS main-thread run-loop, preventing `on_tray_icon_event` from firing. All commands that take `State<'_, AppState>` must be `async`.
2. **`write_debug_log()` in frequent sync commands**: it locks `state.preferences`. Calling it from `get_preferences` (invoked on every render) creates enough mutex contention to starve the run-loop. Use `log::info!` in sync commands; `write_debug_log()` is safe in async/infrequent commands.

Inline WARNING comments in `config.rs` document this.

## Tile Snap Event Monitoring (NSEvent Global Monitor)

Tile Snap uses `NSEvent.addGlobalMonitorForEvents` (listen-only, main-thread, trusted from ad-hoc-signed `.app` bundles — replaces `CGEventTap`, which silently failed under macOS Sequoia). Handler must: stay fast (defer AX calls until the 10 px drag threshold), use `try_lock()` only, wrap in `catch_unwind` (panics can't unwind through ObjC blocks), call `[event type]` via raw `objc_msgSend` + `Sel::register("type")` (`type` is a Rust keyword), and convert Cocoa Y-up coords to CG Y-down via `primary_h - cocoa_y`. Uses the `block` crate (v0.1); the block must outlive the monitor (`.copy()` to heap).

## Soft-Overlay Brightness Fallback

Some panels (e.g. Samsung Smart Monitor M7/M8 over USB-C on Intel Iris Xe) ignore DDC/CI and reject `SetDeviceGammaRamp`. The industry workaround (Twinkle Tray, Lunar) is a transparent, always-on-top, click-through window per monitor whose opacity rises as brightness falls — the OS compositor blends it with everything underneath, so it works on any GPU/driver.

### Per-monitor `brightnessMode` preference

Stored on `MonitorMetadata.brightnessMode`. Four values: `"auto"` (default — DDC → gamma → overlay), `"ddc"`, `"gamma"`, `"overlay"` (skip hardware, dim via overlay window — only mode that works on USB-C Samsung Smart Monitors on Intel Iris Xe). Dropdown in `SettingsPanel.tsx` next to "Hide"; built-in displays don't get the dropdown (they dim natively via DisplayServices / WMI / sysfs backlight).

### Overlay module (`src-tauri/src/overlay.rs`)

One Tauri `WebviewWindow` per external monitor, labeled `overlay-{monitor_id}`. Lazy-created on first request, positioned via `DisplayInfo.monitor_rect`, click-through (`set_ignore_cursor_events(true)`). Content is `public/overlay.html` — a full-viewport black div listening for `set-overlay-alpha` events. Alpha = `1.0 - brightness/100`, clamped to `[0.0, 0.9]` (so the user can always recover via the slider). API: `overlay::set_overlay_brightness(app, monitor_id, monitor_rect, brightness_pct)` and `overlay::destroy_overlay(app, monitor_id)`.

### Routing

Each Tauri brightness command snapshots `min_brightness`, the per-monitor `brightnessMode`, and the cached `monitor_rect` _before_ awaiting (mutex must not be held across `.await` — see macOS Tray Icon Pitfall). Dispatches through `display::route_for_mode(...)` to `DdcOnly` / `GammaOnly` / `OverlayOnly` / `AutoWithOverlayFallback`. The same routing is reused by `tray::execute_command` via `dispatch_brightness_for_{one,all}` helpers.

### Platform status

- **Windows**: fully functional. `DisplayInfo.monitor_rect` from `MONITORINFOEXW.rcMonitor`.
- **macOS / Linux**: overlay window spawns, but `monitor_rect` is currently `None` (see TODOs). `brightnessMode = "overlay"` is selectable but no-ops on those platforms until the rect is filled in. Auto path unaffected (DDC + DisplayServices / ddcutil still work).

## Key Conventions

- All Rust structs sent to frontend use `#[serde(rename_all = "camelCase")]`.
- Tauri commands are snake_case in Rust, called with snake_case strings from the frontend.
- Frontend parameter objects use camelCase (Serde converts).
- `CommandValue` uses `#[serde(untagged)]` so keybindings accept both `"string"` and `["array"]`.
- Preferences use `#[serde(default)]` for forward-compatible config loading.
- Brightness is clamped to `[effective_min_brightness(), 100]` (absolute floor of 5).
- Contrast is DDC-only (`Option<u32>` / `number | null`); the slider is hidden by default and toggled via `showContrast` in Settings.
- Keep Awake uses the `keepawake` crate (v0.6) — guard stored as `Mutex<Option<KeepAwake>>` in `AppState`. Creating enables; dropping (set to `None`) releases. Works on macOS (IOKit), Windows (`SetThreadExecutionState`), Linux (D-Bus). The `set_keep_awake` command is `async` (tray pitfall).

## Keyboard Backlight (beta)

Built-in laptop keyboard backlight slider rendered directly below the volume slider. Slider is `step=25` so the only reachable values are **0 / 25 / 50 / 75 / 100**; the backend re-snaps via `core::keyboard_backlight::snap_to_25()` so the shortcut command (`command/changeKeyboardBacklight/{value}`) and the slider produce identical hardware state.

- **macOS**: IOKit `IOHIDEventSystemClient` + `KeyboardBacklightBrightness` property on the service with HID page `0x0B` / usage `0x4B`. Symbols loaded via `dlopen("/System/Library/Frameworks/IOKit.framework/IOKit")` so a future SDK rename degrades to `is_supported() = false` instead of failing to link. Built-in keyboards only.
- **Windows**: probes Lenovo WMI (`Lenovo_KeyboardBacklightLevel` / `Lenovo_SetKeyboardBacklightLevel` in `root\wmi`) then Dell WMI (`DellKeyboardBacklight`). Vendor 0/1/2 levels map to 0/50/100 %. All spawns route through `core::win_cmd::hidden_command` (console-flash rule).
- **Linux**: unsupported. Sysfs (`/sys/class/leds/*::kbd_backlight/`) and UPower D-Bus are future work — out of scope for v7.0.26.
- **External keyboards**: not supported. No Razer / Corsair / Logitech SDK integration.

UI hides the slider when **either** the backend reports unsupported **or** `preferences.keyboardBacklight.enabled = false`. Default keybinding: `Shift+F2` is a `CommandValue::Multiple` that runs both `command/changeProfile/2` (Focus) and `command/changeKeyboardBacklight/0` — a single "dim everything" shortcut.

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

- **macOS**: AX API (`AXUIElement`) for move/resize. Requires Accessibility. State per CGWindowID in `AppState.tiling_state`. Uses `_AXUIElementGetWindow` (private but stable since 10.6) to bridge AX → CGWindowID. NSScreen visible frames for display bounds — must use `screens[0]` (primary) not `mainScreen` for coord conversion. Tile Snap via `NSEvent.addGlobalMonitorForEvents` (10px move threshold; drop-zone overlays via `build_snap_zones` / `detect_snap_zone_macos`: per display, 4 corner quarters, top-edge maximize, left/right-edge halves, three 1/3 markers + two 2/3 markers on the bottom row; on mouse_up move to pre-calculated target). Crates: `objc` v0.2 + `block` v0.1; AX is raw `extern "C"`. Chromium apps need a `NSWorkspace.frontmostApplication` fallback — see DEV.md.
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

- **macOS**: no public AX API to lower — uses private `CGSOrderWindow(cid, wid, -1, 0)` (CoreGraphics SkyLight, `kCGSOrderBelow`), the standard approach in yabai/Rectangle/AeroSpace. `move_app_to_back` iterates AXWindows front-first to preserve within-app relative order. **Critical: `CGSOrderWindow` alone is invisible when the lowered window's app is the active app** — every window of the active app sits above every inactive app's windows on macOS. After lowering, `activate_next_app_excluding_pid()` activates a different app's frontmost window, dropping the lowered app into the inactive layer. The lowered PID is stored in `LAST_BACKED_PID: Mutex<Option<i32>>` so a subsequent `move_*_to_front` can bring it back; consumed on first front call after a back (natural undo). macOS-only — Windows/Linux WMs have no active-app grouping.
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
| `command/changeKeyboardBacklight/{value}`                 | `core::keyboard_backlight::set_keyboard_backlight(value)`   | Set keyboard backlight (snapped to nearest 25) |
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

- **[display-dj-cli](https://github.com/synle/display-dj-cli)** — upstream of `src-tauri/src/core/`. No runtime dependency: builds and releases don't pull from it. When fixing display/wallpaper/theme bugs, consider whether the fix belongs in both repos. See [`VENDORING.md`](VENDORING.md) for per-file provenance + last-synced SHA; `./scripts/check-vendor-drift.sh` detects drift.

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

- **`build.yml`** — tests + builds on all platforms for every push/PR. PR comment with artifact download links. Rolled-up `Main Build` check (job `main_build`, `needs: [build, coverage]`, `if: always()`) collapses the 4-platform matrix + coverage into one green/red status — use this name in branch protection.
- **`release-official.yml`** — `v*` tag or manual dispatch. Uses `synle/workflows/actions/release/` shared actions (`begin-release` → Tauri matrix build → `end-release`). Generates changelog from commits since last tag + `.github/release-body-static.md`. Sets `TAURI_RELEASE=true` (clean version). Trigger via `/release-official`.
- **`release-beta.yml`** — manual dispatch only. Same flow with `mode: beta`. Draft prerelease tagged `release-beta-<date>-<sha>`. No `TAURI_RELEASE`, so builds show the `[beta - <sha>]` suffix. Trigger via `/release-beta`.

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
