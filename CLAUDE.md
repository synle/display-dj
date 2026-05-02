# Display DJ v5

## Project Overview

Cross-platform desktop system tray application for controlling monitor brightness, contrast, dark mode, volume, keep-awake (sleep prevention), and **window tiling** (macOS + Windows + Linux/X11). Built with **Tauri v2** (Rust backend) + **React 19** (TypeScript frontend) + **Vite 6**.

Display, dark mode, and volume operations are delegated to the [display-dj CLI](https://github.com/synle/display-dj-cli), which runs as a bundled HTTP server sidecar. The Tauri backend makes HTTP requests to it.

For full architecture details, request lifecycle, layer-by-layer breakdown, data flow diagrams, and "where to edit" reference, see **[DEV.md](DEV.md)**.

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
- `display.rs`: `DjDisplay` → `Monitor` conversion (incl. uid), `merge_with_configs`, `reconcile_migrated_configs`, `ensure_metadata_for_monitors`, `resolve_monitor` (id/uid/substring).
- `keep_awake.rs`: `KeepAwake` guard creation, `Mutex<Option<KeepAwake>>` enable/disable cycle.
- `tray_icon.rs`: %-to-px conversion, icon generation across all state combos (dark/light × keep-awake × muted), filled rect/thick line drawing.
- `tray.rs`: `build_command_url()` for all command types, brightness clamping, contrast capping, z-order commands return `None` (in-process dispatch).
- `tiling/mod.rs` (shared): `TilingLayout` parsing, all 17 layouts, gap/padding math, `layout_across_displays` overflow + min cell size + DPI scaling, layout preset resolution + rule matching, `plan_expose` / `plan_expose_app` / `plan_layout_preset`, smart-restore helpers (`is_rect_oversized`, `calculate_smart_restore_rect`, `calculate_smart_restore_rect_at_cursor`), grid-aligned oversized placement (`find_free_cell`, `find_free_block`, `mark_block`), `parse_zorder_command` (all 6 variants), `is_window_at_front` pure helper.
- `tiling/macos.rs` (macOS only): `is_window_move`, `build_snap_zones` / `detect_snap_zone_macos`, `get_display_full_frames`, `is_pseudo_fullscreen` / `send_escape_key`, `get_all_gui_app_pids`, `move_all_windows_to_current_space`.
- `tiling/windows.rs` (Windows only): `should_skip_system_window`, DPI border correction, `dbg_log`, expose debounce.
- `tiling/linux.rs` (Linux only): X11 availability check, strut-to-work-area math, process name resolution.
- `wallpaper.rs`: image validation, MD5 path hashing, content hash comparison, fit/slideshow arg parsing, `WallpaperPreferences` serde, remote pack URL/folder validation.

**Smoke test**: `src-tauri/tests/smoke.rs` verifies the crate compiles, links, and exposes `AppState` and `run`.

### CI

GitHub Actions (`build.yml`) runs `npm test` and `cargo test` on macOS ARM/Intel, Windows, and Linux for every push and PR. PRs get a comment with per-platform build artifact links.

## Formatting

After modifying frontend code (`src/`), config, or docs, always run `npx prettier --write` on the changed files. The prettier hook in `.claude/settings.json` automates this for Edit/Write tool calls — run it manually for non-Tool changes.

## Required Steps for Every Feature Change

1. **Tests**: Frontend → `*.test.tsx`, Rust → `#[cfg(test)]` modules. Mock at the API boundary if hardware/platform/external. Run `npm test` and `cd src-tauri && cargo test` before finishing.
2. **Formatting**: `npx prettier --write` on changed `src/`, `*.ts`, `*.tsx`, `*.json`, `*.md`, `*.yml`.
3. **Documentation**: Update `CLAUDE.md`, `README.md`, and `CONTRIBUTING.md` for new commands, preferences, HTTP routes, UI components, architecture changes.
4. **Method comments**: `///` for Rust, `/** */` JSDoc for TS/React. Every public function, Tauri command, React component, non-trivial helper, and test must have one.
5. **CLI sidecar version bumps**: When updating `displayDjCliVersion` in `package.json`, review the [display-dj-cli changelog](https://github.com/synle/display-dj-cli) for upstream changes; update our code, document changes in CLAUDE.md and CONTRIBUTING.md.

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

States are cached on `AppState` (`is_dark_mode`, `is_muted`); `update_tray_icon()` regenerates on change. Initial state is fetched from the sidecar at startup via `fetch_initial_tray_state()`.

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

- **macOS**: AX API (`AXUIElement`) for move/resize. Requires Accessibility. State per CGWindowID in `AppState.tiling_state`. Uses `_AXUIElementGetWindow` (private but stable since 10.6) to bridge AX → CGWindowID. NSScreen visible frames for display bounds — must use `screens[0]` (primary) not `mainScreen` for coord conversion. Tile Snap via `NSEvent.addGlobalMonitorForEvents` (10px move threshold + position-change check before activation; drop-zone overlays via simple rect hit-tests in `build_snap_zones` / `detect_snap_zone_macos`; on mouse_up, move to pre-calculated target — no re-detection). Crates: `objc` v0.2 + `block` v0.1; AX is raw `extern "C"`.
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

**Default keybindings**: `Shift+Ctrl+Super+Left` = window toggle, `Shift+Ctrl+Super+Right` = app toggle (mnemonic: Left = single window, Right = many windows; `Super` = Cmd/Win/Super).

Parsing centralized in `tiling::parse_zorder_command()`; dispatch in `tiling::execute_zorder()` which forwards to platform impls. Dispatched in-process from `tray.rs::execute_command()` on a background thread (no sidecar HTTP), so `build_command_url()` returns `None`.

### Front

- **macOS**: `move_window_to_front` → `activateWithOptions: 2` (`NSApplicationActivateIgnoringOtherApps`) + `AXRaise`. `move_app_to_front` → `activateWithOptions: 3` (`...|NSApplicationActivateAllWindows`) + iterate AXWindows via `get_all_ax_windows_for_pid()` and `AXRaise` each (some macOS versions don't fully respect `NSApplicationActivateAllWindows`); originally focused window raised last so it stays topmost. PID via public `AXUIElementGetPid`.
- **Windows**: `SetWindowPos(HWND_TOP, …, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)` + `BringWindowToTop` + `SetForegroundWindow`. App scope iterates HWNDs (`EnumWindows` filtered by `GetWindowThreadProcessId`) and re-foregrounds the focused window after the raise loop.
- **Linux**: `_NET_ACTIVE_WINDOW` client message via existing `raise_window()`. App scope filters `_NET_CLIENT_LIST` by `_NET_WM_PID`.

### Back

- **macOS**: no public AX API to lower — uses private `CGSOrderWindow(cid, wid, -1, 0)` (CoreGraphics SkyLight, `kCGSOrderBelow`), the standard approach in yabai/Rectangle/AeroSpace. CGS extern declared next to existing `CGSGetActiveSpace`/`CGSMoveWindowsToManagedSpace` in `tiling/macos.rs`. `move_window_to_back` resolves AX → CGWindowID via `_AXUIElementGetWindow`. `move_app_to_back` iterates AXWindows front-first, lowering each — preserves within-app relative order at the bottom of the stack.
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

Images are validated (existing path, valid extension, > 1 KB), then copied to `{config_dir}/display-dj/wallpapers/wallpaper-{md5(source_path)}.{ext}`. On repeat calls, content MD5 is compared to skip unnecessary overwrites. The cached copy (not the original) is set via the sidecar endpoints `/set_wallpaper/{fit}/{path}` and `/set_wallpaper_one/{index}/{fit}/{path}`.

**Fit modes**: `fill` (default), `fit`, `stretch`, `center`, `tile` — all platforms (sidecar handles per-OS quirks; per-monitor is macOS + Windows only, Linux falls back to global).

`{monitor}` is resolved via `resolve_monitor()` in `display.rs`: exact `id`, then exact `uid`, then case-insensitive substring on name/original_name.

### Slideshow

Managed by the sidecar (timer, state, cycling). GUI starts/stops via `/wallpaper_slideshow_start/{interval}/{order}/{fit}/{folder}` and `/wallpaper_slideshow_stop`. On startup, `resume_slideshow_if_enabled()` resumes if `slideshowEnabled`. Manual `command/wallpaper/change` auto-stops the slideshow (sidecar-side).

**Remote packs**: `command/wallpaper/slideshow_remote/{url}` downloads a `.zip`, extracts valid images to `wallpapers/remote-{md5(url)}/`, starts a slideshow there. Only `.zip`; max 500 MB; idempotent (skips download if folder exists with images). Uses the `zip` crate.

See `WALLPAPER_CLI_SPEC.md` for the full CLI API spec.

### Preferences

`wallpaper.{fit="fill", currentWallpaperPath, perMonitorWallpapers, slideshowEnabled=false, slideshowFolder, slideshowIntervalMinutes=30 (min 5), slideshowOrder=forward|backward|random}`.

Settings UI: Wallpaper Fit dropdown, Enable Slideshow checkbox, folder path, interval (hours + minutes, min 5), order dropdown.

## Command Reference

Commands are strings dispatched by `execute_command()` in `tray.rs`. Bindable to keyboard shortcuts in `keyBindings`, usable in profiles, and selectable from the tray menu. `build_command_url()` maps commands to sidecar HTTP URLs (covered by unit tests).

| Command                                                   | Sidecar Endpoint                 | Description                                    |
| --------------------------------------------------------- | -------------------------------- | ---------------------------------------------- |
| `command/changeBrightness/{value}`                        | `/set_all/{value}`               | Set brightness on all monitors                 |
| `command/changeBrightness/{monitor_id}/{value}`           | `/set_one/{id}/{value}`          | Set brightness on a single monitor             |
| `command/changeContrast/{value}`                          | `/set_contrast_all/{value}`      | Set contrast on all monitors                   |
| `command/changeContrast/{monitor_id}/{value}`             | `/set_contrast_one/{id}/{value}` | Set contrast on a single monitor               |
| `command/changeVolume/{value}`                            | `/set_volume/{value}`            | Set volume (0-100)                             |
| `command/changeDarkMode/{dark\|light\|toggle}`            | `/dark`, `/light`, `/theme`      | Toggle or set dark/light mode                  |
| `command/changeProfile/{index}`                           | (executes profile commands)      | Run all commands in a saved profile            |
| `command/tile/{layoutName}`                               | (calls tiling module)            | Tile the focused window                        |
| `command/layout/{name_or_index}`                          | (calls tiling module)            | Apply a layout preset by name or 0-based index |
| `command/window/moveToFront`                              | (calls tiling module)            | Raise the focused window above all others      |
| `command/app/moveToFront`                                 | (calls tiling module)            | Raise all windows of the focused app           |
| `command/window/moveToBack`                               | (calls tiling module)            | Lower the focused window below all others      |
| `command/app/moveToBack`                                  | (calls tiling module)            | Lower all windows of the focused app           |
| `command/window/toggleFrontBack`                          | (calls tiling module)            | Toggle focused window between front and back   |
| `command/app/toggleFrontBack`                             | (calls tiling module)            | Toggle the focused app between front and back  |
| `command/wallpaper/change/{path}`                         | (calls wallpaper module)         | Set wallpaper on all monitors (default fit)    |
| `command/wallpaper/change/{fit}/{path}`                   | (calls wallpaper module)         | Set wallpaper with explicit fit mode           |
| `command/wallpaper/change_single/{monitor}/{path}`        | (calls wallpaper module)         | Set wallpaper on a single monitor              |
| `command/wallpaper/change_single/{monitor}/{fit}/{path}`  | (calls wallpaper module)         | Per-monitor wallpaper with explicit fit        |
| `command/wallpaper/slideshow/{folder_path}`               | (calls wallpaper module)         | Start slideshow (default interval/order)       |
| `command/wallpaper/slideshow/{interval}/{order}/{folder}` | (calls wallpaper module)         | Start slideshow with explicit interval/order   |
| `command/wallpaper/slideshow_stop`                        | (calls wallpaper module)         | Stop the active slideshow                      |
| `command/wallpaper/slideshow_remote/{url_to_zip}`         | (calls wallpaper module)         | Download zip, extract images, start slideshow  |

Monitor IDs are sidecar API IDs (e.g. `"1"`, `"2"`, `"builtin"`). Brightness clamped to `[effective_min_brightness, 100]`; contrast clamped to `[0, 100]`. Wallpaper monitor matching uses `resolve_monitor()` (id → uid → substring).

## Other Features

- **Night Mode Schedule** (`NightModeSchedule` in `config.rs`): optional `nightCommands` and `dayCommands` arrays of command strings. When non-empty, replace the default brightness + dark/light behavior and execute via `tray::execute_command()` (allows volume changes, profile activation, per-monitor brightness, etc.). When empty (default), falls back to legacy `nightBrightness` / `dayBrightness` + dark/light. Backward-compatible.
- **Window Layout Presets**: named presets in `preferences.layoutPresets`. Each `LayoutPreset` has a `name` and `rules` array; each `LayoutRule` has `appMatch` (case-insensitive substring), `layout` (camelCase TilingLayout), optional `displayIndex` (0-based). Triggered via `command/layout/{name_or_index}` from keybindings, profiles, or the "Layout Presets" tray submenu (only shown when configured). One window per rule (first match wins); duplicate rules to tile multiple windows of the same app. Configured by editing `preferences.json` (Open App Preferences) or browsing the config dir (Open App Folder).

## Related Projects

- **[display-dj-cli](https://github.com/synle/display-dj-cli)** — Rust CLI/HTTP server handling all display ops (brightness, contrast, dark mode, wallpaper). Bundled as a Tauri sidecar (currently v2.2.0). Source at `/Users/syle/Downloads/display-dj-cli`. Always review upstream changes when bumping the sidecar version.

## Dependencies

The display-dj CLI sidecar handles all platform-specific display and volume dependencies — no external tools needed for display/volume control. Other crates:

- `keepawake` v0.6 — sleep prevention (macOS IOKit, Windows `SetThreadExecutionState`, Linux D-Bus).
- `tauri-plugin-dialog` — native OS confirmation dialogs (used by Reset to Default).
- `md5` v0.7 — wallpaper filename generation and content comparison.
- `zip` v2 — extracts remote wallpaper packs.
- `windows` v0.58 (Windows-only) — Win32 window tiling APIs.
- `x11rb` v0.13 (Linux-only) — pure Rust X11/EWMH window tiling.

### Linux (additional system packages)

```bash
sudo apt install ddcutil brightnessctl i2c-tools
sudo modprobe i2c-dev
sudo usermod -aG i2c $USER
```

## Single-Instance Enforcement

Display DJ is a **singleton** — only one copy can run at a time. Enforced via `tauri-plugin-single-instance` (registered first in the Tauri builder in `lib.rs::run()`). When a second instance launches, the plugin acquires-or-detects a per-`identifier` (`com.synle.display-dj`) lock; if the lock is held by another running instance, the second process exits immediately and the running instance's callback fires (we just `log::info!` the duplicate launch — there's no main window to refocus, the app is tray-only). This prevents duplicate tray icons and conflicting sidecar processes when the user double-clicks the `.app`, when autostart races a manual launch, or when the app is opened from multiple Finder/Explorer windows. The plugin **must** be the first plugin registered so the lock check happens before any other initialization (sidecar spawn, tray setup, etc.).

## Sidecar Lifecycle

Three layers of shutdown protection:

1. **Parent-death detection (primary)**: sidecar monitors stdin in a background thread. Tauri's shell plugin pipes stdin automatically. When Tauri exits (normal/crash/force-quit), the OS closes the pipe → stdin EOF → sidecar `process::exit(0)`. Fastest and most reliable.
2. **Explicit kill on exit**: `RunEvent::Exit` → `child.kill()` on the stored `CommandChild` (belt-and-suspenders).
3. **Stale process cleanup on startup**: `kill_stale_sidecars()` kills leftover `display-dj-server` processes via `pkill` (macOS/Linux) or `taskkill` (Windows) — catches edge cases where (1) and (2) both failed.

### Sidecar binaries

Pre-built binaries for all 6 platforms are committed in `src-tauri/binaries/`. `src-tauri/build.rs` skips download if a non-empty binary already exists (avoids `tauri dev` rebuild loops); otherwise tries the latest from GitHub releases, then falls back to the committed binary on failure (offline/timeout).

The sidecar version is in `package.json` under `displayDjCliVersion`. Override with `DISPLAY_DJ_CLI_VERSION` (used by CI `workflow_dispatch`).

To refresh committed binaries after a version bump:

```bash
VERSION=$(node -p "require('./package.json').displayDjCliVersion")
cd src-tauri/binaries
curl -fSL "https://github.com/synle/display-dj-cli/releases/download/${VERSION}/display-dj-macos-arm64" -o display-dj-server-aarch64-apple-darwin
curl -fSL "https://github.com/synle/display-dj-cli/releases/download/${VERSION}/display-dj-macos-x64" -o display-dj-server-x86_64-apple-darwin
curl -fSL "https://github.com/synle/display-dj-cli/releases/download/${VERSION}/display-dj-windows-x64.exe" -o display-dj-server-x86_64-pc-windows-msvc.exe
curl -fSL "https://github.com/synle/display-dj-cli/releases/download/${VERSION}/display-dj-windows-arm64.exe" -o display-dj-server-aarch64-pc-windows-msvc.exe
curl -fSL "https://github.com/synle/display-dj-cli/releases/download/${VERSION}/display-dj-linux-x64" -o display-dj-server-x86_64-unknown-linux-gnu
curl -fSL "https://github.com/synle/display-dj-cli/releases/download/${VERSION}/display-dj-linux-arm64" -o display-dj-server-aarch64-unknown-linux-gnu
chmod 755 display-dj-server-*
```

## CI Workflows

- **`build.yml`** — tests + builds on all platforms for every push/PR. PR comment with artifact download links.
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
