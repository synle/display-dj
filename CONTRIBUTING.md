# Contributing to Display DJ

## Quick Start

Prerequisites: [Node.js](https://nodejs.org) matching `^20.19.0 || ^22.13.0 || >=24.0.0` for the current Vite/jsdom toolchain, and [Rust](https://www.rust-lang.org/tools/install) 1.92+ (stable). Recheck package engines when upgrading frontend tools. Platform code is compiled in; Linux still needs system helpers.

```bash
git clone https://github.com/synle/display-dj
cd display-dj
npm install
npx tauri dev
```

First Rust compile takes 2-10 min; later runs ~5-15s. The app is a **system tray app** — no window. Look for the icon: menu bar (macOS), system tray (Windows), top panel (Linux).

| Command                       | What it does                                                       |
| ----------------------------- | ------------------------------------------------------------------ |
| `npx tauri dev`               | Full app in dev mode (frontend hot-reloads)                        |
| `npx tauri build`             | Production build + installer                                       |
| `npm run build`               | Frontend only (tsc + Vite)                                         |
| `npm run dev`                 | Vite only; Tauri calls fail                                        |
| `npm run lint`                | Oxlint; separate from build, tests, and formatting                 |
| `npm test`                    | Frontend tests                                                     |
| `cd src-tauri && cargo test`  | Backend tests                                                      |
| `cd src-tauri && cargo check` | Fast Rust compile check                                            |
| `npm run format`              | Format frontend and Rust; sync app version (`oxfmt` + `cargo fmt`) |
| `npm run format:rust`         | Format Rust only (`cargo fmt`)                                     |
| `npm run format:check`        | Check frontend and Rust formatting without writing                 |

VS Code launch entries in `.vscode/launch.json` cover Tauri dev/build, Vite, Vitest run/watch, Cargo test, and Cargo check.
These are Node launch configurations, not lldb sessions.
`.vscode/tasks.json` defines a separate `ui:dev` background task; current launch entries do not reference it.

## Architecture

[Tauri v2](https://v2.tauri.app/): React 19 frontend in a WebView, Rust backend. Platform code runs in-process under `src-tauri/src/core/`; most modules are vendored and `audio_output` is display-dj-local. No sidecar or HTTP server runs. Read [ARCHITECTURE.md](ARCHITECTURE.md) for layers, state ownership, startup, and edit locations.

```
Frontend (React 19 + TypeScript + Vite 8)
    │  invoke() / listen()  — Tauri IPC
Backend (Rust): #[tauri::command] wrappers
    │  spawn_blocking → direct calls
core/ (vendored platform code)
```

- `src-tauri/src/{display,dark_mode,volume,wallpaper}.rs` — thin async Tauri-command wrappers around `core::*`. Commands taking `State<'_, AppState>` must be async; offload blocking work with `spawn_blocking` (see the macOS Tray Icon Pitfall in [FEATURES.md](FEATURES.md#macos-tray-icon-pitfall-critical)). The existing synchronous `get_preferences` reader is a legacy exception.
- `src-tauri/src/core/` — platform mechanisms and shared display types, with vendored sources plus local audio, DPI, loudness, shell-action, Linux desktop, and Windows child-process extensions. See [VENDORING.md](VENDORING.md); current Tauri comes from crates.io, not a vendored crate patch.
- Other backend modules: `config.rs` (preferences), `tray.rs` (tray menu + shortcut dispatch), `tray_icon.rs`, `keep_awake.rs`, `sidecar_cache.rs` (5-min TTL cache), `overlay.rs` (soft-overlay brightness fallback), `crash_log.rs`, `tiling/`.
- Windows builds embed `windows-app-manifest.xml` through `build.rs` at `asInvoker` integrity. Preserve the Common Controls v6 dependency, and keep `tile-snap-overlay-*` plus `overlay-*` covered by Tauri capabilities so their event listeners can receive render state.
- Windows mouse regression check: with Tile Snap both off and on, verify ordinary pointer movement and clicks stay responsive, and title-bar double-clicks toggle native maximize/restore. The polling monitor must not install a global low-level mouse hook; its source-level regression guard runs on every host, but interactive verification requires Windows.

### Frontend ↔ Backend

```typescript
const monitors = await invoke<Monitor[]>('get_monitors');
await invoke('set_brightness', { monitorId: '1', value: 75 });
```

Command names are snake_case; parameters are camelCase objects (Serde converts).

```typescript
listen('monitors-changed', () => refetch());
```

Monitor/theme events refresh live metadata and theme; `audio-output-changed` supplies endpoint snapshots. Volume commands also emit `volume-changed`, but the main popup renders saved slider values rather than subscribing to live volume updates.

Before adding or calling a command, read the `invoke_handler` registration in `src-tauri/src/lib.rs` and its owning module.
That registration is the command inventory; do not maintain a second list here.
For bindable `command/...` strings, read [FEATURES.md → Command Reference](FEATURES.md#command-reference).

### Frontend state

`App.tsx` holds all UI state (monitors, darkMode, volume, audio outputs, profiles, independent monitor/speaker expansion); no state library. On mount it fetches via `fetch_all_state` + `get_preferences`; event listeners and a `visibilitychange` listener trigger refetches. A backend 5-second audio refresh owns OS enumeration and updates the tray submenu; the main-view poll reads that shared cache as a fallback. The speaker section has its own chevron; expanded mode lists visible endpoints, including the checked selected output, with padded output radio targets, per-row aliases, and arrows that persist custom order. `SettingsPanel.tsx` loads all endpoints and owns enabled/disabled/hidden selectors on Monitors & Speakers. Settings names are read-only and have no reorder arrows. System contains Startup, Wallpaper, Night Mode, and one selected-profile editor. Its 100ms save queue requests a close/unmount flush but has no save-status badge. All frontend selects use `components/Dropdown.tsx`; context classes may set width or minimum width, while shared sizing, padding, typography, focus, and disabled states stay in `.dropdown`.

## Configuration

Config lives in the platform config dir (`config_dir()` in `config.rs`): `~/Library/Application Support/display-dj` (macOS), `%APPDATA%/display-dj` (Windows), `~/.config/display-dj` (Linux). Main file: `preferences.json` — deserialized into `Preferences` with `#[serde(default)]`, so missing fields fall back to defaults.

Selected top-level fields (see `Preferences` in `config.rs` and `src/types.ts` for the full schema): `showIndividualDisplays`, `minBrightness` (default 10, absolute floor 5), `keyBindings`, `lockScreenBindingInitialized` (backend-owned migration marker), `profiles`, `nightModeSchedule` (21:00–07:00 default, disabled), `showContrast`, `debugLogging`, `launchAtLogin`, `linuxLockCommand` (backend-owned Linux cache), `monitorConfigs`, `audioOutputConfigs`, `tiling`, `layoutPresets`, `wallpaper`.

Key bindings pair a `key` (e.g. `"Shift+F1"`) with a `command` string or array of strings: `command/changeBrightness/{v}`, `command/changeContrast/{v}`, `command/changeDarkMode/{toggle,dark,light}`, `command/changeVolume/{v}`, `command/changeProfile/{index_or_name}`, `command/system/lockScreen` — plus the tile/wallpaper/layout/z-order commands in [FEATURES.md → Command Reference](FEATURES.md#command-reference). Lock Screen defaults to `Ctrl+Alt+Q` on Windows/Linux only. Default z-order chords use `Super` on macOS and `Alt` on Windows/Linux.

`monitorConfigs[]` stores `label`, `sortOrder`, `hidden`, and `brightnessMode` by composite UID (`{api_id}::{api_model_name}`). This is not guaranteed physical identity: runtime IDs can change, and same-name reconciliation is heuristic. Unmatched disconnected metadata remains saved; Settings filters it only after a successful nonempty live load.

`audioOutputConfigs[]` stores Display DJ settings as `{ id, label, state, sortOrder }`, keyed by the platform's stable endpoint ID. `state` is `enabled`, `disabled`, or `hidden` and defaults to `enabled` for older files; missing sort positions retain default ordering. Empty labels clear only the alias; entries disappear only when label, state, and sort order all return to defaults. Native endpoint names containing `Steam Streaming` are always treated as hidden. The OS remains authoritative for live selection. `lastKnownValues.selectedSpeakerId` separately persists the last popup selection for startup restoration when that endpoint remains available and enabled.

## Conventions

- Before changing commands, preferences, or value bounds, read `AGENTS.md` → Key Conventions.
- Before changing UI presentation, read `FEATURES.md` → Settings & About for font tokens, icons, dropdowns, and platform-specific controls.
- Backend commands return `Result<T, String>`; frontend callers catch invoke failures and retain last-known state.
- Before committing or changing versions, follow `AGENTS.md` → Formatting and Versioning.
- Read [ARCHITECTURE.md](ARCHITECTURE.md) for layer/state maps and [DEV.md](DEV.md) for operational details and historical migration results.

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

Tauri APIs are mocked globally in `src/test/setup.ts`, so tests run without a backend.
Oxlint configuration lives in `.oxlintrc.json`; `npm run lint` is independent of test/build commands and is not invoked by current GitHub workflows. Before changing coverage thresholds, read [AGENTS.md → Coverage thresholds](AGENTS.md#coverage-thresholds-where-they-live).

The dated library upgrade log in `DEV.md` records accepted versions, rejected candidates, compatibility fixes, and validation gaps.
Current pins come from manifests and lockfiles. The post-migration CI snapshot distinguishes passing checks from outstanding warnings and native-runtime gaps.
Wallpaper migration tests exercise a real in-memory ZIP reader, filename-decoding failures, flat image names, and a known MD5 cache identity.
Before upgrading dependencies, follow `AGENTS.md` → Dependency Upgrade Boundaries, including TLS preservation and matching-platform verification.

## CI/CD

Before changing workflows or interpreting checks, read `AGENTS.md` → CI Workflows for matrix, test-execution, and branch-protection requirements.
Before releasing or installing a release, read the matching `.agents/skills/release/SKILL.md` or `.agents/skills/install-app/SKILL.md` workflow.

## Platform Setup (dev)

All platforms: Git and the Node/Rust versions listed in Quick Start.

- **macOS**: `xcode-select --install`; Node/Rust via Homebrew or rustup.
- **Windows**: Node LTS (+ C++ build tools), Rustup, WebView2 (preinstalled on Win11). Tile Snap defaults off; verify Settings detects native Snap, links to **System → Multitasking** while it is on, and removes the warning after it is disabled; no success marker renders. With multiple monitors, verify the blue preview appears only on the display containing the active snap zone. At non-100% display scaling, verify the three bottom-third zones remain visible directly above the taskbar. Also maximize a window, drag its title bar, and verify it restores to normal bounds before following the cursor. Test both standard launch (normal windows work and Settings shows advisory elevation guidance) and **Run as administrator** launch (normal and elevated windows work). Optional status-query failures must not block the app or Settings.
- **Tray popup placement**: test horizontal and vertical multi-monitor layouts with mixed scaling. Left-click each display's tray icon; the popup must stay fully inside that same display, below top-edge trays and above bottom-edge trays. Right-click, choose **Show Window**, and verify it uses that same display-local placement. Enable Debug Logging and verify click records include mouse position, selected screen, and every screen's origin/resolution plus explicit `@Nx` DPI scale; choosing **Show Window** should then log final popup placement. Include a Retina-over-1x vertical stack because Tauri may report overlapping bounds.
- **DPI scaling (beta)**: enable Show DPI Settings, change a display's scale while the popup is open, and verify the popup refits and stays anchored under the tray icon.
- **Popup and settings reliability**: constrain the desktop work area or use a short display and verify the popup scrolls without losing the sticky header. Test every slider and expander by keyboard. Change a setting and immediately close Settings; the final value must persist. Force a backend command failure and verify the slider keeps its last-known value with no status bar; relaunch and confirm last-known brightness, contrast, speaker, and volume are re-applied.
- **Night mode scheduling**: use a fake clock in automated tests. One phase must execute once on the first enabled 60-second check or a transition, duplicate 60-second ticks must do nothing, invalid `HH:MM` values must be rejected, and a failed default phase must retry without recording success.
- **Linux (Ubuntu/Debian)** -- Tauri v2 GUI build dependencies:

  ```bash
  sudo apt install -y git build-essential pkg-config \
    libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev \
    libayatana-appindicator3-dev librsvg2-dev patchelf libxdo-dev
  # brightness / display control
  sudo apt install -y ddcutil i2c-tools brightnessctl
  sudo modprobe i2c-dev
  echo "i2c-dev" | sudo tee /etc/modules-load.d/i2c-dev.conf
  sudo usermod -aG i2c,video $USER   # log out and back in
  ```

  Notes:
  - `libssl-dev` is required by the `openssl-sys` crate (reqwest); missing it fails `cargo check` with an openssl-sys build error.
  - On Ubuntu 24.04 / Mint 22+ use `libayatana-appindicator3-dev` (the older `libappindicator3-dev` package no longer exists). Tray icon support needs it.
  - `libwebkit2gtk-4.1-dev` pulls in WebView for Tauri; on Ubuntu 22.04 also install `libjavascriptcoregtk-4.1-dev` if `pkg-config` can't find it.
  - Tile Snap requires X11. For XFCE manual testing, disable **Window Manager Tweaks → Accessibility → Automatically tile windows when moving toward the screen edge** (or run `xfconf-query -c xfwm4 -p /general/tile_on_move -s false`) so native tiling does not compete with Display DJ's zones.

  Brightness adjustment on Linux uses two paths (`src-tauri/src/core/linux.rs`), each needing its own setup:

  - **Built-in laptop panel** — direct write to `/sys/class/backlight/<device>/brightness`, falling back to the `brightnessctl` CLI when the direct write is permission-denied. Requires: `brightnessctl` installed and your user in the `video` group (the `usermod` above). Verify: `cat /sys/class/backlight/*/max_brightness` works as your user and `brightnessctl get` returns a number.
  - **External monitor (DDC/CI)** — shells out to the `ddcutil` CLI over the `i2c-dev` kernel interface. Requires: `ddcutil`, `i2c-tools`, the `i2c-dev` module loaded at boot, and your user in the `i2c` group (the `modprobe`/`tee`/`usermod` lines above). Verify: `ddcutil detect` lists your monitor as your user; contrast also goes through this path.
  - **Software (gamma) dimming** — uses `xrandr` on X11 (already present on X11 desktops); no extra setup.

Verify: `node --version`, `rustc --version`, and on Linux `ddcutil detect`.

- **Linux (Arch/KDE super alpha)** -- use the release AppImage rather than
  rebuilding on Arch. The release pipeline already builds x64 and ARM64
  AppImages on Ubuntu 22.04, Tauri's compatibility baseline. Install runtime
  display helpers with:

  ```bash
  sudo pacman -S --needed fuse2 ddcutil brightnessctl i2c-tools xorg-xrandr
  sudo modprobe i2c-dev
  sudo usermod -aG i2c,video $USER
  ```

  KDE dark mode uses `plasma-apply-colorscheme`; wallpaper uses
  `plasma-apply-wallpaperimage` from Plasma. Use Plasma X11 for tiling, Tile
  Snap, and the most reliable global shortcuts. Plasma Wayland remains
  diagnostic-only: hardware brightness and `pactl` audio may work, but KWin is
  not a wlroots compositor, so the current `wlr-randr` gamma path does not
  cover it.

  Enable **Debug → Enable Logging** before reproducing. The first dump records
  `/etc/os-release` identity, desktop/session, package format, helper-tool
  availability, selected backends, bounded command stdout/stderr, and DDC /
  display-server probes. Rust panics always land in `crash.log` with a forced
  backtrace and the last 80 debug-log lines. Native Linux crashes still require
  the host's system-coredump tooling for a native stack.

## Known Limitations

| Limitation                  | Details                                                                                               |
| --------------------------- | ----------------------------------------------------------------------------------------------------- |
| DDC/CI not universal        | Budget models and some HDMI connections don't implement it.                                           |
| Built-in HDMI on base M1/M2 | No DDC/CI. Use USB-C/DisplayPort.                                                                     |
| Global shortcuts on Wayland | Wayland restricts global hotkeys. Works on X11.                                                       |
| Tray left-click on Linux    | AppIndicator doesn't always fire left-click; right-click works.                                       |
| Dark mode on other desktops | GNOME, KDE Plasma, and XFCE have dedicated backends; other desktops use best-effort fallback probing. |

## Troubleshooting

- **`command not found: rustc/cargo`** — `source "$HOME/.cargo/env"` (or reopen terminal on Windows).
- **First `tauri dev` slow** — normal; cached in `src-tauri/target/`.
- **Can't find the app** — tray app, no window (see Quick Start).
- **"No displays found"** — Linux: `ddcutil detect` as your user; check `i2c` group membership. Windows: enable DDC/CI in the monitor OSD. macOS: try USB-C/DP instead of HDMI.
- **Dark mode does nothing on Linux** — enable Debug Logging and inspect the detected desktop plus `theme_backend`; KDE needs `plasma-apply-colorscheme`, GNOME needs `gsettings`, and XFCE needs `xfconf-query`.
- **Two snap previews appear on Windows** — disable **Settings → System → Multitasking → Snap windows**, then retry the drag. Display DJ Tile Snap defaults off on Windows for this reason.
- **Task View or Show Desktop shortcut does nothing across an elevation boundary** — system shortcuts dispatch after arrow release. On Windows, Task View releases the trigger modifiers and sends `Win+Tab` at Display DJ's current integrity level; restart Display DJ as administrator only when Windows blocks injected input across that boundary. Show Desktop uses `Shell.ToggleDesktop()` and injects no keys. On macOS, Display DJ sends Mission Control and Show Desktop notifications directly to the Dock; verify the matching `Ctrl+Super+Shift+Up/Down` defaults open Mission Control and toggle the desktop.
- **Two snap previews appear on XFCE/X11** — clear **Window Manager Tweaks → Accessibility → Automatically tile windows when moving toward the screen edge**, then retry the drag.
- **Tile shortcuts silently no-op on Chromium apps (macOS)** — known AX limitation; fixed since v7.0.24 via NSWorkspace fallback. Details in [DEV.md](DEV.md).
- **Maximized/fullscreen window does not resize when tiled** — focused tiling restores Windows maximized windows, exits macOS native or browser/video fullscreen, and removes Linux/X11 EWMH maximize/fullscreen states before applying the layout. Linux waits for the window manager to confirm normal state before saving restore bounds. Minimized windows remain untouched outside Exposé.
- **Audio output selection fails or selects the wrong Windows endpoint** — use tray **Debug → Enable Logging**, reproduce once, then open the log. Compare the requested ID with the listed devices, the confirmed active ID, and the console/multimedia/communications defaults logged before and after each `IPolicyConfig` role update. Unchanged background polls are omitted.
