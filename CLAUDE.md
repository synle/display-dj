# Display DJ v2

## Project Overview
Cross-platform desktop system tray application for controlling monitor brightness, dark mode, and volume. Built with **Tauri v2** (Rust backend) + **React 18** (TypeScript frontend) + **Vite 6**.

Display and dark mode operations are delegated to the [display-dj CLI](https://github.com/synle/display-dj-cli), which runs as a bundled HTTP server sidecar. The Tauri backend makes HTTP requests to it. Volume control remains platform-specific in Rust.

## Architecture

### Frontend (`src/`)
- React 18 + TypeScript, bundled with Vite
- Communicates with backend via `invoke()` from `@tauri-apps/api/core`
- Listens for backend events via `listen()` from `@tauri-apps/api/event`
- Components: Header, Slider, MonitorControl, AllMonitorsControl, VolumeControl, DarkModeToggle, SettingsPanel
- `types.ts` — shared TypeScript interfaces (Monitor, MonitorMetadata, Preferences, KeyBinding, NightModeSchedule)
- `types.d.ts` — global type definitions (Command, DisplayType, BrightnessPreset, etc.)
- `constants.ts` — shared constants (e.g. `LAPTOP_BUILT_IN_DISPLAY_ID`)

### Backend (`src-tauri/src/`)
- `lib.rs` — Tauri app setup, plugin init, sidecar launch (display-dj HTTP server), port discovery, window management, dock hiding (macOS), night mode schedule checker
- `display.rs` — Monitor brightness via HTTP requests to the display-dj server
- `dark_mode.rs` — Dark mode detection and toggling via HTTP requests to the display-dj server
- `volume.rs` — System volume get/set (platform-specific, not via display-dj)
- `config.rs` — Preferences persistence (JSON file in OS config dir, includes per-monitor metadata), night mode schedule, min brightness with absolute floor, migration from legacy `monitor-configs.json`, reset to defaults
- `tray.rs` — System tray menu, window positioning, global keyboard shortcuts

### display-dj CLI sidecar (`src-tauri/binaries/`)
The [display-dj CLI](https://github.com/synle/display-dj-cli) is bundled as a Tauri sidecar. On app startup, `lib.rs` finds an available port (starting from 51337) and spawns `display-dj-server serve <port>`. All display and dark mode operations go through its HTTP API at `http://127.0.0.1:<port>/`.

Key HTTP routes used:
- `GET /get_all` — list all displays with live brightness
- `GET /set_one/<id>/<level>` — set one display's brightness
- `GET /set_all/<level>` — set all displays' brightness
- `GET /dark` / `GET /light` — switch dark/light mode
- `GET /theme` — get current theme
- `GET /health` — server health check
- `GET /debug` — full diagnostics: version, OS/arch, display enumeration, active tests (brightness/contrast per display, volume, theme). Restores all settings after testing. Returns JSON.

**Sidecar lifecycle:** The `CommandChild` handle is stored in `AppState.sidecar_child`. On app exit, the `RunEvent::Exit` handler in `lib.rs::run()` calls `child.kill()` to terminate the sidecar server. This prevents orphaned `display-dj-server` processes after the main app closes.

Sidecar binaries follow Tauri's naming convention:
```
src-tauri/binaries/
  display-dj-server-aarch64-apple-darwin      # macOS ARM
  display-dj-server-x86_64-apple-darwin       # macOS Intel
  display-dj-server-x86_64-pc-windows-msvc.exe  # Windows x64
  display-dj-server-x86_64-unknown-linux-gnu  # Linux x64
```

### Volume (platform-specific)
Volume is the only module with platform-specific code, as the display-dj CLI does not handle volume:
- **macOS**: `osascript` (CoreAudio)
- **Windows**: PowerShell + WASAPI COM interop
- **Linux**: `pactl` (PulseAudio/PipeWire)

## Build Commands
```bash
npm install          # Install frontend dependencies
npm run dev          # Start Vite dev server (frontend only)
npm run build        # Build frontend (tsc + vite build)
npx tauri dev        # Run full app in development mode
npx tauri build      # Production build (binary + .app/.dmg/.msi/.deb)
cargo check          # Check Rust compilation (from src-tauri/)
```

## Testing
```bash
npm test             # Run all frontend tests (Vitest)
npm run test:watch   # Run frontend tests in watch mode
cd src-tauri && cargo test  # Run all Rust backend tests
```

### Frontend Tests (Vitest + React Testing Library)
- **Setup**: `src/test/setup.ts` — Configures jsdom, jest-dom matchers, and Tauri API mocks
- **Unit tests**: `src/components/*.test.tsx` — Tests for each component (Header, Slider, DarkModeToggle, VolumeControl, AllMonitorsControl, MonitorControl)
- **Smoke test**: `src/App.test.tsx` — Verifies App renders without errors, fetches initial data, handles backend failures gracefully
- Tauri `invoke()` and `listen()` are mocked globally in the test setup

### Backend Tests (Rust)
- **Unit tests**: Inline `#[cfg(test)]` modules in `config.rs` and `display.rs`
  - `config.rs`: Serialization/deserialization, defaults, camelCase conventions, file roundtrips, CommandValue enum variants, MonitorMetadata serde, effective min brightness, backward-compatible deserialization of old configs, preferences with monitorConfigs roundtrip
  - `display.rs`: `DjDisplay` to `Monitor` conversion (including uid computation), `merge_with_configs` (rename, sort), `reconcile_migrated_configs`, `ensure_metadata_for_monitors`, Monitor serde
- **Smoke test**: `src-tauri/tests/smoke.rs` — Integration test verifying the crate compiles, links, and public API (AppState, run) is accessible

### CI
GitHub Actions (`build.yml`) runs `npm test` and `cargo test` on all platforms (macOS ARM/Intel, Windows, Linux) for every push and PR.

## Config File Locations
- **macOS**: `~/Library/Application Support/display-dj/`
- **Windows**: `%APPDATA%/display-dj/`
- **Linux**: `~/.config/display-dj/`

Files: `preferences.json` (includes per-monitor metadata — labels, sort order — as `monitorConfigs` array)

## Monitor Identity (UID scheme)

Each monitor is identified by a composite UID: `{api_id}::{api_model_name}` (e.g. `"1::Dell U2723QE"`, `"builtin::Built-in Display"`). This is more stable than the raw integer ID from the sidecar API, which can collide when monitors are swapped.

- `Monitor.id` — raw API id, used for sidecar HTTP calls (`/set_one/{id}/{value}`)
- `Monitor.uid` — composite key, used for config lookups, React keys, rename/reorder operations
- `MonitorMetadata` entries in `preferences.monitorConfigs` are **append-only** — new monitors are added on first detection, never removed on unplug. This preserves labels and sort order across plug/unplug cycles.
- On startup, a one-time migration converts old `monitor-configs.json` entries into `MonitorMetadata` format within preferences.

## Key Conventions
- All Rust structs sent to frontend use `#[serde(rename_all = "camelCase")]`
- Tauri commands are snake_case in Rust, called with snake_case strings from frontend `invoke()`
- Frontend parameter objects use camelCase (Serde handles the conversion)
- The `CommandValue` enum uses `#[serde(untagged)]` to support both `"string"` and `["array"]` in keybindings
- Preferences use `#[serde(default)]` so old config files missing new fields gracefully fall back to defaults
- Brightness values are clamped to `effective_min_brightness()` which enforces an absolute floor of 5

## Window Positioning (multi-monitor DPI)

The tray popup window must appear next to the tray icon, which can be on any monitor with any DPI scale factor. This is deceptively hard because of how Tauri handles coordinates on macOS. **Read the doc comment on `position_window_near_tray` in `tray.rs` before modifying positioning code.**

### The coordinate spaces

| API | Returns | Coordinate space |
|---|---|---|
| `tray.rect()` | `PhysicalPosition` / `PhysicalSize` | Global physical pixels |
| `monitor.position()` / `monitor.size()` | `PhysicalPosition` / `PhysicalSize` | Global physical pixels |
| `window.set_position(PhysicalPosition)` | — | Tauri divides by `window.scale_factor()` to get macOS points |
| `window.scale_factor()` | `f64` | Scale of the monitor the window is **currently** on |

### The pitfall

`window.scale_factor()` reflects the **current** monitor, not the target. When the window is on a 1× display and the tray is clicked on a 2× display, Tauri's `set_position` divides by 1 instead of 2, placing the window at double the intended macOS-point coordinate (off-screen).

**Attempted fix that does NOT work:** moving the hidden window to the target monitor before positioning. `scale_factor()` does not update synchronously after `set_position`.

### The fix: scale compensation

All positioning math runs in the global physical pixel space using `target_scale` (the tray's monitor). Before calling `set_position`, multiply by `window_scale / target_scale`:

```
Tauri does:     point = arg / window_scale
We need:        point = physical / target_scale
So we pass:     arg = physical × window_scale / target_scale
```

When both scales match (same monitor), the compensation is 1× (no-op).

### Debug logging

Enable debug logging via the tray menu → "Debug" → "Enable Logging" to write positioning data to `debug.log` in the config directory (auto-truncated at 1 MB, keeps last 80% when limit is hit). Open via tray menu → "Debug" → "Open Debug Log". Each tray click logs: tray rect, all monitors (position/size/scale), target selection, window scale, computed position, compensation factor, and final `set_position` arguments.

When debug logging is enabled, the app also calls the sidecar's `/debug` endpoint on startup and prepends the full diagnostic dump (version, OS, display enumeration, active brightness/contrast/volume/theme tests) to the debug log. This is useful for troubleshooting display detection issues. You can also hit the endpoint directly: `curl http://127.0.0.1:<port>/debug`.

## Dependencies
The display-dj CLI sidecar handles all platform-specific display dependencies internally. No external tools need to be installed for display control.

The sidecar version is defined in `package.json` under `displayDjCliVersion`. The Rust build script (`src-tauri/build.rs`) reads this at compile time and downloads the matching release from GitHub. The `DISPLAY_DJ_CLI_VERSION` env var can override it (used by CI `workflow_dispatch`).

For manual builds, download from [display-dj-cli releases](https://github.com/synle/display-dj-cli/releases) or build from source:
```bash
git clone https://github.com/synle/display-dj-cli.git
cd display-dj-cli
cargo build --release
cp target/release/display-dj ../display-dj2/src-tauri/binaries/display-dj-server-<target-triple>
```

### Linux (additional)
```bash
sudo apt install ddcutil brightnessctl i2c-tools
sudo modprobe i2c-dev
sudo usermod -aG i2c $USER
```
