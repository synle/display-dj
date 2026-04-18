# Display DJ v5

## Project Overview

Cross-platform desktop system tray application for controlling monitor brightness, contrast, dark mode, volume, keep-awake (sleep prevention), and **window tiling** (macOS + Windows + Linux/X11). Built with **Tauri v2** (Rust backend) + **React 18** (TypeScript frontend) + **Vite 6**.

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

## Versioning

The **single source of truth** for the app version is `src-tauri/tauri.conf.json` → `"version"`. This controls:

1. **UI header**: `build.rs` reads `tauri.conf.json` and sets the compile-time env var `APP_VERSION`. For dev/local builds, the version includes `[beta - <short_sha>]` (e.g. `5.6.0 [beta - abc1234]`). Release builds (CI with `TAURI_RELEASE=true`) show the clean version only. The Tauri command `get_app_version()` (`config.rs`) returns it. The frontend `Header.tsx` displays it as "Display DJ v{version}".
2. **Installer/bundle metadata**: Tauri uses this version for `.dmg`, `.exe`, `.deb`, `.AppImage` bundles (shown in macOS "Get Info", Windows "Properties", etc.).

Other version fields:

- `package.json` → `"version"`: Set to `0.0.0`. Not used by the app (not published to npm).
- `Cargo.toml` → `version`: Set to `0.0.0`. Not used (the crate is not published).
- Release versioning is driven by git tags (`v*` triggers `release.yml`).

## Testing

```bash
npm test             # Run all frontend tests (Vitest)
npm run test:watch   # Run frontend tests in watch mode
cd src-tauri && cargo test  # Run all Rust backend tests
```

### Frontend Tests (Vitest + React Testing Library)

- **Setup**: `src/test/setup.ts` — Configures jsdom, jest-dom matchers, and Tauri API mocks
- **Unit tests**: `src/components/*.test.tsx` — Tests for each component (Header, Slider, DarkModeToggle, VolumeControl, AllMonitorsControl, MonitorControl, KeepAwakeToggle)
- **Smoke test**: `src/App.test.tsx` — Verifies App renders without errors, fetches initial data, handles backend failures gracefully
- Tauri `invoke()` and `listen()` are mocked globally in the test setup

### Backend Tests (Rust)

- **Unit tests**: Inline `#[cfg(test)]` modules in `config.rs`, `display.rs`, `keep_awake.rs`, `tray_icon.rs`, and `tiling.rs`
  - `config.rs`: Serialization/deserialization, defaults, camelCase conventions, file roundtrips, CommandValue enum variants, MonitorMetadata serde, effective min brightness, backward-compatible deserialization of old configs, preferences with monitorConfigs roundtrip, expose_columns/expose_rows defaults and roundtrip, legacy expose_max_windows migration
  - `display.rs`: `DjDisplay` to `Monitor` conversion (including uid computation), `merge_with_configs` (rename, sort), `reconcile_migrated_configs`, `ensure_metadata_for_monitors`, Monitor serde
  - `keep_awake.rs`: KeepAwake guard creation, Mutex<Option<KeepAwake>> pattern (enable/disable/re-enable)
  - `tray_icon.rs`: Percentage-to-pixel conversion, icon generation for all state combinations (dark/light, keep-awake, muted), filled rect and thick line drawing
  - `tray.rs`: Command URL building for all command types (brightness, contrast, volume — both all-monitors and per-monitor), min brightness clamping, contrast capping, invalid value handling
  - `tiling/mod.rs` (shared types + layout math): TilingLayout parsing, layout calculation for all 17 layouts (halves, thirds, two-thirds, quarters, maximize), gap/padding math, custom ratio support, TilingState creation, `layout_across_displays` multi-display overflow with oversized window handling
  - `tiling/macos.rs` (macOS only): macOS-specific AXUIElement window manipulation, NSScreen display detection, Tile Snap via CGEventTap
  - `tiling/windows.rs` (Windows only): Win32 window manipulation via GetForegroundWindow/SetWindowPos, EnumDisplayMonitors for display detection, EnumWindows for Expose
  - `tiling/linux.rs` (Linux only): X11 availability check, strut-to-work-area math (top/bottom/left/right panels, dual-monitor panel isolation, combined struts), process name resolution
- **Smoke test**: `src-tauri/tests/smoke.rs` — Integration test verifying the crate compiles, links, and public API (AppState, run) is accessible

### CI

GitHub Actions (`build.yml`) runs `npm test` and `cargo test` on all platforms (macOS ARM/Intel, Windows, Linux) for every push and PR. On PRs, a comment is posted with download links for each platform's build artifacts.

## Formatting

After making changes to frontend code (`src/`), config files, or docs, always run `npx prettier --write` on the changed files before considering the task done. The prettier hook in `.claude/settings.json` handles this automatically for Edit/Write tool calls, but if you create or modify files via other means, run prettier manually.

