# Contributing to Display DJ

## Quick Start

Prerequisites: [Node.js](https://nodejs.org) 20+, [Rust](https://www.rust-lang.org/tools/install) 1.77+ (stable). No external binaries — platform code is compiled in.

```bash
git clone https://github.com/synle/display-dj
cd display-dj
npm install
npx tauri dev
```

First Rust compile takes 2-10 min; later runs ~5-15s. The app is a **system tray app** — no window. Look for the icon: menu bar (macOS), system tray (Windows), top panel (Linux).

| Command                           | What it does                                         |
| --------------------------------- | ---------------------------------------------------- |
| `npx tauri dev`                   | Full app in dev mode (frontend hot-reloads)          |
| `npx tauri build`                 | Production build + installer                         |
| `npm run build`                   | Frontend only (tsc + Vite)                           |
| `npm run dev`                     | Vite only; Tauri calls fail                          |
| `npm test`                        | Frontend tests                                       |
| `cd src-tauri && cargo test`      | Backend tests                                        |
| `cd src-tauri && cargo check`     | Fast Rust compile check                              |
| `npm run format` / `format:check` | Format / verify formatting ([oxfmt](https://oxc.rs)) |

## Architecture

[Tauri v2](https://v2.tauri.app/): React 19 frontend in a WebView, Rust backend. All platform code is vendored in-process under `src-tauri/src/core/` — no sidecar, no HTTP server.

```
Frontend (React 19 + TypeScript + Vite 6)
    │  invoke() / listen()  — Tauri IPC
Backend (Rust): #[tauri::command] wrappers
    │  spawn_blocking → direct calls
core/ (vendored platform code)
```

- `src-tauri/src/{display,dark_mode,volume,wallpaper}.rs` — thin async Tauri-command wrappers around `core::*`. Every command taking `State<'_, AppState>` must be `async fn`, and CPU-bound work runs in `spawn_blocking` (macOS tray starvation pitfall — see AGENTS.md).
- `src-tauri/src/core/` — vendored platform implementations (`macos`/`windows`/`linux`, `theme`, `volume`, `wallpaper`, `display`). See [VENDORING.md](VENDORING.md).
- Other backend modules: `config.rs` (preferences), `tray.rs` (tray menu + shortcut dispatch), `tray_icon.rs`, `keep_awake.rs`, `sidecar_cache.rs` (5-min TTL cache), `overlay.rs` (soft-overlay brightness fallback), `crash_log.rs`, `tiling/`.

### Frontend ↔ Backend

```typescript
const monitors = await invoke<Monitor[]>('get_monitors');
await invoke('set_brightness', { monitorId: '1', value: 75 });
```

Command names are snake_case; parameters are camelCase objects (Serde converts).

```typescript
listen('monitors-changed', () => refetch());
```

Events (`monitors-changed`, `dark-mode-changed`, `volume-changed`) fire when keyboard shortcuts change state from the backend.

**Registered commands** (`lib.rs` `invoke_handler`):

| Module       | Commands                                                                                                                                                     |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `lib`        | `fetch_all_state`                                                                                                                                            |
| `display`    | `get_monitors`, `set_brightness`, `set_all_brightness`, `set_contrast`, `set_all_contrast`, `rename_monitor`, `save_monitor_order`, `set_monitor_visibility` |
| `dark_mode`  | `get_dark_mode`, `set_dark_mode`                                                                                                                             |
| `volume`     | `get_volume`, `set_volume`                                                                                                                                   |
| `config`     | `get_preferences`, `save_preferences`, `open_preferences_file`, `open_debug_log`, `open_app_folder`, `get_app_version`, `get_about_info`                     |
| `crash_log`  | `get_crash_log`, `open_crash_log`                                                                                                                            |
| `keep_awake` | `get_keep_awake`, `set_keep_awake`                                                                                                                           |
| `tray`       | `apply_profile`                                                                                                                                              |
| `tiling`     | `get_tiling_supported`, `get_accessibility_trusted`, `recheck_accessibility_trusted`, `open_accessibility_settings`                                          |

### Frontend state

`App.tsx` holds all UI state (monitors, darkMode, volume, profiles, expanded view); no state library. On mount it fetches via `fetch_all_state` + `get_preferences`; event listeners and a `visibilitychange` listener trigger refetches. Collapsed view shows averages across visible monitors. `SettingsPanel.tsx` auto-saves with a 300ms debounce.

## Configuration

Config lives in the platform config dir (`config_dir()` in `config.rs`): `~/Library/Application Support/display-dj` (macOS), `%APPDATA%/display-dj` (Windows), `~/.config/display-dj` (Linux). Main file: `preferences.json` — deserialized into `Preferences` with `#[serde(default)]`, so missing fields fall back to defaults.

Top-level fields: `showIndividualDisplays`, `minBrightness` (default 10, absolute floor 5), `keyBindings`, `profiles`, `nightModeSchedule` (21:00–07:00 default, disabled), `showContrast`, `debugLogging`, `launchAtLogin`, `monitorConfigs`, `tiling`, `layoutPresets`, `wallpaper`.

Key bindings pair a `key` (e.g. `"Shift+F1"`) with a `command` string or array of strings: `command/changeBrightness/{v}`, `command/changeContrast/{v}`, `command/changeDarkMode/{toggle,dark,light}`, `command/changeVolume/{v}`, `command/changeProfile/{index}` — plus the tile/wallpaper/layout/z-order commands documented in AGENTS.md.

`monitorConfigs[]` stores per-monitor metadata keyed by a stable composite UID (`{api_id}::{api_model_name}`): `label`, `sortOrder`, `hidden`. Unplugged monitors keep their entry.

## Conventions

- Structs sent to the frontend use `#[serde(rename_all = "camelCase")]`.
- Commands are snake_case in Rust and JS.
- `CommandValue` is `#[serde(untagged)]`: `"string"` or `["array"]`.
- Backend returns `Result<T, String>`; frontend wraps `invoke()` in try/catch and keeps last-known state on error.
- Brightness clamps to `[effective_min_brightness(), 100]`; contrast to `[0, 100]`.
- Full conventions, pitfalls, and command reference live in **[AGENTS.md](AGENTS.md)**; architecture deep-dive in **[DEV.md](DEV.md)**.

## Adding a New Tauri Command

1. Define it (async if it touches `AppState`):

```rust
#[tauri::command]
pub async fn my_command(some_param: String) -> Result<String, String> {
    Ok(format!("Hello {some_param}"))
}
```

2. Register in `lib.rs` `invoke_handler`.
3. Call it: `await invoke('my_command', { someParam: 'world' })`.
4. Add tests: `#[cfg(test)]` block in the module, frontend test if UI-visible.

## Testing

```bash
npm test                                 # Frontend (Vitest + RTL), files sit next to components as *.test.tsx
cd src-tauri && cargo test               # Backend, inline #[cfg(test)] modules + tests/smoke.rs
npm run test:coverage                    # Frontend coverage gate
cd src-tauri && cargo llvm-cov --lib --summary-only   # Backend coverage
```

Tauri APIs are mocked globally in `src/test/setup.ts`, so tests run without a backend. Coverage thresholds live in `vite.config.ts` and `.github/workflows/build.yml` — see [DEV.md](DEV.md) before changing them.

## CI/CD

- **build.yml** — every push/PR: tests + builds across a 6-entry matrix (macOS ARM64/x64, Windows x64/arm64, Linux x64/arm64) plus coverage. Rolled-up check: **`Main Build`** (job `main_build`) — require this one in branch protection.
- **release-official.yml** — `v*` tag or manual dispatch: official release.
- **release-beta.yml** — manual dispatch: draft prerelease tagged `release-beta-<date>-<sha>`.

## Platform Setup (dev)

All platforms: Git, Node.js 20+, Rust stable.

- **macOS**: `xcode-select --install`; Node/Rust via Homebrew or rustup.
- **Windows**: Node LTS (+ C++ build tools), Rustup, WebView2 (preinstalled on Win11).
- **Linux (Ubuntu/Debian)** -- Tauri v2 GUI build dependencies:

  ```bash
  sudo apt install -y git build-essential pkg-config \
    libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev \
    libayatana-appindicator3-dev librsvg2-dev patchelf libxdo-dev
  # display control
  sudo apt install -y ddcutil i2c-tools brightnessctl
  sudo modprobe i2c-dev
  echo "i2c-dev" | sudo tee /etc/modules-load.d/i2c-dev.conf
  sudo usermod -aG i2c $USER   # log out and back in
  ```

  Notes:
  - `libssl-dev` is required by the `openssl-sys` crate (reqwest); missing it fails `cargo check` with an openssl-sys build error.
  - On Ubuntu 24.04 / Mint 22+ use `libayatana-appindicator3-dev` (the older `libappindicator3-dev` package no longer exists). Tray icon support needs it.
  - `libwebkit2gtk-4.1-dev` pulls in WebView for Tauri; on Ubuntu 22.04 also install `libjavascriptcoregtk-4.1-dev` if `pkg-config` can't find it.
  - DDC/CI brightness/contrast on external monitors requires `i2c-dev` access: run `ddcutil detect` as your user to verify (see Troubleshooting).

Verify: `node --version`, `rustc --version`, and on Linux `ddcutil detect`.

## Known Limitations

| Limitation                  | Details                                                               |
| --------------------------- | --------------------------------------------------------------------- |
| DDC/CI not universal        | Budget models and some HDMI connections don't implement it.           |
| Built-in HDMI on base M1/M2 | No DDC/CI. Use USB-C/DisplayPort.                                     |
| Global shortcuts on Wayland | Wayland restricts global hotkeys. Works on X11.                       |
| Tray left-click on Linux    | AppIndicator doesn't always fire left-click; right-click works.       |
| Dark mode on non-GNOME      | `gsettings` is GNOME-specific; KDE/XFCE unsupported for theme writes. |

## Troubleshooting

- **`command not found: rustc/cargo`** — `source "$HOME/.cargo/env"` (or reopen terminal on Windows).
- **First `tauri dev` slow** — normal; cached in `src-tauri/target/`.
- **Can't find the app** — tray app, no window (see Quick Start).
- **"No displays found"** — Linux: `ddcutil detect` as your user; check `i2c` group membership. Windows: enable DDC/CI in the monitor OSD. macOS: try USB-C/DP instead of HDMI.
- **Dark mode does nothing on Linux** — requires GNOME (`echo $XDG_CURRENT_DESKTOP`).
- **Tile shortcuts silently no-op on Chromium apps (macOS)** — known AX limitation; fixed since v7.0.24 via NSWorkspace fallback. Details in [DEV.md](DEV.md).
