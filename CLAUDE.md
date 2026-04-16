# Display DJ v2

## Project Overview

Cross-platform desktop system tray application for controlling monitor brightness, contrast, dark mode, volume, and keep-awake (sleep prevention). Built with **Tauri v2** (Rust backend) + **React 18** (TypeScript frontend) + **Vite 6**.

Display and dark mode operations are delegated to the [display-dj CLI](https://github.com/synle/display-dj-cli), which runs as a bundled HTTP server sidecar. The Tauri backend makes HTTP requests to it. Volume control remains platform-specific in Rust.

For full architecture details, request lifecycle, layer-by-layer breakdown, data flow diagrams, and "where to edit" reference, see **[DEV.md](DEV.md)**.

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
- **Unit tests**: `src/components/*.test.tsx` — Tests for each component (Header, Slider, DarkModeToggle, VolumeControl, AllMonitorsControl, MonitorControl, KeepAwakeToggle)
- **Smoke test**: `src/App.test.tsx` — Verifies App renders without errors, fetches initial data, handles backend failures gracefully
- Tauri `invoke()` and `listen()` are mocked globally in the test setup

### Backend Tests (Rust)

- **Unit tests**: Inline `#[cfg(test)]` modules in `config.rs`, `display.rs`, and `keep_awake.rs`
  - `config.rs`: Serialization/deserialization, defaults, camelCase conventions, file roundtrips, CommandValue enum variants, MonitorMetadata serde, effective min brightness, backward-compatible deserialization of old configs, preferences with monitorConfigs roundtrip
  - `display.rs`: `DjDisplay` to `Monitor` conversion (including uid computation), `merge_with_configs` (rename, sort), `reconcile_migrated_configs`, `ensure_metadata_for_monitors`, Monitor serde
  - `keep_awake.rs`: KeepAwake guard creation, Mutex<Option<KeepAwake>> pattern (enable/disable/re-enable)
- **Smoke test**: `src-tauri/tests/smoke.rs` — Integration test verifying the crate compiles, links, and public API (AppState, run) is accessible

### CI

GitHub Actions (`build.yml`) runs `npm test` and `cargo test` on all platforms (macOS ARM/Intel, Windows, Linux) for every push and PR. On PRs, a comment is posted with download links for each platform's build artifacts.

## Formatting

After making changes to frontend code (`src/`), config files, or docs, always run `npx prettier --write` on the changed files before considering the task done. The prettier hook in `.claude/settings.json` handles this automatically for Edit/Write tool calls, but if you create or modify files via other means, run prettier manually.

## Required Steps for Every Feature Change

1. **Tests**: Always add tests to cover new code. Frontend components get `*.test.tsx` files; Rust modules get `#[cfg(test)]` unit tests. Run `npm test` and `cd src-tauri && cargo test` to verify all tests pass before finishing.
2. **Formatting**: Always run `npx prettier --write` on all changed frontend files (`src/`, `*.ts`, `*.tsx`, `*.json`, `*.md`, `*.yml`).
3. **Documentation**: Always update `CLAUDE.md`, `README.md` (if it exists), and `CONTRIBUTING.md` to reflect any features added or removed — including new commands, preferences, HTTP routes, UI components, and architecture changes.
4. **Method comments**: Always add doc comments to all new functions and methods. Rust uses `///` doc comments; TypeScript/React uses `/** */` JSDoc comments. Every public function, Tauri command, React component, and non-trivial helper must have a comment describing what it does.
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

## Related Projects

- **[display-dj-cli](https://github.com/synle/display-dj-cli)** — The Rust CLI/HTTP server that handles all display operations (brightness, contrast, dark mode). Bundled as a Tauri sidecar. Source at `/Users/syle/Downloads/display-dj-cli`. When bumping the sidecar version, always review upstream changes in that repo.

## Dependencies

The display-dj CLI sidecar handles all platform-specific display dependencies internally. No external tools need to be installed for display control. The `keepawake` crate handles sleep prevention natively on all platforms (macOS IOKit, Windows SetThreadExecutionState, Linux D-Bus).

### Sidecar binaries

Pre-built sidecar binaries for all 6 platforms are committed in `src-tauri/binaries/`. The build script (`src-tauri/build.rs`) tries to download the latest from GitHub releases first, then falls back to the committed binary if the download fails (offline, timeout, etc.).

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
- **`release.yml`**: Triggered by `v*` tags. Builds all platforms and uploads artifacts to a GitHub release (created as draft, publish manually).

## GitHub Raw File URLs

When fetching raw file content from GitHub repos, always use the `?raw=true` blob URL format:

```
https://github.com/{owner}/{repo}/blob/head/{path}?raw=true
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