## Required Steps for Every Feature Change

1. **Tests**: Always add tests to cover new code. Frontend components get `*.test.tsx` files; Rust modules get `#[cfg(test)]` unit tests. If a function is hard to test directly (e.g., depends on platform APIs, hardware, or external services), mock the API boundary and test the logic. Run `npm test` and `cd src-tauri && cargo test` to verify all tests pass before finishing.
2. **Formatting**: Always run `npx prettier --write` on all changed frontend files (`src/`, `*.ts`, `*.tsx`, `*.json`, `*.md`, `*.yml`).
3. **Documentation**: Always update `CLAUDE.md`, `README.md` (if it exists), and `CONTRIBUTING.md` to reflect any features added or removed — including new commands, preferences, HTTP routes, UI components, and architecture changes.
4. **Method comments**: Always document every new function, method, and test. Rust uses `///` doc comments; TypeScript/React uses `/** */` JSDoc comments. Every public function, Tauri command, React component, non-trivial helper, and test case must have a comment describing what it does or what it verifies.
5. **CLI sidecar version bumps**: When updating `displayDjCliVersion` in `package.json`, always check the [display-dj-cli changelog and commits](https://github.com/synle/display-dj-cli) for upstream changes (new endpoints, changed response formats, removed features). Update our code to use any new APIs and remove usage of deprecated ones. Document the changes in CLAUDE.md and CONTRIBUTING.md.

## macOS Tray Icon Pitfall (Critical)

On macOS, two patterns in Tauri command handlers break the system tray icon — both left-click and right-click stop working entirely:

1. **Sync Tauri commands that access `AppState`**: Declaring a `#[tauri::command]` as `pub fn` (sync) instead of `pub async fn` causes Tauri to run it on a blocking thread that starves the macOS main-thread run-loop, preventing `on_tray_icon_event` from firing. All Tauri commands that access `State<'_, AppState>` must be `async`.

2. **`write_debug_log()` in frequently-called sync commands**: `write_debug_log()` locks `state.preferences` to check `debug_logging`. Using it in `get_preferences` (sync, called on every frontend render) creates enough mutex contention to starve the run-loop. Use `log::info!` instead in sync commands. `write_debug_log()` is safe in async/infrequent commands like `save_preferences`.

These are documented inline in `config.rs` with WARNING comments.

## Key Conventions

- All Rust structs sent to frontend use `#[serde(rename_all = "camelCase")]`
- Tauri commands are snake_case in Rust, called with snake_case strings from frontend `invoke()`
- Frontend parameter objects use camelCase (Serde handles the conversion)
- The `CommandValue` enum uses `#[serde(untagged)]` to support both `"string"` and `["array"]` in keybindings
- Preferences use `#[serde(default)]` so old config files missing new fields gracefully fall back to defaults
- Brightness values are clamped to `effective_min_brightness()` which enforces an absolute floor of 5
- Contrast is DDC-only (`Option<u32>` / `number | null`): built-in displays return `null`. The contrast slider is hidden by default and toggled via the `showContrast` preference in Settings
- Keep Awake uses the `keepawake` crate (v0.6) to prevent system idle sleep and display sleep. The guard is stored as `Mutex<Option<KeepAwake>>` in `AppState` — creating the guard enables keep-awake, dropping it (setting to `None`) releases the assertion. Works on macOS (IOKit), Windows (SetThreadExecutionState), and Linux (D-Bus). The `set_keep_awake` command is `async` to avoid the tray icon pitfall.
- **Dynamic Tray Icon** (`tray_icon.rs`): The system tray icon is drawn programmatically at 128x128 using percentage-based layout constants (no PNG assets). It reflects three app states: (1) **Dark/light mode** — border color swaps (white on dark menu bar, black on light) with inverse default fill; (2) **Keep-awake** — fill changes to blue (deep blue on dark, sky blue on light); (3) **Muted (volume=0)** — red X drawn over the icon. States are cached in `AppState` (`is_dark_mode`, `is_muted`) and the icon is regenerated via `update_tray_icon()` whenever any state changes. Initial state is fetched from the sidecar on startup via `fetch_initial_tray_state()`.
- **Settings Panel**: Auto-saves preferences on every change (300ms debounce). No Save/Cancel buttons — changes take effect immediately. The `SettingsPanel` component uses `useCallback` + `setTimeout` to debounce `save_preferences` calls and triggers `onPreferencesSaved` after each save to refresh the parent UI.
- **Window Tiling** (macOS + Windows + Linux/X11, `tiling/` module directory): Moves/resizes the focused window into tiled layouts. 19 layouts (halves, thirds, two-thirds including vertical, quarters, maximize) plus restore. Two Exposé modes: **Exposé** (`command/tile/expose`, Shift+Ctrl+E / Ctrl+Up) spreads all on-screen windows into a deterministic alphabetical grid; **App Exposé** (`command/tile/exposeApp`, Shift+Ctrl+A / Ctrl+Down) grids only the frontmost app's windows. Both normalize windows first (unminimize + exit fullscreen) and use fill-first multi-display overflow (fill display 1 up to `exposeColumns * exposeRows`, overflow to display 2, etc.). Windows with minimum size constraints (e.g., Steam, Chrome) that exceed the grid cell dimensions on the current display overflow to subsequent displays where fewer windows mean larger cells; the last display uses an adaptive layout (wider rows) as a catch-all fallback. The shared `layout_across_displays` function in `tiling/mod.rs` handles this overflow logic for all platforms. Each invocation re-lays out all windows (no toggle/restore behavior). Layout is deterministic: sorted alphabetically by app name, then by window_id. The Exposé Grid Size in Settings uses separate Columns and Rows sliders (1-5 each, default 3x3 = 9 per screen), allowing non-square grids like 2x3 or 3x4. Triggered via `command/tile/{layoutName}` commands bound to keyboard shortcuts, or the Tiling submenu in the tray menu. The tiling code is organized as a module directory: `tiling/mod.rs` (shared types, layout math, TilingLayout enum, coordinate calculations), `tiling/macos.rs` (macOS implementation), `tiling/windows.rs` (Windows implementation), `tiling/linux.rs` (Linux/X11 implementation). **macOS implementation:** Uses the Accessibility API (`AXUIElement`) to move/resize windows. Requires Accessibility permission. State tracked per-window by CGWindowID in `AppState.tiling_state`. Uses `_AXUIElementGetWindow` (private API, stable since macOS 10.6) to bridge AXUIElement to CGWindowID. NSScreen visible frames are used for display bounds (accounts for menu bar/dock) — must use `screens[0]` (primary) not `mainScreen` (focused) for coordinate conversion. Tile Snap uses CGEventTap (listen-only) on a background thread to detect window drags near screen edges, shows a translucent NSWindow overlay preview, and tiles on mouse-up after verifying the window actually moved. The `objc` crate (v0.2) is used for NSScreen interop; all other macOS FFI (AX API, CoreFoundation) is raw `extern "C"`. **Windows implementation:** Uses Win32 API (`GetForegroundWindow`, `SetWindowPos`, `EnumDisplayMonitors`, `EnumWindows`) via the `windows` crate (v0.58). No special permissions needed. All 19 layouts + restore + Exposé + App Exposé work. Tile Snap (mouse edge snapping) is not yet implemented on Windows — it remains macOS-only. **Linux/X11 implementation:** Uses `x11rb` crate (pure Rust X11 client) with EWMH window manager hints. Gets the focused window via `_NET_ACTIVE_WINDOW`, moves/resizes via `_NET_MOVERESIZE_WINDOW` client messages, enumerates windows via `_NET_CLIENT_LIST`, and gets display geometry via XRandr. Panel/dock reservations are handled via `_NET_WM_STRUT_PARTIAL` / `_NET_WM_STRUT` with `_NET_WORKAREA` fallback. Window frame decorations are compensated via `_NET_FRAME_EXTENTS`. No special permissions needed on X11. Tiling is runtime-gated on Linux: `get_tiling_supported` checks `$DISPLAY` env var and returns false on Wayland-only sessions. Tile Snap is not yet implemented on Linux. Tested on Linux Mint with XFCE (xfwm4). Preferences: `tiling.enabled`, `tiling.halfRatio` (default 50), `tiling.thirdRatio` (default 33), `tiling.gap` (default 0), `tiling.sideEdgeTrigger` (default 10, px for left/right/bottom snap zones), `tiling.topEdgeTrigger` (default 10, px for top/maximize snap zone), `tiling.cornerTrigger` (default 50, px for corner quarter snap zones), `tiling.exposeEnabled` (default true, master toggle for Exposé features), `tiling.exposeColumns` (default 3, grid columns per display), `tiling.exposeRows` (default 3, grid rows per display). **Exposé has its own top-level tray submenu** (separate from Tiling) with Enable/Disable toggle, Exposé/App Exposé actions, and grid size presets (2x2, 2x3, 3x3, 3x4, 4x4, 5x5). The Exposé submenu is only visible when tiling is supported on the platform (same gate as Tiling). The Settings panel has a separate "Enable Exposé" checkbox; the grid size sliders (Columns/Rows) are only visible when Exposé is enabled. Tiling is platform-gated: on macOS, Windows, and Linux (X11) the tray submenu, Settings toggle, and Tauri commands (`get_tiling_supported`, `get_accessibility_trusted`) are active. On Wayland-only Linux sessions, `get_tiling_supported` returns false at runtime. The Settings panel shows an accessibility permission warning on macOS when tiling is enabled but permission is not granted (Windows and Linux do not require special permissions).

## Command Reference

Commands are strings dispatched by `execute_command()` in `tray.rs`. They can be bound to keyboard shortcuts in `keyBindings`, used in profiles, or triggered from the tray menu. The `build_command_url()` helper maps commands to sidecar HTTP URLs and is covered by unit tests.

| Command | Sidecar Endpoint | Description |
|---------|-----------------|-------------|
| `command/changeBrightness/{value}` | `/set_all/{value}` | Set brightness on all monitors |
| `command/changeBrightness/{monitor_id}/{value}` | `/set_one/{id}/{value}` | Set brightness on a single monitor |
| `command/changeContrast/{value}` | `/set_contrast_all/{value}` | Set contrast on all monitors |
| `command/changeContrast/{monitor_id}/{value}` | `/set_contrast_one/{id}/{value}` | Set contrast on a single monitor |
| `command/changeVolume/{value}` | `/set_volume/{value}` | Set volume (0-100) |
| `command/changeDarkMode/{dark\|light\|toggle}` | `/dark`, `/light`, `/theme` | Toggle or set dark/light mode |
| `command/changeProfile/{index}` | (executes profile commands) | Run all commands in a saved profile |
| `command/tile/{layoutName}` | (calls tiling module) | Tile the focused window |

Monitor IDs are the sidecar API IDs (e.g. `"1"`, `"2"`, `"builtin"`). Brightness is clamped to `[effective_min_brightness, 100]`; contrast is clamped to `[0, 100]`.

## Related Projects

- **[display-dj-cli](https://github.com/synle/display-dj-cli)** — The Rust CLI/HTTP server that handles all display operations (brightness, contrast, dark mode). Bundled as a Tauri sidecar. Source at `/Users/syle/Downloads/display-dj-cli`. When bumping the sidecar version, always review upstream changes in that repo.

## Dependencies

The display-dj CLI sidecar handles all platform-specific display and volume dependencies internally. No external tools need to be installed for display or volume control. The `keepawake` crate handles sleep prevention natively on all platforms (macOS IOKit, Windows SetThreadExecutionState, Linux D-Bus). The `tauri-plugin-dialog` crate provides native OS confirmation dialogs (used by Reset to Default). The `windows` crate (v0.58) is a Windows-only dependency used for Win32 window tiling APIs (`GetForegroundWindow`, `SetWindowPos`, `EnumDisplayMonitors`, `EnumWindows`). The `x11rb` crate (v0.13) is a Linux-only dependency (pure Rust X11 client) used for X11/EWMH window tiling (`_NET_ACTIVE_WINDOW`, `_NET_MOVERESIZE_WINDOW`, `_NET_CLIENT_LIST`, XRandr).

### Stale Sidecar Cleanup

On startup, `kill_stale_sidecars()` kills any leftover `display-dj-server` processes from a previous run that didn't exit cleanly (crash, force-quit, installer update). Uses `pkill` on macOS/Linux and `taskkill` on Windows.

### Sidecar binaries

Pre-built sidecar binaries for all 6 platforms are committed in `src-tauri/binaries/`. The build script (`src-tauri/build.rs`) skips the download if the binary already exists and is non-empty (to avoid triggering infinite rebuild loops in `tauri dev`), otherwise tries to download the latest from GitHub releases, then falls back to the committed binary if the download fails (offline, timeout, etc.).

The sidecar version is defined in `package.json` under `displayDjCliVersion`. The `DISPLAY_DJ_CLI_VERSION` env var can override it (used by CI `workflow_dispatch`).

To update the committed binaries after a version bump, run from the project root:

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

### CI

- **`build.yml`**: Runs tests and builds on all platforms for every push and PR. On PRs, posts a comment with artifact download links.
- **`release.yml`**: Triggered by `v*` tags or manual `workflow_dispatch`. Deletes any existing release/tag first (`--cleanup-tag`), then builds all platforms (dmg, nsis, deb, appimage only — no tar.gz/msi/rpm). Release notes are auto-generated from commit history (top 10 commits since last tag, with full diff link). Custom notes can be prepended via the `release_notes` workflow input. Sets `TAURI_RELEASE=true` so builds show clean version without `[beta]` suffix.

## GitHub Raw File URLs

When fetching raw file content from GitHub repos, always use the `?raw=1` blob URL format:

```
https://github.com/{owner}/{repo}/blob/head/{path}?raw=1
```

Do NOT use:

- `https://api.github.com/repos/{owner}/{repo}/contents/{path}` (GitHub Contents API)
- `https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}`

### Linux (additional)

```bash
sudo apt install ddcutil brightnessctl i2c-tools
sudo modprobe i2c-dev
sudo usermod -aG i2c $USER
```
