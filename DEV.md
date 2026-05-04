# Developer Guide

Full architecture reference for Display DJ v7. Read this before making changes.

---

## Table of Contents

- [IDE Setup (VS Code)](#ide-setup-vs-code)
- [Architecture Overview](#architecture-overview)
- [Directory Structure](#directory-structure)
- [Request Lifecycle](#request-lifecycle)
- [Layer-by-Layer Breakdown](#layer-by-layer-breakdown)
  - [React Components](#1-react-components-src-components)
  - [Tauri IPC Bridge](#2-tauri-ipc-bridge)
  - [Rust Backend Commands](#3-rust-backend-commands-src-taurisrc)
  - [Vendored platform core (`core/`)](#4-vendored-platform-core-srctauri-srccore)
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
- [Known Limitations](#known-limitations)
- [Troubleshooting](#troubleshooting)

---

## IDE Setup (VS Code)

The project includes a `.vscode/` directory with recommended settings, launch configs, and extensions.

### Prerequisites

1. [Node.js 20+](https://nodejs.org/) and npm
2. [Rust toolchain](https://rustup.rs/) (`rustup` + stable)
3. [Tauri v2 CLI](https://v2.tauri.app/): `cargo install tauri-cli`
4. Platform dependencies (see [CLAUDE.md](CLAUDE.md) for Linux `apt` packages)

### Getting Started

```bash
git clone https://github.com/synle/display-dj.git
cd display-dj
npm install
npx tauri dev    # Full app (frontend + Rust backend; platform code is vendored in-process)
```

### Recommended VS Code Extensions

Install the recommended extensions when prompted (or via `.vscode/extensions.json`):

- **rust-analyzer** -- Rust LSP (code completion, inline errors, go-to-definition for all Rust code)
- **Prettier** -- Auto-format TypeScript, JSON, Markdown on save
- **Tauri** -- Tauri v2 config schema support
- **ESLint** -- TypeScript linting

### Launch Configurations

Open the **Run and Debug** panel (Cmd+Shift+D) to use these pre-configured launch targets:

| Configuration                  | What it does                                                                                  |
| ------------------------------ | --------------------------------------------------------------------------------------------- |
| **Tauri Dev (Full App)**       | Runs `npx tauri dev` -- starts Vite + Rust backend. The main development workflow.            |
| **Vite Dev (Frontend Only)**   | Runs `npm run dev` -- frontend only at `localhost:1420`. No backend. Useful for pure UI work. |
| **Vitest (Run Tests)**         | Runs `npm test` -- all frontend tests once.                                                   |
| **Vitest (Watch Mode)**        | Runs `npm run test:watch` -- re-runs tests on file change.                                    |
| **Cargo Test (Rust Backend)**  | Runs `cargo test` in `src-tauri/`. All 222+ Rust unit tests.                                  |
| **Cargo Check (Rust Compile)** | Runs `cargo check` in `src-tauri/`. Fast compile check without building.                      |
| **Tauri Build (Production)**   | Runs `npx tauri build` -- creates the production `.dmg`/`.exe`/`.deb`/`.AppImage`.            |

### rust-analyzer Setup

The `.vscode/settings.json` configures rust-analyzer to find the Cargo workspace at `src-tauri/Cargo.toml`. If you see "failed to discover workspace" errors, ensure rust-analyzer is pointed at the right path:

```json
"rust-analyzer.linkedProjects": ["src-tauri/Cargo.toml"]
```

### Common Dev Workflows

| Task                  | Command                                  |
| --------------------- | ---------------------------------------- |
| Run the app locally   | `npx tauri dev`                          |
| Check Rust compiles   | `cd src-tauri && cargo check`            |
| Run all tests         | `npm test && cd src-tauri && cargo test` |
| Format frontend files | `npx prettier --write src/`              |
| Production build      | `npx tauri build`                        |

---

## Architecture Overview

Display DJ is a system tray app built with [Tauri v2](https://v2.tauri.app/). A React frontend runs inside a WebView and communicates with a Rust backend over Tauri IPC. As of v7.0.0, **all platform code (DDC/CI, gamma, WMI, DisplayServices, dark mode, volume, wallpaper, slideshow) is vendored in-process** under `src-tauri/src/core/`. There is no separate helper binary, no HTTP server, and no port discovery. Tauri commands call `core::*` functions directly via `tauri::async_runtime::spawn_blocking`.

```
┌──────────────────────────────────────────────────────┐
│  React 19 + TypeScript + Vite 6                      │
│  (WebView: WebKit on macOS/Linux, WebView2 on Win)   │
│                                                      │
│  Components:  App.tsx (root state)                   │
│               MonitorControl / AllMonitorsControl    │
│               VolumeControl / DarkModeToggle         │
│               SettingsPanel / ProfileButtons         │
│                                                      │
│  invoke("command_name", { params })   ──────────┐    │
│  listen("event-name", callback)       ◄─────┐   │    │
└──────────────────────────────────────────────┼───┼───┘
                                               │   │
                              Tauri IPC bridge │   │
                                               │   │
┌──────────────────────────────────────────────┼───┼───┐
│  Rust Backend (Tauri v2)                     │   │   │
│                                              │   │   │
│  lib.rs   ── app setup, plugins, state,      │   │   │
│              tray init, night mode scheduler │   │   │
│  display.rs    ── Tauri-cmd wrappers around  │   │   │
│  dark_mode.rs     core::* (spawn_blocking)   │   │   │
│  volume.rs                                   │   │   │
│  wallpaper.rs                                │   │   │
│  config.rs ── preferences persistence        │   │   │
│  tray.rs   ── tray menu, window positioning, │   │   │
│               keyboard shortcut dispatch     │   │   │
│  tiling/   ── window tiling (macOS+Win+X11,  │   │   │
│               native OS APIs, in-process)    │   │   │
│                                              │   │   │
│  app.emit(...)  (pushes events to frontend)  │───┘   │
│                                              │       │
│           direct function calls (no HTTP)    │       │
│                                              ▼       │
│  ┌────────────────────────────────────────────────┐  │
│  │  src-tauri/src/core/  (vendored platform)      │  │
│  │                                                │  │
│  │  core::PlatformImpl (cfg alias →               │  │
│  │     core::macos / core::windows / core::linux) │  │
│  │  core::display   ── set_all_brightness,        │  │
│  │                     set_one_brightness, etc.   │  │
│  │  core::theme     ── dark mode get/set          │  │
│  │  core::volume    ── system volume get/set      │  │
│  │  core::wallpaper ── set + slideshow timer/     │  │
│  │                     state/cycling              │  │
│  └────────────────────┬───────────────────────────┘  │
└───────────────────────┼──────────────────────────────┘
                        │  OS APIs
                        ▼
        IOKit / DisplayServices / NSWorkspace (macOS)
        WMI / DDC/CI / IDesktopWallpaper        (Windows)
        ddcutil / brightnessctl / gsettings     (Linux)
```

---

## Directory Structure

```
display-dj2/
├── .vscode/                      # VS Code settings, launch configs, extension recs
│   ├── launch.json               # Debug/run configurations (Tauri dev, tests, build)
│   ├── settings.json             # Editor settings (formatter, rust-analyzer path)
│   └── extensions.json           # Recommended extensions
├── src/                          # Frontend (React 19 + TypeScript)
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
│       ├── AboutPanel.tsx        # About: version, update check, engine, build date, macOS troubleshooting
│       └── ProfileButtons.tsx    # Profile quick-action buttons with overflow menu
│
├── src-tauri/                    # Backend (Rust + Tauri v2)
│   ├── Cargo.toml                # Rust dependencies
│   ├── build.rs                  # tauri_build::build() + APP_VERSION/BUILD_DATE compile-time env vars
│   ├── tauri.conf.json           # App config: window, tray icon, bundling
│   ├── capabilities/default.json # Security permissions for frontend JS
│   ├── icons/                    # App icons (.icns, .ico, .png)
│   ├── tests/smoke.rs            # Integration smoke test
│   └── src/
│       ├── main.rs               # Binary entry point (calls lib::run)
│       ├── lib.rs                # App setup, plugins, state, tray init, night mode scheduler
│       ├── core/                 # Vendored platform code (in-process; no HTTP, no helper binary)
│       │   ├── mod.rs            # Shared types: DisplayInfo, DisplayControl, Platform; PlatformImpl cfg alias
│       │   ├── macos.rs          # macOS DDC/CI (ddc-macos) + DisplayServices for built-in
│       │   ├── windows.rs        # Windows DDC/CI (ddc-winapi) + WMI for built-in
│       │   ├── linux.rs          # Linux ddcutil + brightnessctl
│       │   ├── theme.rs          # Cross-platform dark mode get/set
│       │   ├── volume.rs         # Cross-platform volume get/set
│       │   ├── wallpaper.rs      # Wallpaper set + slideshow timer/state/cycling
│       │   └── display.rs        # set_all_brightness / set_one_brightness + contrast variants
│       ├── display.rs            # Tauri-cmd wrappers around core::display (spawn_blocking)
│       ├── dark_mode.rs          # Tauri-cmd wrappers around core::theme
│       ├── volume.rs             # Tauri-cmd wrappers around core::volume
│       ├── wallpaper.rs          # Tauri-cmd wrappers around core::wallpaper
│       ├── config.rs             # Preferences + monitor metadata persistence
│       ├── tray.rs               # System tray menu, window positioning, shortcuts
│       └── tiling/               # Window tiling module (macOS + Windows + Linux/X11)
│           ├── mod.rs            # Shared types, layout math, TilingLayout enum
│           ├── macos.rs          # macOS: Accessibility API (AXUIElement), Tile Snap
│           ├── windows.rs        # Windows: Win32 API (SetWindowPos, EnumWindows)
│           └── linux.rs          # Linux: x11rb + EWMH
│
├── .github/workflows/
│   ├── build.yml                 # CI: tests + build on PRs (macOS/Windows/Linux)
│   └── release-official.yml      # CD: GitHub releases on v* tags
│
├── index.html                    # HTML shell that loads src/main.tsx
├── package.json                  # Node deps
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

### 6. Rust calls into `core::*` on a blocking thread

The Tauri command wraps the (potentially blocking) DDC/CI / WMI / DisplayServices call in `spawn_blocking`:

```rust
tauri::async_runtime::spawn_blocking(move || {
    core::display::set_one_brightness(&monitor_id, clamped, "force")
})
.await
.map_err(|e| e.to_string())??;
```

### 7. `core::*` talks to hardware

`core::display::set_one_brightness` resolves the monitor via `core::PlatformImpl::detect_displays()`, then calls DDC/CI (external monitors) or DisplayServices / WMI / brightnessctl (built-in displays) directly inside the Rust process.

### 8. Response flows back

`core::*` returns `Ok(())` (or an error string) → the Tauri command resolves → `invoke()` resolves on the frontend → component updates state.

### 9. Backend-initiated changes

When a keyboard shortcut changes brightness, `tray.rs::execute_command()` calls `core::*` directly on a background thread and then pushes an event to the frontend:

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

Presentational components that render UI and call `invoke()` to talk to the backend. No direct OS calls, no platform code.

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
| `AboutPanel`         | About: version check, engine, build date, macOS troubleshooting   |
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
| `tiling`    | `get_tiling_supported`, `get_accessibility_trusted` (macOS + Windows; X11 supported on Linux when `$DISPLAY` is set)                                         |

Events emitted by backend:

| Event               | When                                 |
| ------------------- | ------------------------------------ |
| `monitors-changed`  | Keyboard shortcut changes brightness |
| `dark-mode-changed` | Keyboard shortcut toggles dark mode  |
| `volume-changed`    | Keyboard shortcut changes volume     |

### 3. Rust Backend Commands (`src-tauri/src/`)

Each `#[tauri::command]` function lives in its domain module. They validate input, read/write shared state (`AppState`), and delegate to `core::*` (in-process) or to `tiling/` (native OS APIs).

| File           | Responsibility                                                                                                                                                                                        |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `lib.rs`       | App bootstrap: plugins, state, tray init, night mode scheduler                                                                                                                                        |
| `display.rs`   | Tauri-cmd wrapper around `core::display::*`. Converts `core::DisplayInfo` -> `Monitor`. Merges with config metadata.                                                                                  |
| `dark_mode.rs` | Tauri-cmd wrapper around `core::theme::{get_dark_mode, set_dark_mode}`                                                                                                                                |
| `volume.rs`    | Tauri-cmd wrapper around `core::volume::{get_volume, set_volume}`                                                                                                                                     |
| `wallpaper.rs` | Tauri-cmd wrapper around `core::wallpaper::*` plus image validation, MD5 caching, remote-pack zip download (via `reqwest::blocking`)                                                                  |
| `config.rs`    | Preferences JSON persistence, defaults, migration, min brightness                                                                                                                                     |
| `tray.rs`      | Tray menu, window positioning, keyboard shortcut registration/dispatch (in-process; calls `core::*` and `tiling::*` directly)                                                                         |
| `tiling/`      | Window tiling module. `mod.rs`: shared types + layout math + plan helpers. `macos.rs`: AXUIElement + Tile Snap. `windows.rs`: Win32 SetWindowPos + EnumWindows. `linux.rs`: x11rb + EWMH. 19 layouts. |

Shared state (`AppState` in `lib.rs`):

```rust
pub struct AppState {
    pub preferences: Mutex<Preferences>,     // Thread-safe config access
    pub last_tray_rect: Mutex<Option<Rect>>, // For window positioning
    pub keep_awake: Mutex<Option<KeepAwake>>,// Sleep-prevention guard
    // ... is_dark_mode, is_muted, tiling_state, etc.
}
```

There is no `sidecar_child` field, no `SERVER_PORT`, and no `server_port()` accessor — all of that was removed in v7.0.0.

### 4. Vendored platform core (`src-tauri/src/core/`)

All display, dark-mode, volume, and wallpaper operations live here. Pure platform code, no Tauri types — usable from any Rust caller (which is also why it can stay in sync with the standalone [display-dj-cli](https://github.com/synle/display-dj-cli) upstream).

| Module            | Responsibility                                                                                              |
| ----------------- | ----------------------------------------------------------------------------------------------------------- |
| `core::mod`       | Shared types: `DisplayInfo`, `DisplayControl` trait, `Platform` trait. `core::PlatformImpl` cfg-gated alias |
| `core::macos`     | macOS DDC/CI (via `ddc-macos`) + DisplayServices for built-in screens                                       |
| `core::windows`   | Windows DDC/CI (via `ddc-winapi`) + WMI for built-in screens                                                |
| `core::linux`     | Linux `ddcutil` for external + `brightnessctl` for built-in                                                 |
| `core::theme`     | Dark mode get/set (osascript/`defaults` on macOS, registry on Windows, `gsettings` on Linux)                |
| `core::volume`    | System volume get/set (osascript on macOS, PowerShell+WASAPI on Windows, `pactl` on Linux)                  |
| `core::wallpaper` | Wallpaper set + slideshow timer/state/cycling (NSWorkspace, `IDesktopWallpaper`, gsettings/feh)             |
| `core::display`   | High-level helpers (`set_all_brightness`, `set_one_brightness`, contrast variants) over `PlatformImpl`      |

`core::PlatformImpl` is a `cfg`-gated type alias resolved at compile time to `core::macos::Platform`, `core::windows::Platform`, or `core::linux::Platform`. Callers don't see the per-OS split.

### 5. Platform-Specific Code (`volume.rs`, `tiling/`)

The platform split is now done inside `core::*` (via `cfg`-gated `PlatformImpl`). The only modules in `src-tauri/src/` outside `core/` that have `#[cfg(target_os)]` blocks are `tiling/` (window tiling needs OS-specific window-server APIs, not display APIs).

**Window Tiling** (`tiling/` module) -- macOS + Windows + Linux/X11:

| Platform | Method                                                                                                                         |
| -------- | ------------------------------------------------------------------------------------------------------------------------------ |
| macOS    | Accessibility API (`AXUIElement`) to move/resize windows, `NSScreen` for display bounds. Tile Snap via NSEvent global monitor  |
| Windows  | Win32 API (`GetForegroundWindow`, `SetWindowPos`, `EnumDisplayMonitors`, `EnumWindows`) via `windows` crate v0.58              |
| Linux    | `x11rb` + EWMH (`_NET_ACTIVE_WINDOW`, `_NET_MOVERESIZE_WINDOW`, `_NET_CLIENT_LIST`, XRandr). X11 only — Wayland not supported. |

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
- `keep_awake: Mutex<Option<KeepAwake>>` -- sleep-prevention guard
- `is_dark_mode`, `is_muted` -- cached for tray-icon rendering
- `tiling_state` -- per-window saved rects for restore

---

## Data Flow

### User-initiated (slider drag)

```
Component ──invoke()──► Tauri IPC ──► Rust command ──spawn_blocking──► core::* ──► OS API
    │                                      │
    └── optimistic state update            └── Ok(()) flows back through IPC
```

### Backend-initiated (keyboard shortcut)

```
Keyboard shortcut
    │
    ▼
tray.rs::execute_command()  (background thread)
    │
    ├── core::display::set_all_brightness(value) ──► OS API
    │
    └── app.emit("monitors-changed")
              │
              ▼
        Frontend listener ──invoke("get_monitors")──► Rust ──spawn_blocking──► core::*
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
    │     ├── core::display::set_all_brightness(value)
    │     └── core::theme::set_dark_mode(true | false)
    │
    └── app.emit("monitors-changed") + app.emit("dark-mode-changed")
```

---

## Key Architectural Rules

1. **Platform code lives in `core::*`.** All display, dark-mode, volume, and wallpaper operations go through `core::*`. The per-OS split is done inside `core/` via `cfg`-gated `PlatformImpl`. `tiling/` is the other module with `#[cfg(target_os)]` (window tiling needs OS-specific window-server APIs).

2. **snake_case commands, camelCase parameters.** Tauri commands are `snake_case` in Rust and called with `snake_case` strings from the frontend. Parameter objects use `camelCase` keys -- serde converts automatically via `#[serde(rename_all = "camelCase")]`.

3. **Brightness has an absolute floor.** All brightness values are clamped to `effective_min_brightness()` which enforces `ABSOLUTE_MIN_BRIGHTNESS = 5`. The user-configured `minBrightness` in preferences can raise but never lower this floor.

4. **Contrast is DDC-only and optional.** `contrast` is `Option<u32>` / `number | null`. Built-in displays return `null`. The contrast slider is hidden by default and toggled via the `showContrast` preference.

5. **Monitor metadata is append-only.** `MonitorMetadata` entries in `preferences.monitorConfigs` are never removed when a monitor is unplugged. This preserves labels and sort order across plug/unplug cycles.

6. **Preferences use `#[serde(default)]`.** Old config files missing new fields gracefully fall back to defaults without breaking deserialization.

7. **Errors are strings, not crashes.** Backend functions return `Result<T, String>`. Frontend `invoke()` calls are wrapped in try/catch -- the UI silently keeps the last known state on error.

8. **Blocking work runs on a blocking thread.** DDC/CI, WMI, registry, and `gsettings` calls can block. Every Tauri command that calls into `core::*` wraps the call in `tauri::async_runtime::spawn_blocking` so the async runtime stays responsive.

9. **Tauri commands accessing AppState must be `async` on macOS.** Sync `#[tauri::command]` functions that take `State<'_, AppState>` block the macOS main-thread run-loop, preventing tray icon click events from firing. This was the root cause of a hard-to-diagnose bug where both left-click and right-click on the tray icon stopped working. See `config.rs` `save_preferences` for the documented warning.

10. **Do not use `write_debug_log()` in frequently-called sync commands.** `write_debug_log()` locks `state.preferences` to check the `debug_logging` flag. In sync Tauri commands called on every frontend render (like `get_preferences`), this mutex contention starves the macOS run-loop and breaks tray icon events. Use `log::info!` in those paths. `write_debug_log()` is safe in async or infrequently-called commands.

---

## Where to Edit

Quick reference for common tasks:

| Task                            | Files to change                                                                                                                                                  |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| New brightness/contrast feature | `core::display` and the relevant `core::{macos,windows,linux}::Platform` impl, then `display.rs` (Tauri cmd), `MonitorControl.tsx`/`AllMonitorsControl.tsx` (UI) |
| New dark mode behavior          | `core::theme` (per-OS branches), `dark_mode.rs` (Tauri cmd), `DarkModeToggle.tsx` (UI)                                                                           |
| New volume behavior             | `core::volume` (per-OS branches), `volume.rs` (Tauri cmd), `VolumeControl.tsx` (UI)                                                                              |
| New wallpaper behavior          | `core::wallpaper` (per-OS branches), `wallpaper.rs` (Tauri cmd + caching), `SettingsPanel.tsx` if user-configurable                                              |
| New preference field            | `config.rs` (add to `Preferences` struct + default), `types.ts` (TS interface), `SettingsPanel.tsx` (UI)                                                         |
| New Tauri command               | Domain module (Rust), `lib.rs` (register in `invoke_handler`), frontend component                                                                                |
| New keyboard shortcut command   | `tray.rs` (`execute_command` match arm), `config.rs` (default keybinding)                                                                                        |
| New UI component                | `src/components/NewComponent.tsx` + `NewComponent.test.tsx`, wire into `App.tsx`                                                                                 |
| Tray menu change                | `tray.rs` (`build_tray_menu`)                                                                                                                                    |
| Window tiling                   | `tiling/mod.rs` (shared layout math), `tiling/{macos,windows,linux}.rs` (per-OS), `tray.rs` (Tiling submenu), `config.rs` (`TilingPreferences`)                  |
| Window positioning              | `tray.rs` (`position_window_near_tray`) -- read the doc comment first!                                                                                           |
| Night mode schedule logic       | `lib.rs` (`check_night_mode_schedule`, `is_night_time`)                                                                                                          |
| CI changes                      | `.github/workflows/build.yml` or `release-official.yml`                                                                                                          |

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
    // For blocking platform calls, wrap in spawn_blocking:
    tauri::async_runtime::spawn_blocking(move || {
        core::display::set_all_brightness(50)
    })
    .await
    .map_err(|e| e.to_string())??;
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

Each monitor is identified by a composite UID: `{api_id}::{api_model_name}` (e.g. `"1::Dell U2723QE"`, `"builtin::Built-in Display"`). This is more stable than the raw integer ID from `core::DisplayInfo`, which can collide when monitors are swapped.

- `Monitor.id` -- raw `core::DisplayInfo.id`, used for `core::display::set_one_*` calls
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

| Action             | Values                                               | Effect                                             |
| ------------------ | ---------------------------------------------------- | -------------------------------------------------- |
| `changeBrightness` | 0-100                                                | Sets all monitors' brightness                      |
| `changeContrast`   | 0-100                                                | Sets all monitors' contrast                        |
| `changeDarkMode`   | `toggle`, `dark`, `light`                            | Toggles or sets dark mode                          |
| `changeVolume`     | 0-100                                                | Sets system volume                                 |
| `changeProfile`    | Profile index (0, 1, 2, ...)                         | Applies a saved profile                            |
| `tile`             | Layout name (e.g. `leftHalf`, `maximize`, `restore`) | Tiles focused window (macOS + Windows + Linux/X11) |

### Monitor metadata (monitorConfigs)

| Field       | Type   | Purpose                                              |
| ----------- | ------ | ---------------------------------------------------- |
| `uid`       | string | Composite key: `"{api_id}::{api_model_name}"`        |
| `apiId`     | string | Raw `core::DisplayInfo.id` (e.g. `"1"`, `"builtin"`) |
| `apiName`   | string | Model name from `core::DisplayInfo`                  |
| `label`     | string | User-set name (empty = use apiName)                  |
| `sortOrder` | number | Display order in UI (lower = higher)                 |
| `hidden`    | bool   | Whether the monitor is hidden from main UI           |

---

## App Versioning

The app version flows from a single source through the build pipeline to the UI:

```
tauri.conf.json ("version": "7.0.0")
       │
       ▼
build.rs reads it at compile time
       │
       ▼
cargo:rustc-env=APP_VERSION=7.0.0
       │
       ▼
config.rs: get_app_version() → env!("APP_VERSION")
       │
       ▼
App.tsx: invoke("get_app_version") → setVersion()
       │
       ▼
Header.tsx: "Display DJ v7.0.0"
```

- `tauri.conf.json` → `"version"`: The single source of truth. Controls both the UI header and installer/bundle metadata.
- `package.json` → `"version"`: `0.0.0` — not used (not published to npm).
- `Cargo.toml` → `version`: `0.0.0` — not used (crate not published).
- Release versioning is driven by git tags (`v*` triggers `release-official.yml`).

---

## Known Limitations

| Limitation                    | Details                                                                                                                                      |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| DDC/CI not universal          | Budget monitors and some HDMI connections may not support it                                                                                 |
| Built-in HDMI on base M1/M2   | No DDC/CI support. Use USB-C/DisplayPort                                                                                                     |
| Global shortcuts on Wayland   | Wayland restricts global hotkey capture. X11 works fine                                                                                      |
| Tray clicks dead on macOS     | Sync Tauri commands or `write_debug_log()` in hot sync commands starve the run-loop. See rules 9-10 above                                    |
| Tiling requires Accessibility | macOS tiling needs Accessibility permission. Without it, tile commands silently do nothing. Windows needs no special permissions             |
| Tile Snap macOS-only          | Mouse edge snapping (Tile Snap) is only implemented for macOS. Keyboard shortcuts and tray menu tiling work on macOS, Windows, and Linux/X11 |
| Tiling Wayland-only sessions  | Window tiling on Linux requires X11 (`$DISPLAY` set). Wayland is not supported.                                                              |
| Tray left-click on Linux      | AppIndicator doesn't always fire left-click                                                                                                  |
| Dark mode on non-GNOME        | `gsettings` is GNOME-specific. KDE, XFCE not supported                                                                                       |

---

## Troubleshooting

| Problem                                 | Fix                                                                                                                                                                                      |
| --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `command not found: rustc`              | Run `source "$HOME/.cargo/env"` or reopen terminal                                                                                                                                       |
| First build takes 5+ minutes            | Normal. Rust compiles from source. Subsequent runs are ~5-15s                                                                                                                            |
| App launched but can't find it          | System tray app. macOS: menu bar top-right. Windows: system tray bottom-right. Linux: top panel                                                                                          |
| "No displays found"                     | `core::PlatformImpl::detect_displays()` returned empty. macOS: check Accessibility for tiling. Windows: confirm DDC/CI in OSD. Linux: `ddcutil detect` and verify `i2c` group membership |
| Dark mode toggle does nothing (Linux)   | Requires GNOME. Check `echo $XDG_CURRENT_DESKTOP`                                                                                                                                        |
| macOS "System Events" permission prompt | Expected on first launch. Volume control uses `osascript` which requires System Events access. Click Allow — only prompts once                                                           |
