# Display DJ v2

## Project Overview
Cross-platform desktop system tray application for controlling monitor brightness, contrast, dark mode, and volume. Built with **Tauri v2** (Rust backend) + **React 18** (TypeScript frontend) + **Vite 6**.

## Architecture

### Frontend (`src/`)
- React 18 + TypeScript, bundled with Vite
- Communicates with backend via `invoke()` from `@tauri-apps/api/core`
- Listens for backend events via `listen()` from `@tauri-apps/api/event`
- Components: Header, Slider, MonitorControl, AllMonitorsControl, VolumeControl, DarkModeToggle

### Backend (`src-tauri/src/`)
- `lib.rs` — Tauri app setup, plugin init, window management, dock hiding (macOS)
- `display.rs` — Platform-specific monitor brightness/contrast control via DDC/CI
- `dark_mode.rs` — System dark mode detection and toggling
- `volume.rs` — System volume get/set
- `config.rs` — Preferences and monitor config persistence (JSON files in OS config dir)
- `tray.rs` — System tray menu, window positioning, global keyboard shortcuts

### Platform-specific implementations
Each module (`display`, `dark_mode`, `volume`) uses `#[cfg(target_os = "...")]` for platform code:
- **macOS**: CLI tools (`m1ddc`, `ddcctl`, `brightness`, `osascript`)
- **Windows**: Win32 API (`Dxva2` for DDC/CI, `winreg` for dark mode, PowerShell for WMI/volume)
- **Linux**: CLI tools (`ddcutil`, `brightnessctl`, `pactl`, `gsettings`)

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
  - `display.rs`: `extract_display_number`, `merge_with_configs` (rename, disable, sort), Monitor serde
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

## Dependencies (macOS)
External monitor control requires `m1ddc` (Apple Silicon) or `ddcctl` (Intel):
```bash
brew install m1ddc
```

## Dependencies (Linux)
```bash
sudo apt install ddcutil brightnessctl i2c-tools
sudo modprobe i2c-dev
sudo usermod -aG i2c $USER
```
