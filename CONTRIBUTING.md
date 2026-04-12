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
- [display-dj CLI Sidecar](#display-dj-cli-sidecar)
- [Known Limitations](#known-limitations)
- [Troubleshooting](#troubleshooting)

---

## Quick Start

**Prerequisites**: [Node.js](https://nodejs.org) 18+, [Rust](https://www.rust-lang.org/tools/install) 1.77+, plus the display-dj CLI sidecar binary (see [display-dj CLI Sidecar](#display-dj-cli-sidecar)).

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

Display DJ is built with [Tauri v2](https://v2.tauri.app/), which pairs a **Rust** backend with a **web-based** frontend. Display and dark mode operations are delegated to the [display-dj CLI](https://github.com/synle/display-dj-cli), which runs as a bundled HTTP server sidecar.

```
┌──────────────────────────────────────────────┐
│  Frontend (React 18 + TypeScript + Vite 6)   │
│  Runs inside a WebView (WebKit on macOS/     │
│  Linux, WebView2 on Windows)                 │
│                                              │
│  Communicates via invoke() and listen()      │
└──────────────────┬───────────────────────────┘
                   │  Tauri IPC bridge
┌──────────────────▼───────────────────────────┐
│  Backend (Rust + Tauri v2)                   │
│                                              │
│  #[tauri::command] functions handle:         │
│  - Monitor brightness (via HTTP)              │
│  - System dark mode toggle (via HTTP)        │
│  - System volume control (platform-specific) │
│  - Preferences/config persistence            │
│  - System tray + global keyboard shortcuts   │
│                                              │
│  HTTP requests to display-dj server sidecar  │
└──────────────────┬───────────────────────────┘
                   │  HTTP (127.0.0.1:<port>)
┌──────────────────▼───────────────────────────┐
│  display-dj CLI sidecar (HTTP server)        │
│  https://github.com/synle/display-dj-cli     │
│                                              │
│  Handles all display operations:             │
│  - DDC/CI, gamma, DisplayServices, WMI       │
│  - Dark mode (osascript, registry, gsettings)│
│  - Cross-platform, single binary per OS      │
└──────────────────────────────────────────────┘
```

The frontend calls Rust functions using `invoke("command_name", { params })`. The Rust backend makes HTTP requests to the display-dj server for display and dark mode operations. The backend can push events to the frontend using `app.emit("event-name", payload)`.

---

## Project Structure

```
display-dj2/
├── src/                          # Frontend (React 18 + TypeScript)
│   ├── main.tsx                  # Entry point -- mounts React into <div id="root">
│   ├── App.tsx                   # Root component: fetches data, manages state, renders layout
│   ├── App.test.tsx              # Smoke test: App renders, fetches data, handles errors
│   ├── App.css                   # All CSS (dark tray popup theme, sliders, toggles)
│   ├── types.ts                  # Shared TypeScript interfaces (Monitor, MonitorConfig, Preferences, KeyBinding, NightModeSchedule)
│   ├── types.d.ts                # Global type definitions (Command, DisplayType, BrightnessPreset, etc.)
│   ├── index.d.ts                # Ambient module declarations (SVG/image imports, legacy adapters)
│   ├── constants.ts              # Shared constants (LAPTOP_BUILT_IN_DISPLAY_ID)
│   ├── test/
│   │   └── setup.ts              # Vitest setup: jsdom, jest-dom matchers, Tauri API mocks
│   └── components/
│       ├── Header.tsx            # "Display DJ v2.0.0" + expand/collapse chevron
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
│       └── SettingsPanel.tsx     # In-app settings: min brightness, deltas, night mode schedule
│
├── src-tauri/                    # Backend (Rust + Tauri v2)
│   ├── Cargo.toml                # Rust dependencies (tauri, reqwest, serde, etc.)
│   ├── build.rs                  # Tauri build script (runs before compilation)
│   ├── tauri.conf.json           # App config: window size, tray icon, sidecar, bundling options
│   ├── binaries/                 # display-dj CLI sidecar (per-platform binaries)
│   │   └── display-dj-server-<target-triple>
│   ├── capabilities/
│   │   └── default.json          # Security permissions (what frontend JS can access)
│   ├── icons/                    # App icons for all platforms (.icns, .ico, .png)
│   ├── tests/
│   │   └── smoke.rs              # Integration smoke test: crate links, public API usable
│   └── src/
│       ├── main.rs               # Binary entry point (calls lib::run)
│       ├── lib.rs                # Tauri app setup: sidecar launch, port discovery, plugins, state, tray, shortcuts, night mode schedule checker
│       ├── display.rs            # Monitor detection + brightness via HTTP to display-dj server (+ unit tests)
│       ├── dark_mode.rs          # Dark mode read/write via HTTP to display-dj server
│       ├── volume.rs             # System volume get/set (platform-specific: osascript, PowerShell, pactl)
│       ├── config.rs             # Preferences + monitor config load/save, NightModeSchedule, min brightness, reset to defaults (+ unit tests)
│       └── tray.rs               # System tray menu, window positioning, keyboard shortcut dispatch
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

**Backend** (`display.rs`): All display operations go through the display-dj HTTP server sidecar. No platform-specific code.

| Operation | HTTP Route | Response |
|---|---|---|
| Detect monitors | `GET /get_all` | JSON array of `DisplayInfo` with live brightness |
| Set one monitor | `GET /set_one/<id>/<level>` | Status |
| Set all monitors | `GET /set_all/<level>` | Status per display |

The `DjDisplay` struct maps the server's JSON response (with `display_type`, nullable `brightness`) into the app's `Monitor` struct (with `is_built_in`, `supports_brightness`).

**Key functions to know**:
- `detect_monitors()` -- HTTP GET to `/get_all`, converts `DjDisplay` -> `Monitor`
- `merge_with_configs()` -- applies user-configured names, sort orders, and disabled flags from `monitor-configs.json`
- `base_url()` -- returns `http://127.0.0.1:<port>` using the port stored at startup

### Dark Mode

**Frontend**: `App.tsx` calls `invoke("get_dark_mode")` and `invoke("set_dark_mode", { enabled })`. `DarkModeToggle.tsx` renders two buttons.

**Backend** (`dark_mode.rs`): Delegates to the display-dj HTTP server.

| Operation | HTTP Route | Response |
|---|---|---|
| Get current theme | `GET /theme` | `{"theme": "dark"}` or `{"theme": "light"}` |
| Set dark mode | `GET /dark` | Status |
| Set light mode | `GET /light` | Status |

### Volume

**Frontend**: `App.tsx` calls `invoke("get_volume")` and `invoke("set_volume", { value })`. `VolumeControl.tsx` wraps `Slider.tsx` with muted/unmuted icon logic.

**Backend** (`volume.rs`): Volume is the only module with platform-specific code, as the display-dj CLI does not handle volume.

| Platform | Method |
|---|---|
| macOS | `osascript` for CoreAudio: `output volume of (get volume settings)` / `set volume output volume` |
| Windows | PowerShell with inline C# `Add-Type` for WASAPI COM (`IAudioEndpointVolume`) |
| Linux | `pactl get-sink-volume @DEFAULT_SINK@` / `pactl set-sink-volume @DEFAULT_SINK@ <val>%` |

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
- Brightness delta (5–20) — step size for keyboard shortcut brightness changes
- Contrast delta (5–20) — step size for contrast changes
- Night mode schedule — enable/disable, set times and brightness levels

Loads preferences via `invoke("get_preferences")`, saves via `invoke("save_preferences")`. Values are clamped to slider ranges on load.

### System Tray & Keyboard Shortcuts

**Backend** (`tray.rs`):
- `setup_tray()` -- creates the tray icon + context menu (Dark Mode, Light Mode, Open Configs, Open Preferences, Quit)
- Left-click toggles main window visibility; `position_window_near_tray()` places it near the tray icon
- Right-click opens the context menu
- `register_shortcuts()` -- reads key bindings from `Preferences`, registers via `tauri-plugin-global-shortcut`
- `execute_command()` -- dispatches command strings like `"command/changeBrightness/50"` to the display-dj HTTP server and emits events to the frontend so it can refresh

### Config Persistence

**Backend** (`config.rs`):
- `config_dir()` resolves to `~/Library/Application Support/display-dj` (macOS), `%APPDATA%/display-dj` (Windows), `~/.config/display-dj` (Linux)
- `load_preferences()` / `save_preferences_to_disk()` -- reads/writes `preferences.json`, returns `Preferences::default()` if file missing or malformed
- `load_monitor_configs()` / `save_monitor_configs_to_disk()` -- reads/writes `monitor-configs.json` as `HashMap<String, MonitorConfig>`

---

## How the Frontend and Backend Communicate

**Frontend -> Backend** (calling Rust functions):

```typescript
import { invoke } from "@tauri-apps/api/core";

const monitors = await invoke<Monitor[]>("get_monitors");
await invoke("set_brightness", { monitorId: "external-1", value: 75 });
```

- Command names are **snake_case** strings matching the Rust function name
- Parameters are passed as a single object with **camelCase** keys (Serde converts them)

**Backend -> display-dj server** (HTTP requests):

```rust
let url = format!("http://127.0.0.1:{}/set_all/{}", port, value);
reqwest::blocking::get(&url)?;
```

**Backend -> Frontend** (pushing events):

```rust
use tauri::Emitter;
app.emit("monitors-changed", ())?;
```

```typescript
import { listen } from "@tauri-apps/api/event";

const unlisten = await listen("monitors-changed", () => {
  // Refetch monitor data
});
```

Events are used when keyboard shortcuts change brightness/volume/dark mode from the backend, so the frontend can update its UI.

**All registered commands** (defined in `lib.rs` `invoke_handler`):

| Module | Commands |
|---|---|
| `display` | `get_monitors`, `set_brightness`, `set_all_brightness`, `rename_monitor` |
| `dark_mode` | `get_dark_mode`, `set_dark_mode` |
| `volume` | `get_volume`, `set_volume` |
| `config` | `get_preferences`, `save_preferences`, `get_monitor_configs`, `save_monitor_config`, `open_config_file`, `open_preferences_file`, `get_app_version` |

**Events emitted by backend**:

| Event | Emitted when |
|---|---|
| `monitors-changed` | Keyboard shortcut changes brightness |
| `dark-mode-changed` | Keyboard shortcut toggles dark mode |
| `volume-changed` | Keyboard shortcut changes volume |

---

## Frontend State Management

`App.tsx` is the single source of truth for all UI state. There is no external state library.

**State variables**:
- `monitors: Monitor[]` -- current monitor list with brightness values
- `darkMode: boolean` -- system dark mode state
- `volume: number` -- system volume (0-100)
- `expanded: boolean` -- collapsed (all-monitors) vs expanded (individual) view
- `version: string` -- app version from Cargo.toml

**Data flow**:
1. On mount, `useEffect` calls `invoke()` for each data source (`get_monitors`, `get_dark_mode`, `get_volume`, `get_app_version`)
2. Event listeners (`listen("monitors-changed", ...)`) trigger refetches when the backend changes state via keyboard shortcuts
3. `visibilitychange` listener refetches all data when the tray popup becomes visible (catches external changes)
4. Slider/toggle handlers call `invoke()` to push changes to the backend, then optimistically update local state

**Average calculations** (collapsed view):
- `avgBrightness` = mean of all monitors' brightness

**Settings panel**: Opened from the header, `SettingsPanel.tsx` manages its own local state loaded from `get_preferences`. On save, it writes back to the backend and triggers a refresh in `App.tsx` via the `onPreferencesSaved` callback.

---

## Configuration Files

All config files live in the platform-specific config directory (`config_dir()` in `config.rs`).

### preferences.json

Deserialized into the `Preferences` struct. If the file is missing or malformed, `Preferences::default()` is used and written to disk.

| Field | Type | Default | Purpose |
|---|---|---|---|
| `showIndividualDisplays` | bool | `false` | Start in expanded view |
| `brightnessDelta` | number | `10` | Step size for keyboard shortcut brightness changes |
| `contrastDelta` | number | `10` | Step size for contrast changes |
| `minBrightness` | number | `10` | Minimum brightness floor (absolute floor: 5) |
| `nightModeSchedule` | object | disabled, 21:00–07:00 | Auto brightness + dark mode by time of day |
| `keyBindings` | array | 9 default bindings | Global keyboard shortcuts |

**Night mode schedule fields**:

| Field | Type | Default | Purpose |
|---|---|---|---|
| `enabled` | bool | `false` | Whether the schedule is active |
| `nightStart` | string | `"21:00"` | Time to switch to night mode (HH:MM, 24h) |
| `nightBrightness` | number | `20` | Brightness during night window |
| `dayStart` | string | `"07:00"` | Time to switch to day mode (HH:MM, 24h) |
| `dayBrightness` | number | `100` | Brightness during day window |

Each key binding has a `key` (e.g. `"Shift+F1"`) and a `command` -- either a single string (`"command/changeBrightness/50"`) or an array of strings for multi-action shortcuts.

**Supported command format**: `command/<action>/<value>`

| Action | Values | Effect |
|---|---|---|
| `changeBrightness` | Any integer 0-100 (e.g. `0`, `10`, `50`, `100`) | Sets all monitors' brightness |
| `changeDarkMode` | `toggle`, `dark`, `light` | Toggles or sets system dark mode |
| `changeVolume` | Any integer 0-100 (e.g. `0`, `50`, `100`) | Sets system volume |

### monitor-configs.json

A `HashMap<String, MonitorConfig>` keyed by monitor ID (e.g. `"external-1"`, `"builtin-0"`).

| Field | Type | Purpose |
|---|---|---|
| `id` | string | Monitor identifier (matches backend) |
| `name` | string | Custom display name (empty string = keep auto-detected name) |
| `sortOrder` | number | Display order in the UI (lower = higher) |
| `disabled` | bool | Hide this monitor from the UI |

---

## Conventions

### Naming

- **Rust structs** sent to the frontend use `#[serde(rename_all = "camelCase")]`. Fields are `snake_case` in Rust, `camelCase` in JSON/TypeScript.
- **Tauri commands** are `snake_case` in Rust and called with `snake_case` strings from the frontend (`invoke("get_monitors")`).
- **Frontend parameters** are passed as `camelCase` objects -- Serde handles the conversion automatically.
- The `CommandValue` enum uses `#[serde(untagged)]` so keybinding commands can be either `"string"` or `["array"]` in JSON.

### Display operations

- All display and dark mode operations go through the display-dj HTTP server sidecar -- there is **no platform-specific display code** in the Rust backend.
- Volume is the only platform-specific module, using `#[cfg(target_os = "...")]` conditional compilation.
- The sidecar port is discovered at startup and stored in a global `AtomicU16`. All modules access it via `crate::server_port()`.
- `Preferences` and `NightModeSchedule` use `#[serde(default)]` so old config files missing new fields gracefully fall back to defaults without breaking.
- Brightness values are clamped to `effective_min_brightness()` (which enforces `ABSOLUTE_MIN_BRIGHTNESS = 5`) before being sent to displays.

### Error handling

- Backend functions return `Result<T, String>`. Errors are human-readable strings.
- Frontend `invoke()` calls are wrapped in try/catch with `console.error` logging. The UI does not crash on backend errors -- it silently keeps the last known state.

### State

- `AppState` (in `lib.rs`) holds `Mutex<Preferences>` and `Mutex<MonitorConfigs>` for thread-safe shared access across Tauri commands.
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

| File | What it covers |
|---|---|
| `src/components/Header.test.tsx` | Title rendering, version display, expand/collapse toggle, chevron state |
| `src/components/Slider.test.tsx` | Range input rendering, debounced onChange (150ms), fill width calculation, prop updates |
| `src/components/DarkModeToggle.test.tsx` | Active button state, click handlers for dark/light |
| `src/components/VolumeControl.test.tsx` | Slider value, muted/unmuted icon switching |
| `src/components/AllMonitorsControl.test.tsx` | Brightness slider, average brightness calculation |
| `src/components/MonitorControl.test.tsx` | Monitor name rendering, inline edit mode (Enter/Escape), brightness slider, built-in vs external icons |
| `src/App.test.tsx` | **Smoke test**: App mounts without crashing, fetches initial data, renders all sections, handles backend errors gracefully |

**Tauri API mocking**: `src/test/setup.ts` globally mocks `invoke()` and `listen()` from `@tauri-apps/api` so tests run without a Tauri backend. The App smoke test provides per-command mock responses.

### Backend Tests (Rust)

Inline `#[cfg(test)]` modules plus an integration smoke test.

| Location | What it covers |
|---|---|
| `config.rs` | `Preferences` defaults, `CommandValue` Single/Multiple serde, camelCase JSON fields, file roundtrips, malformed JSON fallback, `get_app_version`, effective min brightness, backward-compatible deserialization of old configs |
| `display.rs` | `DjDisplay` to `Monitor` conversion (builtin, external DDC, null brightness), `Monitor` serde (camelCase, roundtrip), `merge_with_configs` (rename, disable, sort, empty name) |
| `tests/smoke.rs` | **Smoke test**: crate compiles and links, `AppState` is constructable, `run` function is exported |

Rust tests don't require external tools, hardware, or the display-dj server -- they test pure logic that works on all platforms.

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

Triggers on `v*` tags. Uses `tauri-apps/tauri-action` to build and upload platform installers to a GitHub draft release.

```bash
git tag v2.0.1
git push origin v2.0.1
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
const result = await invoke<string>("my_new_command", { someParam: "world" });
```

Note: Rust `some_param` maps to TypeScript `someParam` via serde's camelCase renaming.

4. **Add tests**: Unit test in the Rust module's `#[cfg(test)]` block, frontend test if it affects the UI.

---

## Day-to-day Commands

| Command | What it does |
|---|---|
| `npx tauri dev` | Full app in dev mode. Frontend hot-reloads. Rust recompiles incrementally (~5-15s). |
| `npx tauri build` | Production build: optimized binary + platform installer. |
| `npm run build` | Frontend only (TypeScript check + Vite bundle). Quick way to catch frontend errors. |
| `npm run dev` | Vite dev server only (no Rust). Tauri API calls fail, but useful for CSS/layout work. |
| `npm test` | Frontend tests (Vitest). ~1 second. |
| `cd src-tauri && cargo test` | Backend tests. |
| `cd src-tauri && cargo check` | Check Rust compiles. Faster than a full build. |

---

## Building for Production

```bash
npx tauri build
```

| Platform | Output |
|---|---|
| macOS | `src-tauri/target/release/bundle/dmg/Display DJ_2.0.0_aarch64.dmg` |
| Windows | `src-tauri\target\release\bundle\nsis\Display DJ_2.0.0_x64-setup.exe` |
| Linux | `src-tauri/target/release/bundle/deb/display-dj_2.0.0_amd64.deb`<br>`src-tauri/target/release/bundle/appimage/display-dj_2.0.0_amd64.AppImage` |

---

## Platform Setup Guides

These guides walk through setting up a development environment from a fresh machine. If you already have Node.js 18+ and Rust 1.77+ installed, skip to the sidecar setup.

### Prerequisites (All Platforms)

| Tool | Minimum Version | Purpose |
|---|---|---|
| **Git** | Any recent version | Clone the repository |
| **Node.js** | 18+ (CI uses 20) | Build the React/TypeScript frontend |
| **Rust** | 1.77+ (stable) | Build the Tauri/Rust backend |

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

#### Step 4: Set up the display-dj sidecar

See [display-dj CLI Sidecar](#display-dj-cli-sidecar) below.

#### Step 5: Clone and run

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

#### Step 5: Set up the display-dj sidecar

See [display-dj CLI Sidecar](#display-dj-cli-sidecar) below.

#### Step 6: Clone and run

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
# External monitors (DDC/CI) -- required by the display-dj CLI sidecar
sudo apt install -y ddcutil i2c-tools
sudo modprobe i2c-dev
echo "i2c-dev" | sudo tee /etc/modules-load.d/i2c-dev.conf
sudo usermod -aG i2c $USER    # log out and back in after this

# Built-in laptop display -- required by the display-dj CLI sidecar
sudo apt install -y brightnessctl
```

**You must log out and back in** after the `usermod` command. Verify with `ddcutil detect`.

Volume (`pactl`) and dark mode (`gsettings`) are pre-installed on Ubuntu/GNOME.

#### Step 4: Set up the display-dj sidecar

See [display-dj CLI Sidecar](#display-dj-cli-sidecar) below.

#### Step 5: Clone and run

```bash
git clone <repo-url>
cd display-dj2
npm install
npx tauri dev
```

---

## display-dj CLI Sidecar

Display DJ delegates all display and dark mode operations to the [display-dj CLI](https://github.com/synle/display-dj-cli), which runs as a Tauri sidecar. The CLI is a single cross-platform binary (~556KB) that handles DDC/CI, gamma tables, DisplayServices, WMI, and more.

### How it works

1. On app startup, `lib.rs` finds an available TCP port (starting from 51337)
2. Spawns `display-dj-server serve <port>` as a background sidecar process
3. Waits for the server's `/health` endpoint to respond (up to 5 seconds)
4. All display/dark mode commands (`display.rs`, `dark_mode.rs`, `tray.rs`) make HTTP GET requests to `http://127.0.0.1:<port>/...`

### Getting the sidecar binary

**Option A: Download from releases**

Download the appropriate binary from [display-dj-cli releases](https://github.com/synle/display-dj-cli/releases) and place it in `src-tauri/binaries/` with the Tauri naming convention:

```
src-tauri/binaries/
  display-dj-server-aarch64-apple-darwin        # macOS ARM (M1/M2/M3/M4)
  display-dj-server-x86_64-apple-darwin         # macOS Intel
  display-dj-server-x86_64-pc-windows-msvc.exe  # Windows x64
  display-dj-server-aarch64-pc-windows-msvc.exe # Windows ARM
  display-dj-server-x86_64-unknown-linux-gnu    # Linux x64
  display-dj-server-aarch64-unknown-linux-gnu   # Linux ARM
```

**Option B: Build from source**

```bash
git clone https://github.com/synle/display-dj-cli.git
cd display-dj-cli
cargo build --release

# Copy to the sidecar directory with the correct name
# Replace <target-triple> with your platform (e.g. aarch64-apple-darwin)
cp target/release/display-dj ../display-dj2/src-tauri/binaries/display-dj-server-<target-triple>
```

You only need the binary for your current platform during development. The CI pipeline builds all platform variants for releases.

### Verifying the sidecar

```bash
# Test the CLI directly
./src-tauri/binaries/display-dj-server-aarch64-apple-darwin list

# Test the HTTP server
./src-tauri/binaries/display-dj-server-aarch64-apple-darwin serve 51337 &
curl http://127.0.0.1:51337/health    # should return {"status":"ok"}
curl http://127.0.0.1:51337/get_all   # should return display JSON
kill %1
```

### HTTP API routes used by Display DJ

| Route | Used by | Purpose |
|---|---|---|
| `GET /health` | `lib.rs` | Wait for server readiness on startup |
| `GET /get_all` | `display.rs` | List all displays with live brightness |
| `GET /set_one/<id>/<level>` | `display.rs` | Set one display's brightness |
| `GET /set_all/<level>` | `display.rs`, `tray.rs` | Set all displays' brightness |
| `GET /theme` | `dark_mode.rs` | Get current system theme (dark/light) |
| `GET /dark` | `dark_mode.rs`, `tray.rs` | Switch to dark mode |
| `GET /light` | `dark_mode.rs`, `tray.rs` | Switch to light mode |

For the full API reference, see the [display-dj CLI documentation](https://github.com/synle/display-dj-cli).

---

## Known Limitations

| Limitation | Details |
|---|---|
| DDC/CI not universal | Not every monitor implements DDC/CI. Budget models and some HDMI connections may not work. |
| Built-in HDMI on base M1/M2 | Doesn't support DDC/CI. Use USB-C/DisplayPort. |
| Global shortcuts on Wayland | Wayland restricts global hotkey capture. Works on X11. |
| Tray left-click on Linux | AppIndicator doesn't always fire left-click. Right-click works. |
| Dark mode on non-GNOME | `gsettings` is GNOME-specific. KDE, XFCE not supported. |

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

### "display-dj-server sidecar not found"

The sidecar binary is missing from `src-tauri/binaries/`. See [Getting the sidecar binary](#getting-the-sidecar-binary).

### "display-dj server did not become ready"

The sidecar started but isn't responding. Check:
1. Is the binary executable? `chmod +x src-tauri/binaries/display-dj-server-*`
2. Is port 51337 (or nearby) available? `lsof -i :51337`
3. Test the binary directly: `./src-tauri/binaries/display-dj-server-* serve 51337`

### "No displays found"

The display-dj CLI can't detect monitors. Test directly:
```bash
./src-tauri/binaries/display-dj-server-* list
```
If this returns an empty array, see the [display-dj CLI troubleshooting](https://github.com/synle/display-dj-cli).

On Linux, ensure `ddcutil` works: `sudo ddcutil detect`. If it works as root but not as user, check `groups` for `i2c` membership.

### Dark mode toggle does nothing (Linux)

Requires GNOME. Check: `echo $XDG_CURRENT_DESKTOP` -- if not `GNOME`, dark mode won't work.
