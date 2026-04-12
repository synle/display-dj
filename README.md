# display-dj

A cross-platform desktop app for controlling monitor brightness, dark mode, and volume -- all from one system tray popup. Works with both built-in laptop displays and external monitors via DDC/CI. Supports **macOS**, **Windows**, and **Linux**.

## Downloads

Grab the latest release from the [Releases](../../releases) page.

| Platform | File |
|---|---|
| macOS Apple Silicon (ARM64) | `Display DJ_x.x.x_aarch64.dmg` |
| macOS Intel (x64) | `Display DJ_x.x.x_x64.dmg` |
| Windows (x64) | `Display DJ_x.x.x_x64-setup.exe` / `.msi` |
| Linux (x64) | `Display DJ_x.x.x_amd64.deb` / `.AppImage` |

## Features

- **Monitor brightness** -- one slider for all monitors, or expand for individual control
- **Dark mode toggle** -- system-wide dark/light switch
- **Volume control** -- system volume slider with mute indicator
- **Night mode schedule** -- auto-set brightness and dark/light mode on a time schedule (e.g., dim at 9 PM, bright at 7 AM)
- **Settings panel** -- in-app UI for min brightness, brightness/contrast delta, and night mode schedule
- **Global keyboard shortcuts** -- work even when the app isn't focused (configurable via settings or `preferences.json`)
- **Monitor renaming** -- click a display name to rename it
- **System tray app** -- lives in your menu bar / system tray, no dock/taskbar clutter

## Quick Start (Development)

**Prerequisites**: [Node.js](https://nodejs.org) 18+, [Rust](https://www.rust-lang.org/tools/install) 1.77+

```bash
git clone <repo-url>
cd display-dj2
npm install
npx tauri dev
```

The first build takes a few minutes while Rust compiles from source. After that, incremental builds take ~5-15 seconds.

The app appears in your **system tray** (not as a regular window):
- **macOS**: menu bar, top-right
- **Windows**: system tray, bottom-right (click `^` if hidden)
- **Linux**: top panel (may need the [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/))

> **Platform-specific dependencies** (external monitor tools, Tauri build libraries, etc.) are covered in **[CONTRIBUTING.md](CONTRIBUTING.md)**.

## Common Commands

| Command | What it does |
|---|---|
| `npx tauri dev` | Run the full app in dev mode with hot reload |
| `npx tauri build` | Production build (binary + installer) |
| `npm test` | Run frontend tests |
| `cd src-tauri && cargo test` | Run backend tests |

## Configuration

Config files live in `~/Library/Application Support/display-dj/` (macOS), `%APPDATA%\display-dj\` (Windows), or `~/.config/display-dj/` (Linux).

- **`preferences.json`** -- keyboard shortcuts, min brightness, night mode schedule, profiles, per-monitor metadata (labels, sort order)

### Default Keyboard Shortcuts

| Keys | Command |
|---|---|
| Shift + Escape | Toggle Dark Mode |
| Shift + F1 | Brightness 10% + Dark Mode |
| Shift + F2 | Brightness 100% + Light Mode |
| Shift + F3-F5 | Brightness 0% / 50% / 100% |
| Shift + F10-F12 | Volume 0% / 10% / 100% |

## Known Issues

- Not every external monitor supports DDC/CI (some budget models and certain HDMI connections)
- Built-in HDMI on base M1/M2 Macs doesn't support DDC/CI -- use USB-C/DisplayPort
- Linux global shortcuts may not work under Wayland (X11 works fine)

## Contributing

See **[CONTRIBUTING.md](CONTRIBUTING.md)** for the full development setup (per-platform), architecture, testing, project structure, and code walkthrough.

## Bug Reports & Suggestions

Use the [Issues](../../issues) page. Please include your OS version, monitor model(s), and connection type.

## Tech Stack

[Tauri v2](https://v2.tauri.app/) (Rust) + React 18 + TypeScript + Vite 6
