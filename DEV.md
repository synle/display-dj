# Developer Guide

Full architecture reference for Display DJ v4. Read this before making changes.

---

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Directory Structure](#directory-structure)
- [Request Lifecycle](#request-lifecycle)
- [Layer-by-Layer Breakdown](#layer-by-layer-breakdown)
  - [React Components](#1-react-components-src-components)
  - [Tauri IPC Bridge](#2-tauri-ipc-bridge)
  - [Rust Backend Commands](#3-rust-backend-commands-src-taurisrc)
  - [HTTP Sidecar](#4-http-sidecar-display-dj-cli)
  - [Platform-Specific Code](#5-platform-specific-code-volumers)
- [State Management](#state-management)
- [Data Flow](#data-flow)
- [Key Architectural Rules](#key-architectural-rules)
- [Where to Edit](#where-to-edit)
- [Adding a New Tauri Command](#adding-a-new-tauri-command)
- [Window Positioning (multi-monitor DPI)](#window-positioning-multi-monitor-dpi)
- [Monitor Identity (UID scheme)](#monitor-identity-uid-scheme)
- [Configuration Files](#configuration-files)
- [App Versioning](#app-versioning)
- [display-dj CLI Sidecar](#display-dj-cli-sidecar)
- [Known Limitations](#known-limitations)
- [Troubleshooting](#troubleshooting)

---

## Architecture Overview

Display DJ is a system tray app built with [Tauri v2](https://v2.tauri.app/). A React frontend runs inside a WebView and communicates with a Rust backend over Tauri IPC. The Rust backend delegates all display and dark mode operations to a bundled HTTP server sidecar ([display-dj CLI](https://github.com/synle/display-dj-cli)). Volume is the only feature handled directly in Rust with platform-specific code.

```
┌──────────────────────────────────────────────────────┐
│  React 18 + TypeScript + Vite 6                      │
│  (WebView: WebKit on macOS/Linux, WebView2 on Win)   │
│                                                      │
│  Components:  App.tsx (root state)                   │
│               MonitorControl / AllMonitorsControl     │
│               VolumeControl / DarkModeToggle          │
│               SettingsPanel / ProfileButtons          │
│                                                      │
│  invoke("command_name", { params })   ──────────┐    │
│  listen("event-name", callback)       ◄─────┐   │    │
└──────────────────────────────────────────────┼───┼────┘
                                               │   │
                              Tauri IPC bridge │   │
                                               │   │
┌──────────────────────────────────────────────┼───┼────┐
│  Rust Backend (Tauri v2)                     │   │    │
│                                              │   │    │
│  lib.rs   ── app setup, sidecar launch,      │   │    │
│              port discovery, tray, shortcuts  │   │    │
│  display.rs ── brightness/contrast (HTTP) ───┼───┼──┐ │
│  dark_mode.rs ── theme toggle (HTTP) ────────┼───┼──┤ │
│  volume.rs ── system volume (platform) ──────┘   │  │ │
│  tiling/   ── window tiling (macOS + Windows,    │  │ │
│               native OS APIs, no sidecar)        │  │ │
│  config.rs ── preferences persistence            │  │ │
│  tray.rs ── tray menu, window positioning,       │  │ │
│             keyboard shortcut dispatch ──────────┘  │ │
│                                                     │ │
│  app.emit("monitors-changed")  (pushes to frontend) │ │
└─────────────────────────────────────────────────────┼─┘
                                                      │
                              HTTP GET 127.0.0.1:port │
                                                      │
┌─────────────────────────────────────────────────────┼─┐
│  display-dj CLI sidecar (HTTP server)               │ │
│  https://github.com/synle/display-dj-cli            │ │
│                                                     │ │
│  /get_all ── list displays + brightness/contrast  ◄─┘ │
│  /set_one/<id>/<level> ── set brightness               │
│  /set_all/<level> ── set all brightness                │
│  /set_contrast_one/<id>/<level>                        │
│  /set_contrast_all/<level>                             │
│  /dark  /light  /theme                                 │
│  /health  /debug                                       │
│                                                        │
│  Handles: DDC/CI, gamma, DisplayServices, WMI,         │
│           osascript, registry, gsettings               │
└────────────────────────────────────────────────────────┘
```

---

## Directory Structure

```
display-dj2/
├── src/                          # Frontend (React 18 + TypeScript)
│   ├── main.tsx                  # Entry point: mounts React into <div id="root">
│   ├── App.tsx                   # Root component: all state, data fetching, layout
│   ├── App.css                   # All CSS (dark tray popup theme, sliders, toggles)
│   ├── types.ts                  # Shared TS interfaces (Monitor, Preferences, Profile, etc.)
│   ├── types.d.ts                # Global type definitions (Command, DisplayType, etc.)
│   ├── constants.ts              # Shared constants (LAPTOP_BUILT_IN_DISPLAY_ID)
│   ├── test/
│   │   └── setup.ts              # Vitest setup: jsdom, jest-dom, Tauri API mocks
│   └── components/
│       ├── Header.tsx            # Title + version + expand/collapse chevron
│       ├── Slider.tsx            # Reusable range slider, debounced onChange (150ms)
│       ├── AllMonitorsControl.tsx # Collapsed view: single brightness/contrast slider
│       ├── MonitorControl.tsx    # Expanded view: per-monitor sliders + editable name
│       ├── VolumeControl.tsx     # Volume slider with mute/unmute icon
│       ├── DarkModeToggle.tsx    # Dark / Light toggle buttons
│       ├── SettingsPanel.tsx     # Settings: min brightness, contrast, night mode, etc.
│       └── ProfileButtons.tsx    # Profile quick-action buttons with overflow menu
│
├── src-tauri/                    # Backend (Rust + Tauri v2)
│   ├── Cargo.toml                # Rust dependencies
│   ├── build.rs                  # Downloads sidecar binary + sets APP_VERSION from tauri.conf.json
│   ├── tauri.conf.json           # App config: window, tray icon, sidecar, bundling
│   ├── binaries/                 # display-dj CLI sidecar (per-platform)
│   ├── capabilities/default.json # Security permissions for frontend JS
│   ├── icons/                    # App icons (.icns, .ico, .png)
│   ├── tests/smoke.rs            # Integration smoke test
│   └── src/
│       ├── main.rs               # Binary entry point (calls lib::run)
│       ├── lib.rs                # App setup, sidecar launch, port, plugins, state, tray
│       ├── display.rs            # Monitor brightness/contrast via HTTP to sidecar
│       ├── dark_mode.rs          # Dark mode read/write via HTTP to sidecar
│       ├── volume.rs             # System volume (platform-specific: osascript/PowerShell/pactl)
│       ├── config.rs             # Preferences + monitor metadata persistence
│       ├── tray.rs               # System tray menu, window positioning, shortcuts
│       └── tiling/               # Window tiling module (macOS + Windows)
│           ├── mod.rs            # Shared types, layout math, TilingLayout enum
│           ├── macos.rs          # macOS: Accessibility API (AXUIElement), Tile Snap
│           └── windows.rs        # Windows: Win32 API (SetWindowPos, EnumWindows)
│
├── .github/workflows/
│   ├── build.yml                 # CI: tests + build on PRs (macOS/Windows/Linux)
│   └── release.yml               # CD: GitHub releases on v* tags
│
├── index.html                    # HTML shell that loads src/main.tsx
├── package.json                  # Node deps + displayDjCliVersion
├── vite.config.ts                # Vite config (dev server port 1420, test config)
└── tsconfig.json                 # TypeScript config
```

---

## Request Lifecycle

Here is the full path of a brightness change, from UI interaction to hardware:

### 1. User drags a slider

`MonitorControl.tsx` receives the `onChange` event from `Slider.tsx`.

```tsx
<Slider value={monitor.brightness} onChange={(val) => handleBrightnessChange(monitor.id, val)} />
```

### 2. Slider debounces (150ms)

`Slider.tsx` debounces the `onChange` callback to avoid flooding the backend:

```tsx
const debouncedOnChange = useRef(debounce((val: number) => onChange(val), 150));
```

### 3. Component calls invoke()

The handler in `App.tsx` calls the Tauri IPC bridge:

```tsx
const handleBrightnessChange = async (monitorId: string, value: number) => {
  await invoke('set_brightness', { monitorId, value });
};
```

### 4. Tauri IPC dispatches to Rust

Tauri deserializes `{ monitorId, value }` (camelCase) into Rust parameters (snake_case via serde):

```rust
#[tauri::command]
pub async fn set_brightness(monitor_id: String, value: u32) -> Result<(), String> { ... }
```

### 5. Rust clamps the value

`display.rs` enforces the minimum brightness floor:

```rust
let clamped = value.max(config::effective_min_brightness(&prefs));
```

### 6. Rust sends HTTP GET to sidecar

```rust
let url = format!("{}/set_one/{}/{}", base_url(), monitor_id, clamped);
reqwest::get(&url).await.map_err(|e| e.to_string())?;
```

### 7. Sidecar talks to hardware

The display-dj CLI uses DDC/CI (external monitors) or gamma tables / DisplayServices / WMI (built-in displays) to set the brightness.

### 8. Response flows back

HTTP 200 -> Rust `Ok(())` -> Tauri IPC -> `invoke()` resolves -> component updates state.

### 9. Backend-initiated changes

When a keyboard shortcut changes brightness, the backend calls the sidecar directly and then pushes an event to the frontend:

```rust
app.emit("monitors-changed", ())?;
```

The frontend listener triggers a refetch:

```tsx
listen('monitors-changed', () => fetchMonitors());
```

---

## Layer-by-Layer Breakdown

### 1. React Components (`src/components/`)

Presentational components that render UI and call `invoke()` to talk to the backend. No direct HTTP calls, no platform code.

| Component            | Responsibility                                                    |
| -------------------- | ----------------------------------------------------------------- |
| `App.tsx`            | Root state holder. Fetches all data on mount. Renders everything. |
| `Header.tsx`         | Title, version, expand/collapse toggle                            |
| `Slider.tsx`         | Reusable range input with debounce and fill styling               |
| `AllMonitorsControl` | Collapsed view: average brightness/contrast for all monitors      |
| `MonitorControl`     | Expanded view: per-monitor sliders + inline rename                |
| `VolumeControl`      | Volume slider with muted/unmuted icon                             |
| `DarkModeToggle`     | Dark / Light mode buttons                                         |
| `SettingsPanel`      | Full settings UI (min brightness, night mode, etc.)               |
| `ProfileButtons`     | Profile quick-apply buttons with overflow menu                    |

### 2. Tauri IPC Bridge

The frontend uses two Tauri APIs:

- **`invoke(command, params)`** -- Frontend calls a Rust function and awaits the result. Command names are snake_case strings. Parameters are a camelCase object (serde converts to snake_case).
- **`listen(event, callback)`** -- Backend pushes events to the frontend. Used when keyboard shortcuts or the night mode scheduler change state.

All registered commands:

| Module      | Commands                                                                                                                                                     |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `display`   | `get_monitors`, `set_brightness`, `set_all_brightness`, `set_contrast`, `set_all_contrast`, `rename_monitor`, `save_monitor_order`, `set_monitor_visibility` |
| `dark_mode` | `get_dark_mode`, `set_dark_mode`                                                                                                                             |
| `volume`    | `get_volume`, `set_volume`                                                                                                                                   |
| `config`    | `get_preferences`, `save_preferences`, `open_preferences_file`, `open_debug_log`, `get_app_version`                                                          |
| `tray`      | `apply_profile`                                                                                                                                              |
| `tiling`    | `get_tiling_supported`, `get_accessibility_trusted` (macOS + Windows; stubs return `false` on Linux)                                                         |

Events emitted by backend:

| Event               | When                                 |
| ------------------- | ------------------------------------ |
| `monitors-changed`  | Keyboard shortcut changes brightness |
| `dark-mode-changed` | Keyboard shortcut toggles dark mode  |
| `volume-changed`    | Keyboard shortcut changes volume     |

### 3. Rust Backend Commands (`src-tauri/src/`)

Each `#[tauri::command]` function lives in its domain module. They validate input, read/write shared state (`AppState`), and delegate to the sidecar or platform APIs.

| File           | Responsibility                                                                                                                                                                                           |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `lib.rs`       | App bootstrap: sidecar launch, port discovery, plugins, state, tray init, night mode scheduler                                                                                                           |
| `display.rs`   | Monitor CRUD via HTTP to sidecar. Converts `DjDisplay` -> `Monitor`. Merges with config metadata.                                                                                                        |
| `dark_mode.rs` | Dark mode get/set via HTTP to sidecar (`/theme`, `/dark`, `/light`)                                                                                                                                      |
| `volume.rs`    | System volume get/set (platform-specific, no sidecar)                                                                                                                                                    |
| `config.rs`    | Preferences JSON persistence, defaults, migration, min brightness                                                                                                                                        |
| `tray.rs`      | Tray menu, window positioning, keyboard shortcut registration/dispatch                                                                                                                                   |
| `tiling/`      | Window tiling module. `mod.rs`: shared types + layout math. `macos.rs`: AXUIElement + Tile Snap. `windows.rs`: Win32 SetWindowPos + EnumWindows. 19 layouts, multi-monitor, platform-gated (Linux stubs) |

Shared state (`AppState` in `lib.rs`):

```rust
pub struct AppState {
    pub preferences: Mutex<Preferences>,     // Thread-safe config access
    pub last_tray_rect: Mutex<Option<Rect>>, // For window positioning
    pub sidecar_child: Mutex<Option<CommandChild>>, // Sidecar process handle
}
```

The sidecar port is a global `AtomicU16` accessed via `crate::server_port()`.

### 4. HTTP Sidecar (display-dj CLI)

All display and dark mode operations go through the sidecar's HTTP API. The Rust backend makes simple `GET` requests -- no request bodies, no auth.

| Route                                | Purpose                                  |
| ------------------------------------ | ---------------------------------------- |
| `GET /health`                        | Server readiness check (startup)         |
| `GET /get_all`                       | List displays + live brightness/contrast |
| `GET /set_one/<id>/<level>`          | Set one display's brightness             |
| `GET /set_all/<level>`               | Set all displays' brightness             |
| `GET /set_contrast_one/<id>/<level>` | Set one display's contrast (DDC-only)    |
| `GET /set_contrast_all/<level>`      | Set all displays' contrast (DDC-only)    |
| `GET /theme`                         | Get current theme (dark/light)           |
| `GET /dark`                          | Switch to dark mode                      |
| `GET /light`                         | Switch to light mode                     |
| `GET /debug`                         | Full diagnostics dump                    |

Sidecar lifecycle:

1. `lib.rs` finds an available port (starting from 51337)
2. Spawns `display-dj-server serve <port>` via Tauri shell plugin
3. Polls `/health` until ready (up to 5 seconds)
4. On app exit, `child.kill()` terminates the sidecar

### 5. Platform-Specific Code (`volume.rs`, `tiling/`)

Two modules have `#[cfg(target_os)]` conditional compilation:

| Platform | Method                                                                      |
| -------- | --------------------------------------------------------------------------- |
| macOS    | `osascript` -- CoreAudio `get volume settings` / `set volume output volume` |
| Windows  | PowerShell + inline C# `Add-Type` for WASAPI COM (`IAudioEndpointVolume`)   |
| Linux    | `pactl get-sink-volume` / `pactl set-sink-volume`                           |

**Window Tiling** (`tiling/` module) -- macOS + Windows, no sidecar involved:

| Platform | Method                                                                                                              |
| -------- | ------------------------------------------------------------------------------------------------------------------- |
| macOS    | Accessibility API (`AXUIElement`) to move/resize windows, `NSScreen` for display bounds. Tile Snap via `CGEventTap` |
| Windows  | Win32 API (`GetForegroundWindow`, `SetWindowPos`, `EnumDisplayMonitors`, `EnumWindows`) via `windows` crate v0.58   |
| Linux    | Not yet implemented (planned: X11/EWMH, Wayland per-compositor IPC). Stub commands return `false`                   |

---

## State Management

There is no external state library. All state lives in `App.tsx` as `useState` hooks.

### Frontend State (`App.tsx`)

| State           | Type        | Source                      | Updated by                                      |
| --------------- | ----------- | --------------------------- | ----------------------------------------------- |
| `monitors`      | `Monitor[]` | `invoke("get_monitors")`    | Slider change, backend event, visibility change |
| `darkMode`      | `boolean`   | `invoke("get_dark_mode")`   | Toggle click, backend event                     |
| `volume`        | `number`    | `invoke("get_volume")`      | Slider change, backend event                    |
| `minBrightness` | `number`    | `invoke("get_preferences")` | Settings save                                   |
| `showContrast`  | `boolean`   | `invoke("get_preferences")` | Settings save                                   |
| `profiles`      | `Profile[]` | `invoke("get_preferences")` | Settings save                                   |
| `expanded`      | `boolean`   | User toggle                 | Header chevron click                            |
| `version`       | `string`    | `invoke("get_app_version")` | Once on mount                                   |

### Refresh triggers

1. **On mount**: `useEffect` fetches all data sources
2. **Backend events**: `listen("monitors-changed")`, `listen("dark-mode-changed")`, `listen("volume-changed")` trigger refetches
3. **Visibility change**: `visibilitychange` listener refetches everything when the tray popup becomes visible (catches external changes)
4. **Optimistic updates**: Slider/toggle handlers update local state immediately, then `invoke()` in the background

### Backend State (`AppState`)

- `preferences: Mutex<Preferences>` -- shared across all Tauri commands
- `last_tray_rect: Mutex<Option<Rect>>` -- used by window positioning
- `sidecar_child: Mutex<Option<CommandChild>>` -- killed on app exit
- `SERVER_PORT: AtomicU16` -- global, set once at startup

---

## Data Flow

### User-initiated (slider drag)

```
Component ──invoke()──► Tauri IPC ──► Rust command ──HTTP GET──► Sidecar ──► Hardware
    │                                      │
    └── optimistic state update            └── Ok(()) flows back through IPC
```

### Backend-initiated (keyboard shortcut)

```
Keyboard shortcut
    │
    ▼
tray.rs::execute_command()
    │
    ├── HTTP GET ──► Sidecar ──► Hardware
    │
    └── app.emit("monitors-changed")
              │
              ▼
        Frontend listener ──invoke("get_monitors")──► Rust ──HTTP──► Sidecar
              │
              └── setMonitors(freshData)
```

### Night mode scheduler

```
lib.rs background thread (every 60s)
    │
    ├── check_night_mode_schedule()
    │     │
    │     ├── Read preferences (NightModeSchedule)
    │     ├── Compare current time vs nightStart / dayStart
    │     │
    │     ├── HTTP GET /set_all/<brightness> ──► Sidecar
    │     └── HTTP GET /dark or /light ──► Sidecar
    │
    └── app.emit("monitors-changed") + app.emit("dark-mode-changed")
```

---

## Key Architectural Rules

1. **No platform-specific display code in Rust.** All display and dark mode operations go through the HTTP sidecar. Only `volume.rs` and `tiling/` have `#[cfg(target_os)]` blocks. Tiling uses native OS APIs directly (no sidecar).

2. **snake_case commands, camelCase parameters.** Tauri commands are `snake_case` in Rust and called with `snake_case` strings from the frontend. Parameter objects use `camelCase` keys -- serde converts automatically via `#[serde(rename_all = "camelCase")]`.

3. **Brightness has an absolute floor.** All brightness values are clamped to `effective_min_brightness()` which enforces `ABSOLUTE_MIN_BRIGHTNESS = 5`. The user-configured `minBrightness` in preferences can raise but never lower this floor.

4. **Contrast is DDC-only and optional.** `contrast` is `Option<u32>` / `number | null`. Built-in displays return `null`. The contrast slider is hidden by default and toggled via the `showContrast` preference.

5. **Monitor metadata is append-only.** `MonitorMetadata` entries in `preferences.monitorConfigs` are never removed when a monitor is unplugged. This preserves labels and sort order across plug/unplug cycles.

6. **Preferences use `#[serde(default)]`.** Old config files missing new fields gracefully fall back to defaults without breaking deserialization.

7. **Errors are strings, not crashes.** Backend functions return `Result<T, String>`. Frontend `invoke()` calls are wrapped in try/catch -- the UI silently keeps the last known state on error.

8. **The sidecar port is global.** Set once at startup in `AtomicU16`, accessed everywhere via `crate::server_port()`. All modules use `base_url()` to build HTTP URLs.

9. **Tauri commands accessing AppState must be `async` on macOS.** Sync `#[tauri::command]` functions that take `State<'_, AppState>` block the macOS main-thread run-loop, preventing tray icon click events from firing. This was the root cause of a hard-to-diagnose bug where both left-click and right-click on the tray icon stopped working. See `config.rs` `save_preferences` for the documented warning.

10. **Do not use `write_debug_log()` in frequently-called sync commands.** `write_debug_log()` locks `state.preferences` to check the `debug_logging` flag. In sync Tauri commands called on every frontend render (like `get_preferences`), this mutex contention starves the macOS run-loop and breaks tray icon events. Use `log::info!` in those paths. `write_debug_log()` is safe in async or infrequently-called commands.

---

## Where to Edit

Quick reference for common tasks:

| Task                            | Files to change                                                                                                                                              |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| New brightness/contrast feature | `display.rs` (Rust), `MonitorControl.tsx` or `AllMonitorsControl.tsx` (UI)                                                                                   |
| New dark mode behavior          | `dark_mode.rs` (Rust), `DarkModeToggle.tsx` (UI)                                                                                                             |
| New volume behavior             | `volume.rs` (Rust, platform-specific), `VolumeControl.tsx` (UI)                                                                                              |
| New preference field            | `config.rs` (add to `Preferences` struct + default), `types.ts` (TS interface), `SettingsPanel.tsx` (UI)                                                     |
| New Tauri command               | Domain module (Rust), `lib.rs` (register in `invoke_handler`), frontend component                                                                            |
| New keyboard shortcut command   | `tray.rs` (`execute_command` match arm), `config.rs` (default keybinding)                                                                                    |
| New UI component                | `src/components/NewComponent.tsx` + `NewComponent.test.tsx`, wire into `App.tsx`                                                                             |
| Tray menu change                | `tray.rs` (`build_tray_menu`)                                                                                                                                |
| Window tiling (macOS + Windows) | `tiling/mod.rs` (shared layout math), `tiling/macos.rs` (AX API), `tiling/windows.rs` (Win32), `tray.rs` (Tiling submenu), `config.rs` (`TilingPreferences`) |
| Window positioning              | `tray.rs` (`position_window_near_tray`) -- read the doc comment first!                                                                                       |
| Night mode schedule logic       | `lib.rs` (`check_night_mode_schedule`, `is_night_time`)                                                                                                      |
| Sidecar version bump            | `package.json` (`displayDjCliVersion`), review [upstream changes](https://github.com/synle/display-dj-cli)                                                   |
| CI changes                      | `.github/workflows/build.yml` or `release.yml`                                                                                                               |

---

## Adding a New Tauri Command

### 1. Define the Rust function

In the appropriate module (e.g., `display.rs`). **Use `async fn` if the command accesses `State<'_, AppState>`** — sync commands with state access break macOS tray icon events (see rule 9):

```rust
/// Does something useful.
#[tauri::command]
pub async fn my_new_command(
    state: tauri::State<'_, crate::AppState>,
    some_param: String,
) -> Result<String, String> {
    Ok(format!("Hello {}", some_param))
}
```

### 2. Register it in `lib.rs`

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    display::my_new_command,
])
```

### 3. Call from the frontend

```typescript
const result = await invoke<string>('my_new_command', { someParam: 'world' });
```

Note: Rust `some_param` maps to TypeScript `someParam` via serde's camelCase renaming.

### 4. Add tests

- Rust: `#[cfg(test)]` block in the module
- Frontend: `*.test.tsx` if it affects the UI

---

## Window Positioning (multi-monitor DPI)

The tray popup must appear next to the tray icon, which can be on any monitor with any DPI scale factor. **Read the doc comment on `position_window_near_tray` in `tray.rs` before modifying positioning code.**

### The coordinate spaces

| API                                     | Returns                             | Coordinate space                                             |
| --------------------------------------- | ----------------------------------- | ------------------------------------------------------------ |
| `tray.rect()`                           | `PhysicalPosition` / `PhysicalSize` | Global physical pixels                                       |
| `monitor.position()` / `monitor.size()` | `PhysicalPosition` / `PhysicalSize` | Global physical pixels                                       |
| `window.set_position(PhysicalPosition)` | --                                  | Tauri divides by `window.scale_factor()` to get macOS points |
| `window.scale_factor()`                 | `f64`                               | Scale of the monitor the window is **currently** on          |

### The pitfall

`window.scale_factor()` reflects the **current** monitor, not the target. When the window is on a 1x display and the tray is clicked on a 2x display, Tauri divides by 1 instead of 2 -- placing the window at double the intended coordinate (off-screen).

### The fix: scale compensation

All positioning math runs in global physical pixel space using `target_scale` (the tray's monitor). Before calling `set_position`, multiply by `window_scale / target_scale`:

```
Tauri does:     point = arg / window_scale
We need:        point = physical / target_scale
So we pass:     arg = physical * window_scale / target_scale
```

When both scales match (same monitor), the compensation is 1x (no-op).

### Debug logging

Enable via tray menu > "Debug" > "Enable Logging". Logs are written to `debug.log` in the config directory (auto-truncated at 1 MB). Each tray click logs: tray rect, all monitors, target selection, window scale, computed position, compensation factor, and final `set_position` arguments.

---

## Monitor Identity (UID scheme)

Each monitor is identified by a composite UID: `{api_id}::{api_model_name}` (e.g. `"1::Dell U2723QE"`, `"builtin::Built-in Display"`). This is more stable than the raw integer ID from the sidecar API, which can collide when monitors are swapped.

- `Monitor.id` -- raw API id, used for sidecar HTTP calls (`/set_one/{id}/{value}`)
- `Monitor.uid` -- composite key, used for config lookups, React keys, rename/reorder operations
- On startup, a one-time migration converts old `monitor-configs.json` entries into `MonitorMetadata` format within preferences

---

## Configuration Files

Config directory: `~/Library/Application Support/display-dj/` (macOS), `%APPDATA%/display-dj/` (Windows), `~/.config/display-dj/` (Linux).

### preferences.json

| Field                    | Type   | Default               | Purpose                                                      |
| ------------------------ | ------ | --------------------- | ------------------------------------------------------------ |
| `showIndividualDisplays` | bool   | `false`               | Start in expanded view                                       |
| `brightnessDelta`        | number | `10`                  | Step size for keyboard shortcut brightness changes           |
| `contrastDelta`          | number | `10`                  | Step size for contrast changes                               |
| `minBrightness`          | number | `10`                  | Minimum brightness floor (absolute floor: 5)                 |
| `showContrast`           | bool   | `false`               | Show contrast sliders in the UI                              |
| `nightModeSchedule`      | object | disabled, 21:00-07:00 | Auto brightness + dark mode by time of day                   |
| `keyBindings`            | array  | 9 default bindings    | Global keyboard shortcuts                                    |
| `profiles`               | array  | `[]`                  | Saved brightness/contrast/dark mode/volume presets           |
| `monitorConfigs`         | array  | `[]`                  | Per-monitor metadata (label, sort order, hidden)             |
| `tiling`                 | object | enabled, 50/33/0      | Window tiling settings (enabled, halfRatio, thirdRatio, gap) |

### Night mode schedule fields

| Field             | Type   | Default   | Purpose                                   |
| ----------------- | ------ | --------- | ----------------------------------------- |
| `enabled`         | bool   | `false`   | Whether the schedule is active            |
| `nightStart`      | string | `"21:00"` | Time to switch to night mode (HH:MM, 24h) |
| `nightBrightness` | number | `20`      | Brightness during night window            |
| `dayStart`        | string | `"07:00"` | Time to switch to day mode (HH:MM, 24h)   |
| `dayBrightness`   | number | `100`     | Brightness during day window              |

### Keyboard shortcut command format

Format: `command/<action>/<value>`

| Action             | Values                                               | Effect                                 |
| ------------------ | ---------------------------------------------------- | -------------------------------------- |
| `changeBrightness` | 0-100                                                | Sets all monitors' brightness          |
| `changeContrast`   | 0-100                                                | Sets all monitors' contrast            |
| `changeDarkMode`   | `toggle`, `dark`, `light`                            | Toggles or sets dark mode              |
| `changeVolume`     | 0-100                                                | Sets system volume                     |
| `changeProfile`    | Profile index (0, 1, 2, ...)                         | Applies a saved profile                |
| `tile`             | Layout name (e.g. `leftHalf`, `maximize`, `restore`) | Tiles focused window (macOS + Windows) |

### Monitor metadata (monitorConfigs)

| Field       | Type   | Purpose                                       |
| ----------- | ------ | --------------------------------------------- |
| `uid`       | string | Composite key: `"{api_id}::{api_model_name}"` |
| `apiId`     | string | Raw ID from sidecar (e.g. `"1"`, `"builtin"`) |
| `apiName`   | string | Model name from sidecar API                   |
| `label`     | string | User-set name (empty = use apiName)           |
| `sortOrder` | number | Display order in UI (lower = higher)          |
| `hidden`    | bool   | Whether the monitor is hidden from main UI    |

---

## App Versioning

The app version flows from a single source through the build pipeline to the UI:

```
tauri.conf.json ("version": "3.0.0")
       │
       ▼
build.rs reads it at compile time
       │
       ▼
cargo:rustc-env=APP_VERSION=3.0.0
       │
       ▼
config.rs: get_app_version() → env!("APP_VERSION")
       │
       ▼
App.tsx: invoke("get_app_version") → setVersion()
       │
       ▼
Header.tsx: "Display DJ v3.0.0"
```

- `tauri.conf.json` → `"version"`: The single source of truth. Controls both the UI header and installer/bundle metadata.
- `package.json` → `"version"`: `0.0.0` — not used (not published to npm).
- `Cargo.toml` → `version`: `0.0.0` — not used (crate not published).
- Release versioning is driven by git tags (`v*` triggers `release.yml`).

---

## display-dj CLI Sidecar

The [display-dj CLI](https://github.com/synle/display-dj-cli) is bundled as a Tauri sidecar. The version is defined in `package.json` under `displayDjCliVersion`.

Pre-built binaries for all 6 platforms are committed to the repo. The build script (`src-tauri/build.rs`) tries to download the latest from GitHub releases first (10s timeout), then falls back to the committed binary if the download fails. This enables offline builds and faster CI.

### Sidecar binaries

```
src-tauri/binaries/
  display-dj-server-aarch64-apple-darwin        # macOS ARM
  display-dj-server-x86_64-apple-darwin         # macOS Intel
  display-dj-server-x86_64-pc-windows-msvc.exe  # Windows x64
  display-dj-server-aarch64-pc-windows-msvc.exe # Windows ARM
  display-dj-server-x86_64-unknown-linux-gnu    # Linux x64
  display-dj-server-aarch64-unknown-linux-gnu   # Linux ARM
```

### Building from source

```bash
git clone https://github.com/synle/display-dj-cli.git
cd display-dj-cli
cargo build --release
cp target/release/display-dj ../display-dj2/src-tauri/binaries/display-dj-server-<target-triple>
```

### Verifying

```bash
./src-tauri/binaries/display-dj-server-aarch64-apple-darwin serve 51337 &
curl http://127.0.0.1:51337/health    # {"status":"ok"}
curl http://127.0.0.1:51337/get_all   # display JSON
kill %1
```

---

## Known Limitations

| Limitation                    | Details                                                                                                                               |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| DDC/CI not universal          | Budget monitors and some HDMI connections may not support it                                                                          |
| Built-in HDMI on base M1/M2   | No DDC/CI support. Use USB-C/DisplayPort                                                                                              |
| Global shortcuts on Wayland   | Wayland restricts global hotkey capture. X11 works fine                                                                               |
| Tray clicks dead on macOS     | Sync Tauri commands or `write_debug_log()` in hot sync commands starve the run-loop. See rules 9-10 above                             |
| Tiling requires Accessibility | macOS tiling needs Accessibility permission. Without it, tile commands silently do nothing. Windows needs no special permissions      |
| Tile Snap macOS-only          | Mouse edge snapping (Tile Snap) is only implemented for macOS. Keyboard shortcuts and tray menu tiling work on both macOS and Windows |
| Tiling not on Linux           | Window tiling is not yet implemented on Linux. On Linux the tray submenu and Settings toggle are hidden                               |
| Tray left-click on Linux      | AppIndicator doesn't always fire left-click                                                                                           |
| Dark mode on non-GNOME        | `gsettings` is GNOME-specific. KDE, XFCE not supported                                                                                |

---

## Troubleshooting

| Problem                                 | Fix                                                                                                                            |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `command not found: rustc`              | Run `source "$HOME/.cargo/env"` or reopen terminal                                                                             |
| First build takes 5+ minutes            | Normal. Rust compiles from source. Subsequent runs are ~5-15s                                                                  |
| App launched but can't find it          | System tray app. macOS: menu bar top-right. Windows: system tray bottom-right. Linux: top panel                                |
| "sidecar not found"                     | Binary missing from `src-tauri/binaries/`. See [sidecar section](#display-dj-cli-sidecar)                                      |
| "server did not become ready"           | Check binary is executable (`chmod +x`), port available (`lsof -i :51337`), test directly                                      |
| "No displays found"                     | Run `./src-tauri/binaries/display-dj-server-* list` directly. Linux: check `ddcutil detect`                                    |
| Dark mode toggle does nothing (Linux)   | Requires GNOME. Check `echo $XDG_CURRENT_DESKTOP`                                                                              |
| macOS "System Events" permission prompt | Expected on first launch. Volume control uses `osascript` which requires System Events access. Click Allow — only prompts once |
