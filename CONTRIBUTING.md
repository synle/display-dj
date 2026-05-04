# Contributing to Display DJ

---

## Table of Contents

- [Quick Start](#quick-start)
- [Architecture Overview](#architecture-overview)
- [Project Structure](#project-structure)
- [Code Paths by Feature](#code-paths-by-feature)
- [How the Frontend and Backend Communicate](#how-the-frontend-and-backend-communicate)
- [Frontend State Management](#frontend-state-management)
- [Configuration Files](#configuration-files)
- [Conventions](#conventions)
- [Testing](#testing)
- [CI/CD Pipelines](#cicd-pipelines)
- [Adding a New Tauri Command](#adding-a-new-tauri-command)
- [Day-to-day Commands](#day-to-day-commands)
- [Building for Production](#building-for-production)
- [Platform Setup Guides](#platform-setup-guides)
  - [macOS](#macos-setup)
  - [Windows](#windows-setup)
  - [Linux](#linux-ubuntudebian-setup)
- [Vendored platform core](#vendored-platform-core-src-tauri-src-core)
- [Known Limitations](#known-limitations)
- [Troubleshooting](#troubleshooting)

---

## Quick Start

**Prerequisites**: [Node.js](https://nodejs.org) 18+, [Rust](https://www.rust-lang.org/tools/install) 1.77+. No external binaries — the platform code is compiled into the app.

```bash
git clone <repo-url>
cd display-dj2
npm install
npx tauri dev
```

The first Rust compilation takes 2-10 minutes. Subsequent runs are incremental (~5-15s).

Display DJ is a **system tray app** -- it does NOT open a regular window. Look for its icon in the menu bar (macOS), system tray (Windows), or top panel (Linux).

---

## Architecture Overview

Display DJ is built with [Tauri v2](https://v2.tauri.app/), which pairs a **Rust** backend with a **web-based** frontend. As of v7.0.0, all platform code is vendored into the Rust backend at `src-tauri/src/core/` and runs in-process — there is no separate helper process or HTTP server.

```
┌──────────────────────────────────────────────┐
│  Frontend (React 19 + TypeScript + Vite 6)   │
│  Runs inside a WebView (WebKit on macOS/     │
│  Linux, WebView2 on Windows)                 │
│                                              │
│  Communicates via invoke() and listen()      │
└──────────────────┬───────────────────────────┘
                   │  Tauri IPC bridge
┌──────────────────▼───────────────────────────┐
│  Backend (Rust + Tauri v2)                   │
│                                              │
│  #[tauri::command] functions in              │
│  display.rs / dark_mode.rs / volume.rs /     │
│  wallpaper.rs are thin wrappers that call    │
│  spawn_blocking → core::* in-process.        │
│                                              │
│  Also: preferences/config persistence,       │
│  system tray, global keyboard shortcuts,     │
│  night mode scheduler, window tiling.        │
└──────────────────┬───────────────────────────┘
                   │  direct function calls
┌──────────────────▼───────────────────────────┐
│  src-tauri/src/core/  (vendored platform)    │
│                                              │
│  - core::macos / core::windows / core::linux │
│    DDC/CI, gamma, DisplayServices, WMI,      │
│    brightnessctl/ddcutil. PlatformImpl is    │
│    a cfg-gated alias to the right impl.      │
│  - core::theme   — system dark mode          │
│  - core::volume  — system volume             │
│  - core::wallpaper — wallpaper + slideshow   │
│  - core::display — high-level brightness/    │
│    contrast helpers fanning out to displays  │
└──────────────────────────────────────────────┘
```

The frontend calls Rust functions using `invoke("command_name", { params })`. The Rust backend executes the work in-process by calling `core::*`. The backend can push events to the frontend using `app.emit("event-name", payload)`.

---

## Project Structure

```
display-dj2/
├── src/                          # Frontend (React 18 + TypeScript)
│   ├── main.tsx                  # Entry point -- mounts React into <div id="root">
│   ├── App.tsx                   # Root component: fetches data, manages state, renders layout
│   ├── App.test.tsx              # Smoke test: App renders, fetches data, handles errors
│   ├── App.css                   # All CSS (dark tray popup theme, sliders, toggles)
│   ├── types.ts                  # Shared TypeScript interfaces (Monitor, MonitorMetadata, Preferences, KeyBinding, NightModeSchedule)
│   ├── types.d.ts                # Global type definitions (Command, DisplayType, BrightnessPreset, etc.)
│   ├── index.d.ts                # Ambient module declarations (SVG/image imports, legacy adapters)
│   ├── constants.ts              # Shared constants (LAPTOP_BUILT_IN_DISPLAY_ID)
│   ├── test/
│   │   └── setup.ts              # Vitest setup: jsdom, jest-dom matchers, Tauri API mocks
│   └── components/
│       ├── Header.tsx            # "Display DJ v{version}" + expand/collapse chevron
│       ├── Header.test.tsx
│       ├── Slider.tsx            # Reusable range slider with icon, debounced onChange
│       ├── Slider.test.tsx
│       ├── AllMonitorsControl.tsx # Collapsed view: single brightness slider for all monitors
│       ├── AllMonitorsControl.test.tsx
│       ├── MonitorControl.tsx    # Expanded view: per-monitor brightness + editable name
│       ├── MonitorControl.test.tsx
│       ├── VolumeControl.tsx     # Volume slider
│       ├── VolumeControl.test.tsx
│       ├── DarkModeToggle.tsx    # Dark / Light toggle buttons
│       ├── DarkModeToggle.test.tsx
│       ├── SettingsPanel.tsx     # In-app settings: min brightness, show contrast, night mode schedule
│       └── ProfileButtons.tsx   # Profile quick-action buttons with overflow menu
│
├── src-tauri/                    # Backend (Rust + Tauri v2)
│   ├── Cargo.toml                # Rust dependencies (tauri, ddc, ddc-macos/winapi, windows, x11rb, serde, etc.)
│   ├── build.rs                  # tauri_build::build() + expose_app_version() for APP_VERSION/BUILD_DATE
│   ├── tauri.conf.json           # App config: window size, tray icon, bundling options
│   ├── capabilities/
│   │   └── default.json          # Security permissions (what frontend JS can access)
│   ├── icons/                    # App icons for all platforms (.icns, .ico, .png)
│   ├── tests/
│   │   └── smoke.rs              # Integration smoke test: crate links, public API usable
│   └── src/
│       ├── main.rs               # Binary entry point (calls lib::run)
│       ├── lib.rs                # Tauri app setup: plugins, state, tray, shortcuts, night mode schedule checker
│       ├── core/                 # Vendored platform code (in-process; no HTTP, no sidecar)
│       │   ├── mod.rs            # Shared types: DisplayInfo, DisplayControl, Platform; PlatformImpl alias
│       │   ├── macos.rs          # macOS DDC/CI + DisplayServices (built-in)
│       │   ├── windows.rs        # Windows WMI + DDC/CI
│       │   ├── linux.rs          # Linux ddcutil + brightnessctl
│       │   ├── theme.rs          # Cross-platform dark mode get/set
│       │   ├── volume.rs         # Cross-platform volume get/set
│       │   ├── wallpaper.rs      # Wallpaper set + slideshow timer/state/cycling
│       │   └── display.rs        # set_all_brightness / set_one_brightness + contrast variants
│       ├── display.rs            # Tauri-command wrappers around core::display (+ unit tests). Uses spawn_blocking
│       ├── dark_mode.rs          # Tauri-command wrappers around core::theme
│       ├── volume.rs             # Tauri-command wrappers around core::volume
│       ├── wallpaper.rs          # Tauri-command wrappers around core::wallpaper (incl. remote-pack download via reqwest)
│       ├── config.rs             # Preferences + monitor metadata persistence, NightModeSchedule, min brightness, reset to defaults (+ unit tests)
│       ├── tray.rs               # System tray menu, window positioning, keyboard shortcut dispatch (in-process command execution)
│       └── tray_icon.rs          # Programmatic tray icon generation (128x128, percentage-based layout, state indicators)
│
├── index.html                    # HTML shell that loads src/main.tsx
├── package.json                  # Node deps (react, tauri API, vite, typescript)
├── vite.config.ts                # Vite config (dev server on port 1420, test config)
├── tsconfig.json                 # TypeScript config (ES2021, react-jsx)
├── CLAUDE.md                     # Conventions for AI-assisted development
└── .github/workflows/
    ├── build.yml                 # CI: tests + build on PRs (macOS ARM, macOS Intel, Windows, Linux)
    └── release.yml               # CD: creates GitHub releases on v* tags
```

---

## Code Paths by Feature

This section maps each user-facing feature to the exact files and functions that implement it. Use this when debugging or extending a feature.

### Monitor Brightness

**Frontend flow**: `App.tsx` calls `invoke("get_monitors")` on mount and on `monitors-changed` events. Slider changes call `invoke("set_brightness", { monitorId, value })`. The `Slider.tsx` component debounces changes by 150ms to avoid flooding the backend. In collapsed view, `AllMonitorsControl.tsx` sets all monitors at once via `set_all_brightness`.

**Backend** (`display.rs` → `core::display` → `core::PlatformImpl`): The Tauri command (`async fn`) wraps the call in `tauri::async_runtime::spawn_blocking` and invokes the in-process `core::*` function. The `core::PlatformImpl` type alias resolves to `core::macos::Platform`, `core::windows::Platform`, or `core::linux::Platform` at compile time.

| Operation                   | In-process call                                | Returns                                              |
| --------------------------- | ---------------------------------------------- | ---------------------------------------------------- |
| Detect monitors             | `PlatformImpl::detect_displays()`              | `Vec<DisplayInfo>` with live brightness and contrast |
| Set one monitor brightness  | `core::display::set_one_brightness(id, level)` | `Result<(), String>`                                 |
| Set all monitors brightness | `core::display::set_all_brightness(level)`     | per-display result                                   |
| Set one monitor contrast    | `core::display::set_one_contrast(id, level)`   | `Result<(), String>` (DDC-only)                      |
| Set all monitors contrast   | `core::display::set_all_contrast(level)`       | per-display result (DDC-only)                        |

`core::DisplayInfo` (with `display_type`, nullable `brightness`, nullable `contrast`) is converted into the app's `Monitor` struct (with `is_built_in`, `supports_brightness`, `contrast`) inside `display.rs`.

### Monitor Contrast

**Frontend flow**: Contrast sliders are shown alongside brightness sliders only when `showContrast` is enabled in preferences and the monitor supports DDC contrast (`contrast !== null`). Built-in displays never show contrast. The collapsed `AllMonitorsControl` shows an average contrast slider across all contrast-capable monitors.

**Backend** (`display.rs` → `core::display`): Contrast uses the same in-process pattern as brightness. Contrast values are 0-100, not subject to `min_brightness` clamping.

| Operation                 | In-process call                              |
| ------------------------- | -------------------------------------------- |
| Set one monitor contrast  | `core::display::set_one_contrast(id, level)` |
| Set all monitors contrast | `core::display::set_all_contrast(level)`     |

**Key functions**: `set_monitor_contrast()`, `set_all_monitors_contrast()`, `set_contrast` (Tauri command), `set_all_contrast` (Tauri command).

**Key functions to know**:

- `detect_monitors()` -- calls `PlatformImpl::detect_displays()`, converts `core::DisplayInfo` -> `Monitor`
- `merge_with_configs()` -- applies user-configured labels and sort orders from `preferences.monitorConfigs`

### Dark Mode

**Frontend**: `App.tsx` calls `invoke("get_dark_mode")` and `invoke("set_dark_mode", { enabled })`. `DarkModeToggle.tsx` renders two buttons.

**Backend** (`dark_mode.rs` → `core::theme`): Calls in-process functions. Implementation under `core::theme` uses `defaults`/AppleScript on macOS, registry writes on Windows, and `gsettings` on Linux/GNOME.

| Operation         | In-process call                     |
| ----------------- | ----------------------------------- |
| Get current theme | `core::theme::get_dark_mode()`      |
| Set dark mode     | `core::theme::set_dark_mode(true)`  |
| Set light mode    | `core::theme::set_dark_mode(false)` |

### Volume

**Frontend**: `App.tsx` calls `invoke("get_volume")` and `invoke("set_volume", { value })`. `VolumeControl.tsx` wraps `Slider.tsx` with muted/unmuted icon logic.

**Backend** (`volume.rs` → `core::volume`): Like all the other display ops, this is just an in-process call now. Per-OS implementations:

| Platform | Method                                                                                           |
| -------- | ------------------------------------------------------------------------------------------------ |
| macOS    | `osascript` for CoreAudio: `output volume of (get volume settings)` / `set volume output volume` |
| Windows  | PowerShell with inline C# `Add-Type` for WASAPI COM (`IAudioEndpointVolume`)                     |
| Linux    | `pactl get-sink-volume @DEFAULT_SINK@` / `pactl set-sink-volume @DEFAULT_SINK@ <val>%`           |

### Night Mode Schedule

**Backend** (`lib.rs`): A background thread runs `check_night_mode_schedule()` every 60 seconds. It reads the `NightModeSchedule` from preferences and, if enabled, compares the current time against `nightStart` / `dayStart` (both "HH:MM" 24-hour format). During the night window it sets all monitors to `nightBrightness` and switches to dark mode; during the day window it sets `dayBrightness` and light mode. Brightness values are clamped to `effective_min_brightness()`.

**Frontend**: `SettingsPanel.tsx` provides UI for enabling/disabling the schedule, setting night/day start times, and night/day brightness levels. Changes are saved via `invoke("save_preferences", { preferences })`.

Helper functions in `lib.rs`:

- `parse_time_minutes()` — converts "HH:MM" to minutes since midnight
- `is_night_time()` — determines if current time falls in the night window (handles midnight wraparound)
- `check_night_mode_schedule()` — the main scheduler that applies brightness + dark mode changes

### Settings Panel

**Frontend** (`SettingsPanel.tsx`): In-app settings UI accessed from the header. Allows editing:

- Min brightness (5–100) — floor for all brightness operations
- Show Contrast Slider — toggles contrast controls in the UI (DDC-only monitors)
- Monitor order, labels, and visibility — per-monitor configuration
- Night mode schedule — enable/disable, set times and brightness levels
- Launch at Login — OS autostart

Loads preferences via `invoke("get_preferences")`, saves via `invoke("save_preferences")`. Values are clamped to slider ranges on load.

### System Tray & Keyboard Shortcuts

**Backend** (`tray.rs`):

- `setup_tray()` -- creates the tray icon + context menu (Dark Mode, Light Mode, Profiles, Tiling, Exposé, Debug submenu with Reset to Default, Quit)
- Left-click toggles main window visibility; `position_window_near_tray()` places it near the tray icon
- Right-click opens the context menu
- `register_shortcuts()` -- reads key bindings from `Preferences`, registers via `tauri-plugin-global-shortcut`
- `execute_command()` -- dispatches command strings like `"command/changeBrightness/50"` in-process by calling the matching `core::*` function on a background thread, then emits events to the frontend so it can refresh

### Config Persistence

**Backend** (`config.rs`):

- `config_dir()` resolves to `~/Library/Application Support/display-dj` (macOS), `%APPDATA%/display-dj` (Windows), `~/.config/display-dj` (Linux)
- `load_preferences()` / `save_preferences_to_disk()` -- reads/writes `preferences.json` (includes per-monitor metadata), returns `Preferences::default()` if file missing or malformed
- Monitor metadata (labels, sort order) is stored inline in `preferences.json` as `monitorConfigs` array — no separate file

---

## How the Frontend and Backend Communicate

**Frontend -> Backend** (calling Rust functions):

```typescript
import { invoke } from '@tauri-apps/api/core';

const monitors = await invoke<Monitor[]>('get_monitors');
await invoke('set_brightness', { monitorId: 'external-1', value: 75 });
```

- Command names are **snake_case** strings matching the Rust function name
- Parameters are passed as a single object with **camelCase** keys (Serde converts them)

**Backend -> platform code** (in-process function calls — no HTTP):

```rust
// inside an async Tauri command:
tauri::async_runtime::spawn_blocking(move || {
    core::display::set_all_brightness(value)
}).await.map_err(|e| e.to_string())??;
```

**Backend -> Frontend** (pushing events):

```rust
use tauri::Emitter;
app.emit("monitors-changed", ())?;
```

```typescript
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen('monitors-changed', () => {
  // Refetch monitor data
});
```

Events are used when keyboard shortcuts change brightness/volume/dark mode from the backend, so the frontend can update its UI.

**All registered commands** (defined in `lib.rs` `invoke_handler`):

| Module       | Commands                                                                                                                                                     |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `display`    | `get_monitors`, `set_brightness`, `set_all_brightness`, `set_contrast`, `set_all_contrast`, `rename_monitor`, `save_monitor_order`, `set_monitor_visibility` |
| `dark_mode`  | `get_dark_mode`, `set_dark_mode`                                                                                                                             |
| `volume`     | `get_volume`, `set_volume`                                                                                                                                   |
| `config`     | `get_preferences`, `save_preferences`, `open_preferences_file`, `open_debug_log`, `open_app_folder`, `get_app_version`                                       |
| `keep_awake` | `get_keep_awake`, `set_keep_awake`                                                                                                                           |
| `tray`       | `apply_profile`                                                                                                                                              |

**Events emitted by backend**:

| Event               | Emitted when                         |
| ------------------- | ------------------------------------ |
| `monitors-changed`  | Keyboard shortcut changes brightness |
| `dark-mode-changed` | Keyboard shortcut toggles dark mode  |
| `volume-changed`    | Keyboard shortcut changes volume     |

---

## Frontend State Management

`App.tsx` is the single source of truth for all UI state. There is no external state library.

**State variables**:

- `monitors: Monitor[]` -- current monitor list with brightness and contrast values
- `darkMode: boolean` -- system dark mode state
- `volume: number` -- system volume (0-100)
- `minBrightness: number` -- effective minimum brightness floor
- `showContrast: boolean` -- whether contrast sliders are visible (from preferences)
- `profiles: Profile[]` -- saved profiles from preferences
- `expanded: boolean` -- collapsed (all-monitors) vs expanded (individual) view
- `version: string` -- app version from `tauri.conf.json` (via `build.rs` compile-time env var `APP_VERSION`). Dev builds append `[beta - <short_sha>]`; release builds show clean version only

**Data flow**:

1. On mount, `useEffect` calls `invoke()` for each data source (`get_monitors`, `get_dark_mode`, `get_volume`, `get_app_version`)
2. Event listeners (`listen("monitors-changed", ...)`) trigger refetches when the backend changes state via keyboard shortcuts
3. `visibilitychange` listener refetches all data when the tray popup becomes visible (catches external changes)
4. Slider/toggle handlers call `invoke()` to push changes to the backend, then optimistically update local state

**Average calculations** (collapsed view):

- `avgBrightness` = mean of all visible monitors' brightness
- `avgContrast` = mean of visible monitors that support contrast (`contrast !== null`); `null` if none support it

**Settings panel**: Opened from the header, `SettingsPanel.tsx` manages its own local state loaded from `get_preferences`. On save, it writes back to the backend and triggers a refresh in `App.tsx` via the `onPreferencesSaved` callback.

---

## Configuration Files

All config files live in the platform-specific config directory (`config_dir()` in `config.rs`).

### preferences.json

Deserialized into the `Preferences` struct. If the file is missing or malformed, `Preferences::default()` is used and written to disk.

| Field                    | Type   | Default               | Purpose                                             |
| ------------------------ | ------ | --------------------- | --------------------------------------------------- |
| `showIndividualDisplays` | bool   | `false`               | Start in expanded view                              |
| `brightnessDelta`        | number | `10`                  | Step size for keyboard shortcut brightness changes  |
| `contrastDelta`          | number | `10`                  | Step size for contrast changes                      |
| `minBrightness`          | number | `10`                  | Minimum brightness floor (absolute floor: 5)        |
| `showContrast`           | bool   | `false`               | Show contrast sliders in the UI (DDC-only monitors) |
| `nightModeSchedule`      | object | disabled, 21:00–07:00 | Auto brightness + dark mode by time of day          |
| `keyBindings`            | array  | 9 default bindings    | Global keyboard shortcuts                           |

**Night mode schedule fields**:

| Field             | Type   | Default   | Purpose                                   |
| ----------------- | ------ | --------- | ----------------------------------------- |
| `enabled`         | bool   | `false`   | Whether the schedule is active            |
| `nightStart`      | string | `"21:00"` | Time to switch to night mode (HH:MM, 24h) |
| `nightBrightness` | number | `20`      | Brightness during night window            |
| `dayStart`        | string | `"07:00"` | Time to switch to day mode (HH:MM, 24h)   |
| `dayBrightness`   | number | `100`     | Brightness during day window              |

Each key binding has a `key` (e.g. `"Shift+F1"`) and a `command` -- either a single string (`"command/changeBrightness/50"`) or an array of strings for multi-action shortcuts.

**Supported command format**: `command/<action>/<value>`

| Action             | Values                                          | Effect                                 |
| ------------------ | ----------------------------------------------- | -------------------------------------- |
| `changeBrightness` | Any integer 0-100 (e.g. `0`, `10`, `50`, `100`) | Sets all monitors' brightness          |
| `changeContrast`   | Any integer 0-100 (e.g. `0`, `50`, `100`)       | Sets all monitors' contrast (DDC-only) |
| `changeDarkMode`   | `toggle`, `dark`, `light`                       | Toggles or sets system dark mode       |
| `changeVolume`     | Any integer 0-100 (e.g. `0`, `50`, `100`)       | Sets system volume                     |
| `changeProfile`    | Profile index (e.g. `0`, `1`, `2`)              | Applies a saved profile by index       |

### Monitor Metadata (in preferences.json)

The `monitorConfigs` array in `preferences.json` stores per-monitor metadata. Each monitor is identified by a composite UID (`{api_id}::{api_model_name}`) that survives reconnections. Entries are never removed when a monitor is unplugged — labels and sort order persist across plug/unplug cycles.

| Field       | Type   | Purpose                                                     |
| ----------- | ------ | ----------------------------------------------------------- |
| `uid`       | string | Composite unique key: `"{api_id}::{api_model_name}"`        |
| `apiId`     | string | Raw ID from `core::DisplayInfo` (e.g. `"1"`, `"builtin"`)   |
| `apiName`   | string | Model name from `core::DisplayInfo` (e.g. `"Dell U2723QE"`) |
| `label`     | string | User-set friendly name (empty string = use apiName)         |
| `sortOrder` | number | Display order in the UI (lower = higher)                    |
| `hidden`    | bool   | Whether the monitor is hidden from the main UI              |

---

## Conventions

### Naming

- **Rust structs** sent to the frontend use `#[serde(rename_all = "camelCase")]`. Fields are `snake_case` in Rust, `camelCase` in JSON/TypeScript.
- **Tauri commands** are `snake_case` in Rust and called with `snake_case` strings from the frontend (`invoke("get_monitors")`).
- **Frontend parameters** are passed as `camelCase` objects -- Serde handles the conversion automatically.
- The `CommandValue` enum uses `#[serde(untagged)]` so keybinding commands can be either `"string"` or `["array"]` in JSON.

### Display operations

- All display, dark-mode, volume, and wallpaper operations live in `src-tauri/src/core/`. Each Tauri-command file (`display.rs`, `dark_mode.rs`, `volume.rs`, `wallpaper.rs`) is a thin async wrapper that calls into `core::*` via `spawn_blocking`.
- Platform splits use `#[cfg(target_os = "...")]` and the `core::PlatformImpl` type alias resolves to the right implementation at compile time.
- `Preferences` and `NightModeSchedule` use `#[serde(default)]` so old config files missing new fields gracefully fall back to defaults without breaking.
- Brightness values are clamped to `effective_min_brightness()` (which enforces `ABSOLUTE_MIN_BRIGHTNESS = 5`) before being sent to displays.

### Error handling

- Backend functions return `Result<T, String>`. Errors are human-readable strings.
- Frontend `invoke()` calls are wrapped in try/catch with `console.error` logging. The UI does not crash on backend errors -- it silently keeps the last known state.

### State

- `AppState` (in `lib.rs`) holds `Mutex<Preferences>` for thread-safe shared access across Tauri commands. Monitor metadata (labels, sort order) is stored within `Preferences.monitor_configs`.
- Frontend state lives entirely in `App.tsx` -- no external state library.

---

## Testing

### Running Tests

```bash
npm test                       # Run all frontend tests (once, ~1 second)
npm run test:watch             # Run frontend tests in watch mode
cd src-tauri && cargo test     # Run all Rust backend tests
```

### Frontend Tests (Vitest + React Testing Library)

Test files live next to the components they test (`*.test.tsx`).

| File                                         | What it covers                                                                                                             |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `src/components/Header.test.tsx`             | Title rendering, version display, expand/collapse toggle, chevron state                                                    |
| `src/components/Slider.test.tsx`             | Range input rendering, debounced onChange (150ms), fill width calculation, prop updates                                    |
| `src/components/DarkModeToggle.test.tsx`     | Active button state, click handlers for dark/light                                                                         |
| `src/components/VolumeControl.test.tsx`      | Slider value, muted/unmuted icon switching                                                                                 |
| `src/components/AllMonitorsControl.test.tsx` | Brightness slider, average brightness calculation                                                                          |
| `src/components/MonitorControl.test.tsx`     | Monitor name rendering, inline edit mode (Enter/Escape), brightness slider, built-in vs external icons                     |
| `src/App.test.tsx`                           | **Smoke test**: App mounts without crashing, fetches initial data, renders all sections, handles backend errors gracefully |

**Tauri API mocking**: `src/test/setup.ts` globally mocks `invoke()` and `listen()` from `@tauri-apps/api` so tests run without a Tauri backend. The App smoke test provides per-command mock responses.

### Backend Tests (Rust)

Inline `#[cfg(test)]` modules plus an integration smoke test.

| Location         | What it covers                                                                                                                                                                                                                                                                                      |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `config.rs`      | `Preferences` defaults, `MonitorMetadata` serde, `CommandValue` Single/Multiple serde, camelCase JSON fields, file roundtrips, malformed JSON fallback, `get_app_version`, effective min brightness, backward-compatible deserialization of old configs, preferences with monitor configs roundtrip |
| `display.rs`     | `DjDisplay` to `Monitor` conversion (builtin, external DDC, null brightness, uid computation), `Monitor` serde (camelCase, roundtrip), `merge_with_configs` (rename, sort, empty label), `reconcile_migrated_configs`, `ensure_metadata_for_monitors`                                               |
| `keep_awake.rs`  | KeepAwake guard creation, Mutex<Option<KeepAwake>> pattern (enable/disable/re-enable)                                                                                                                                                                                                               |
| `tray_icon.rs`   | Percentage-to-pixel conversion, icon generation for all state combinations (dark/light, keep-awake, muted), filled rect and thick line drawing                                                                                                                                                      |
| `tests/smoke.rs` | **Smoke test**: crate compiles and links, `AppState` is constructable, `run` function is exported                                                                                                                                                                                                   |

Rust tests don't require external tools or hardware -- they test pure logic that works on all platforms.

### Adding New Tests

**Frontend**: Create `ComponentName.test.tsx` next to the component. Import from `@testing-library/react` and `vitest`. Tauri mocks are available globally.

**Backend**: Add a `#[cfg(test)] mod tests { ... }` block at the bottom of the source file. For integration tests, add files to `src-tauri/tests/`.

---

## CI/CD Pipelines

### build.yml (PR validation)

Triggers on pushes and PRs to `main` and `v2` branches. Runs on four matrix targets:

- `macos-latest` (ARM64), `macos-13` (Intel x64), `windows-latest`, `ubuntu-22.04`

Steps: checkout -> Node 20 -> Rust stable -> Linux deps (Ubuntu only) -> `npm install` -> `npm test` -> `npm run build` -> `cargo test` -> `cargo check`

### release.yml (production releases)

Triggers on `v*` tags or manual `workflow_dispatch`. Deletes any existing release/tag first, then builds platform installers (`.dmg`, `.exe`, `.deb`, `.AppImage` — no `.tar.gz`/`.msi`/`.rpm`). Release notes are auto-generated from commit history (top 10 commits since last tag with full diff link). Sets `TAURI_RELEASE=true` so the version header shows clean version without `[beta]` suffix.

```bash
git tag v5.6.0
git push origin v5.6.0
```

---

## Adding a New Tauri Command

1. **Define the Rust function** in the appropriate module:

```rust
#[tauri::command]
pub fn my_new_command(some_param: String) -> Result<String, String> {
    Ok(format!("Hello {}", some_param))
}
```

2. **Register it** in `lib.rs` inside `invoke_handler`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    display::my_new_command,
])
```

3. **Call it from the frontend**:

```typescript
const result = await invoke<string>('my_new_command', { someParam: 'world' });
```

Note: Rust `some_param` maps to TypeScript `someParam` via serde's camelCase renaming.

4. **Add tests**: Unit test in the Rust module's `#[cfg(test)]` block, frontend test if it affects the UI.

---

## Day-to-day Commands

| Command                       | What it does                                                                          |
| ----------------------------- | ------------------------------------------------------------------------------------- |
| `npx tauri dev`               | Full app in dev mode. Frontend hot-reloads. Rust recompiles incrementally (~5-15s).   |
| `npx tauri build`             | Production build: optimized binary + platform installer.                              |
| `npm run build`               | Frontend only (TypeScript check + Vite bundle). Quick way to catch frontend errors.   |
| `npm run dev`                 | Vite dev server only (no Rust). Tauri API calls fail, but useful for CSS/layout work. |
| `npm test`                    | Frontend tests (Vitest). ~1 second.                                                   |
| `cd src-tauri && cargo test`  | Backend tests.                                                                        |
| `cd src-tauri && cargo check` | Check Rust compiles. Faster than a full build.                                        |

---

## Building for Production

```bash
npx tauri build
```

| Platform | Output                                                                                                                                         |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS    | `src-tauri/target/release/bundle/dmg/Display DJ_5.6.0_aarch64.dmg`                                                                             |
| Windows  | `src-tauri\target\release\bundle\nsis\Display DJ_5.6.0_x64-setup.exe`                                                                          |
| Linux    | `src-tauri/target/release/bundle/deb/display-dj_5.6.0_amd64.deb`<br>`src-tauri/target/release/bundle/appimage/display-dj_5.6.0_amd64.AppImage` |

---

## Platform Setup Guides

These guides walk through setting up a development environment from a fresh machine. If you already have Node.js 18+ and Rust 1.77+ installed, skip ahead to "Clone and run" for your platform.

### Prerequisites (All Platforms)

| Tool        | Minimum Version    | Purpose                             |
| ----------- | ------------------ | ----------------------------------- |
| **Git**     | Any recent version | Clone the repository                |
| **Node.js** | 18+ (CI uses 20)   | Build the React/TypeScript frontend |
| **Rust**    | 1.77+ (stable)     | Build the Tauri/Rust backend        |

---

### macOS Setup

#### Step 1: Install Xcode Command Line Tools

```bash
xcode-select --install
```

Click **Install** in the popup. This provides the C/C++ compiler, linker, and `git`.

#### Step 2: Install Homebrew

If you don't have it (`brew --version`):

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

On Apple Silicon, add to PATH as shown in the installer output:

```bash
echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
eval "$(/opt/homebrew/bin/brew shellenv)"
```

#### Step 3: Install Node.js and Rust

```bash
brew install node
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # press 1 for default
source "$HOME/.cargo/env"
```

Verify: `node --version` (18+), `rustc --version` (1.77+).

#### Step 4: Clone and run

```bash
git clone <repo-url>
cd display-dj2
npm install
npx tauri dev
```

---

### Windows Setup

#### Step 1: Install Git

Download from [git-scm.com](https://git-scm.com/download/win). Keep defaults.

#### Step 2: Install Node.js

Download the **LTS** installer from [nodejs.org](https://nodejs.org). During installation, **check "Automatically install the necessary tools"** -- this installs the C++ build tools Rust needs.

If you already have Node.js but not C++ build tools: install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload.

#### Step 3: Install Rust

```powershell
winget install Rustlang.Rustup
```

Or download from [rust-lang.org](https://www.rust-lang.org/tools/install). **Close and reopen** your terminal after installation.

Verify: `node --version` (18+), `rustc --version` (1.77+).

#### Step 4: Install WebView2 (Windows 10 only)

Windows 11 includes it. On Windows 10, download from [developer.microsoft.com](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).

#### Step 5: Clone and run

```powershell
git clone <repo-url>
cd display-dj2
npm install
npx tauri dev
```

---

### Linux (Ubuntu/Debian) Setup

#### Step 1: Install Git, build essentials, Node.js, Rust

```bash
sudo apt update
sudo apt install -y git build-essential curl wget file

# Node.js (via NodeSource)
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # press 1 for default
source "$HOME/.cargo/env"
```

Verify: `node --version` (18+), `rustc --version` (1.77+).

#### Step 2: Install Tauri system dependencies

```bash
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf \
  libxdo-dev \
  libssl-dev
```

#### Step 3: Install display control dependencies

```bash
# External monitors (DDC/CI) -- called directly by core::linux
sudo apt install -y ddcutil i2c-tools
sudo modprobe i2c-dev
echo "i2c-dev" | sudo tee /etc/modules-load.d/i2c-dev.conf
sudo usermod -aG i2c $USER    # log out and back in after this

# Built-in laptop display -- called directly by core::linux
sudo apt install -y brightnessctl
```

**You must log out and back in** after the `usermod` command. Verify with `ddcutil detect`.

Volume (`pactl`) and dark mode (`gsettings`) are pre-installed on Ubuntu/GNOME.

#### Step 4: Clone and run

```bash
git clone <repo-url>
cd display-dj2
npm install
npx tauri dev
```

---

## Vendored platform core (`src-tauri/src/core/`)

As of v7.0.0, all platform code that used to live in the [display-dj CLI](https://github.com/synle/display-dj-cli) is vendored directly into Display DJ at `src-tauri/src/core/`. There is no helper binary, no HTTP server, no port discovery, and no runtime dependency on the upstream CLI repo or its releases.

### Modules

| Module            | Responsibility                                                                                              |
| ----------------- | ----------------------------------------------------------------------------------------------------------- |
| `core::mod`       | Shared types: `DisplayInfo`, `DisplayControl` trait, `Platform` trait. `core::PlatformImpl` cfg-gated alias |
| `core::macos`     | macOS DDC/CI (via `ddc-macos`) + DisplayServices for built-in screens                                       |
| `core::windows`   | Windows DDC/CI (via `ddc-winapi`) + WMI for built-in screens                                                |
| `core::linux`     | Linux `ddcutil` for external + `brightnessctl` for built-in                                                 |
| `core::theme`     | System dark mode get/set (osascript/`defaults` on macOS, registry on Windows, `gsettings` on Linux)         |
| `core::volume`    | System volume get/set (osascript on macOS, PowerShell+WASAPI on Windows, `pactl` on Linux)                  |
| `core::wallpaper` | Wallpaper set + slideshow timer/state/cycling (NSWorkspace, `IDesktopWallpaper`, gsettings/feh)             |
| `core::display`   | High-level helpers (`set_all_brightness`, `set_one_brightness`, contrast variants) over `PlatformImpl`      |

### How the wrappers call core

`display.rs`, `dark_mode.rs`, `volume.rs`, `wallpaper.rs` are thin Tauri-command modules. Every command that takes `State<'_, AppState>` is `async fn` (see CLAUDE.md "macOS Tray Icon Pitfall"); CPU-bound work runs inside `tauri::async_runtime::spawn_blocking` so the macOS main-thread run-loop stays responsive.

### Relationship to display-dj-cli

The standalone [display-dj CLI](https://github.com/synle/display-dj-cli) (local checkout at `/Users/syle/git/display-dj-cli`) is the **upstream** of this code, not a runtime dependency. When fixing a bug in `core::*`, consider whether the same fix should be cross-applied upstream so the standalone CLI stays in sync. Display DJ releases do **not** download anything from that repo.

---

## Known Limitations

| Limitation                  | Details                                                                                    |
| --------------------------- | ------------------------------------------------------------------------------------------ |
| DDC/CI not universal        | Not every monitor implements DDC/CI. Budget models and some HDMI connections may not work. |
| Built-in HDMI on base M1/M2 | Doesn't support DDC/CI. Use USB-C/DisplayPort.                                             |
| Global shortcuts on Wayland | Wayland restricts global hotkey capture. Works on X11.                                     |
| Tray left-click on Linux    | AppIndicator doesn't always fire left-click. Right-click works.                            |
| Dark mode on non-GNOME      | `gsettings` is GNOME-specific. KDE, XFCE not supported.                                    |

---

## Troubleshooting

### "command not found: rustc" or "cargo"

Rust isn't in your PATH. Run `source "$HOME/.cargo/env"` (macOS/Linux) or close and reopen your terminal (Windows).

### First `npx tauri dev` takes 5+ minutes

Normal. Rust compiles everything from source. Cached in `src-tauri/target/`, so subsequent runs take ~5-15s.

### App launched but I can't find it

System tray app, not a regular window:

- **macOS**: Menu bar, top-right
- **Windows**: System tray, bottom-right (click `^` if hidden)
- **Linux**: Top panel. May need [AppIndicator GNOME extension](https://extensions.gnome.org/extension/615/appindicator-support/).

### "No displays found"

`core::PlatformImpl::detect_displays()` returned empty. Per platform:

- **macOS**: ensure the app/terminal has Accessibility permission for tiling (display detection itself doesn't require it, but useful to rule out). Try `cargo run` with logging on a known external monitor.
- **Windows**: confirm DDC/CI is enabled in the monitor's OSD; some HDMI ports on certain GPUs strip DDC.
- **Linux**: ensure `ddcutil` works as your user (not just root): `ddcutil detect`. If it works as root but not as user, check `groups` for `i2c` membership; you may need to log out and back in after `usermod -aG i2c $USER`.

### Dark mode toggle does nothing (Linux)

Requires GNOME. Check: `echo $XDG_CURRENT_DESKTOP` -- if not `GNOME`, dark mode won't work.
