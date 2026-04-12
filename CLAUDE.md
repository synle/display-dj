# Display DJ v2

## Project Overview
Cross-platform desktop system tray application for controlling monitor brightness, contrast, dark mode, and volume. Built with **Tauri v2** (Rust backend) + **React 18** (TypeScript frontend) + **Vite 6**.

Display and dark mode operations are delegated to the [display-dj CLI](https://github.com/synle/display-dj-cli), which runs as a bundled HTTP server sidecar. The Tauri backend makes HTTP requests to it. Volume control remains platform-specific in Rust.

## Architecture

### Frontend (`src/`)
- React 18 + TypeScript, bundled with Vite
- Communicates with backend via `invoke()` from `@tauri-apps/api/core`
- Listens for backend events via `listen()` from `@tauri-apps/api/event`
- Components: Header, Slider, MonitorControl, AllMonitorsControl, VolumeControl, DarkModeToggle
- `types.ts` — shared TypeScript interfaces (Monitor, MonitorConfig, Preferences, KeyBinding)
- `constants.ts` — shared constants (e.g. `LAPTOP_BUILT_IN_DISPLAY_ID`)

### Backend (`src-tauri/src/`)
- `lib.rs` — Tauri app setup, plugin init, sidecar launch (display-dj HTTP server), port discovery, window management, dock hiding (macOS)
- `display.rs` — Monitor brightness/contrast via HTTP requests to the display-dj server
- `dark_mode.rs` — Dark mode detection and toggling via HTTP requests to the display-dj server
- `volume.rs` — System volume get/set (platform-specific, not via display-dj)
- `config.rs` — Preferences and monitor config persistence (JSON files in OS config dir)
- `tray.rs` — System tray menu, window positioning, global keyboard shortcuts

### display-dj CLI sidecar (`src-tauri/binaries/`)
The [display-dj CLI](https://github.com/synle/display-dj-cli) is bundled as a Tauri sidecar. On app startup, `lib.rs` finds an available port (starting from 51337) and spawns `display-dj-server serve <port>`. All display and dark mode operations go through its HTTP API at `http://127.0.0.1:<port>/`.

Key HTTP routes used:
- `GET /get_all` — list all displays with live brightness/contrast
- `GET /set_one/<id>/<level>` — set one display's brightness
- `GET /set_all/<level>` — set all displays' brightness
- `GET /dark` / `GET /light` — switch dark/light mode
- `GET /theme` — get current theme
- `GET /health` — server health check

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
  - `config.rs`: Serialization/deserialization, defaults, camelCase conventions, file roundtrips, CommandValue enum variants
  - `display.rs`: `DjDisplay` to `Monitor` conversion, `merge_with_configs` (rename, disable, sort), Monitor serde
- **Smoke test**: `src-tauri/tests/smoke.rs` — Integration test verifying the crate compiles, links, and public API (AppState, run) is accessible

### CI
GitHub Actions (`build.yml`) runs `npm test` and `cargo test` on all platforms (macOS ARM/Intel, Windows, Linux) for every push and PR.

## Config File Locations
- **macOS**: `~/Library/Application Support/display-dj/`
- **Windows**: `%APPDATA%/display-dj/`
- **Linux**: `~/.config/display-dj/`

Files: `preferences.json`, `monitor-configs.json`

## Key Conventions
- All Rust structs sent to frontend use `#[serde(rename_all = "camelCase")]`
- Tauri commands are snake_case in Rust, called with snake_case strings from frontend `invoke()`
- Frontend parameter objects use camelCase (Serde handles the conversion)
- The `CommandValue` enum uses `#[serde(untagged)]` to support both `"string"` and `["array"]` in keybindings

## Dependencies
The display-dj CLI sidecar handles all platform-specific display dependencies internally. No external tools need to be installed for display control.

For the sidecar binary itself, download from [display-dj-cli releases](https://github.com/synle/display-dj-cli/releases) or build from source:
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
