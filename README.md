[![Build](https://github.com/synle/display-dj/actions/workflows/build.yml/badge.svg)](https://github.com/synle/display-dj/actions/workflows/build.yml)

# Display DJ

A cross-platform desktop system tray app for controlling monitor brightness, contrast, dark mode, volume, and more -- all from one popup. Works with both built-in laptop displays and external monitors via DDC/CI. Supports **macOS**, **Windows**, and **Linux**.

## Features

- **Brightness control** -- a single slider to adjust all monitors at once, or expand to control each monitor individually
- **Contrast control** -- DDC/CI contrast adjustment for external monitors (enable in Settings)
- **Dark mode toggle** -- system-wide dark/light mode switch
- **Volume control** -- system volume slider with mute indicator
- **Keep Awake** -- prevent your system from sleeping with a single toggle (macOS, Windows, Linux)
- **Night mode schedule** -- automatically set brightness and dark/light mode on a time-based schedule (e.g., dim at 9 PM, bright at 7 AM)
- **Profiles** -- save and restore preset combinations of brightness, contrast, dark mode, and volume
- **Global keyboard shortcuts** -- work even when the app isn't focused; fully configurable
- **Monitor renaming** -- click any display name to give it a custom label
- **Window tiling** (macOS) -- tile windows to halves, thirds, two-thirds, quarters, or maximize via keyboard shortcuts or tray menu. See [Window Tiling](#window-tiling-macos) below
- **Settings panel** -- configure min brightness, contrast visibility, monitor ordering, night mode schedule, tiling, and launch at login
- **System tray app** -- lives in your menu bar / system tray with no dock or taskbar clutter

## Download & Install

Grab the latest release from the **[Releases](../../releases)** page.

### macOS

| Chip          | File                           |
| ------------- | ------------------------------ |
| Apple Silicon | `Display DJ_2.2.0_aarch64.dmg` |
| Intel         | `Display DJ_2.2.0_x64.dmg`     |

1. Download the `.dmg` for your chip
2. Open the `.dmg` and drag **Display DJ** into your **Applications** folder
3. Launch **Display DJ** from Applications -- it will appear in your **menu bar** (top-right)

> **First launch note:** macOS Gatekeeper may show _"Display DJ is damaged and can't be opened"_ or _"unidentified developer"_ because the app is not notarized via the App Store. See the [macOS Gatekeeper fix](#macos-gatekeeper-fix) below.

### Windows

| Architecture | File                             |
| ------------ | -------------------------------- |
| x64          | `Display DJ_2.2.0_x64-setup.exe` |
| x64          | `Display DJ_2.2.0_x64_en-US.msi` |

1. Download either the `.exe` installer or the `.msi`
2. Run the installer and follow the prompts
3. Launch **Display DJ** -- it will appear in your **system tray** (bottom-right; click `^` if hidden)

### Linux

| Format   | File                              |
| -------- | --------------------------------- |
| Debian   | `Display DJ_2.2.0_amd64.deb`      |
| AppImage | `Display DJ_2.2.0_amd64.AppImage` |
| RPM      | `Display.DJ-2.2.0-1.x86_64.rpm`   |

1. Install via your preferred format:

   ```bash
   # Debian / Ubuntu
   sudo dpkg -i "Display DJ_2.2.0_amd64.deb"

   # RPM-based (Fedora, etc.)
   sudo rpm -i Display.DJ-2.2.0-1.x86_64.rpm

   # AppImage (no install needed)
   chmod +x "Display DJ_2.2.0_amd64.AppImage"
   ./"Display DJ_2.2.0_amd64.AppImage"
   ```

2. Install the required display-control dependencies:
   ```bash
   sudo apt install ddcutil brightnessctl i2c-tools
   sudo modprobe i2c-dev
   sudo usermod -aG i2c $USER
   ```
3. The app appears in your **top panel** (you may need the [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/) on GNOME)

## Configuration

Config files are stored in:

- **macOS**: `~/Library/Application Support/display-dj/`
- **Windows**: `%APPDATA%\display-dj\`
- **Linux**: `~/.config/display-dj/`

The main config file is **`preferences.json`** -- it holds keyboard shortcuts, min brightness, night mode schedule, profiles, and per-monitor metadata (labels, sort order).

### Default Keyboard Shortcuts

| Keys            | Action                       |
| --------------- | ---------------------------- |
| Shift + Escape  | Toggle Dark Mode             |
| Shift + F1      | Brightness 10% + Dark Mode   |
| Shift + F2      | Brightness 100% + Light Mode |
| Shift + F3-F5   | Brightness 0% / 50% / 100%   |
| Shift + F10-F12 | Volume 0% / 10% / 100%       |

**Window Tiling** (macOS only):

| Keys                  | Action                           |
| --------------------- | -------------------------------- |
| Ctrl + Shift + Left   | Left Third                       |
| Ctrl + Shift + Right  | Right Third                      |
| Ctrl + Shift + Up     | Left Two-Thirds                  |
| Ctrl + Shift + Down   | Right Two-Thirds                 |
| Ctrl + Shift + D/C/G  | Left/Center/Right Third          |
| Ctrl + Shift + I/O    | Top-Left/Top-Right Quarter       |
| Ctrl + Shift + K/L    | Bottom-Left/Bottom-Right Quarter |
| Ctrl + Shift + M or / | Maximize                         |

## Window Tiling (macOS)

Window tiling lets you snap windows to halves, thirds, two-thirds, quarters, or maximize using keyboard shortcuts or the **Tiling** submenu in the tray icon's right-click menu.

### Enabling Tiling

1. Open the tray menu and go to **Tiling > Enable Tiling**, or toggle **Enable Window Tiling** in the Settings panel
2. Grant **Accessibility permission** (required for tiling to move/resize other apps' windows):
   - Open **System Settings > Privacy & Security > Accessibility**
   - Click the **+** button
   - Add **Display DJ** (from `/Applications/Display DJ.app`)
   - Make sure the toggle next to it is **on**
   - If running in development mode (`npx tauri dev`), add your **terminal app** (e.g., iTerm, Terminal.app) instead

> **Note:** Tiling will not work without Accessibility permission. The Settings panel shows a warning with a link to these instructions if permission is missing.

### Tiling Preferences

Tiling settings are stored in `preferences.json` under the `tiling` key:

| Setting      | Default | Description                                              |
| ------------ | ------- | -------------------------------------------------------- |
| `enabled`    | `true`  | Master toggle for all tiling features                    |
| `halfRatio`  | `50`    | Percentage for half splits (affects halves and quarters) |
| `thirdRatio` | `33`    | Percentage for third splits (center = 100 - 2 x third)   |
| `gap`        | `0`     | Padding in points around the tiling area                 |

## Known Issues

- Not every external monitor supports DDC/CI (some budget models and certain HDMI connections)
- Built-in HDMI on base M1/M2 Macs doesn't support DDC/CI -- use USB-C or DisplayPort instead
- Linux global shortcuts may not work under Wayland (X11 works fine)

## macOS Gatekeeper Fix

macOS Gatekeeper quarantines apps downloaded outside the App Store by setting an extended attribute (`com.apple.quarantine`) on the `.app` bundle. This causes the _"app is damaged and can't be opened"_ or _"unidentified developer"_ error when you try to launch the app.

To fix this, open **Terminal** and run:

```bash
xattr -cr "/Applications/Display DJ.app"
```

This recursively clears the quarantine flag so macOS allows the app to run. You only need to do this once after the initial install (or after updating to a new version).

## Tech Stack

[Tauri v2](https://v2.tauri.app/) (Rust) + React 18 + TypeScript + Vite 6 + [display-dj CLI](https://github.com/synle/display-dj-cli)

Display and dark mode operations are handled by the bundled [display-dj CLI](https://github.com/synle/display-dj-cli) sidecar -- no external tools need to be installed on macOS or Windows.

## Contributing

See **[CONTRIBUTING.md](CONTRIBUTING.md)** for the full development setup, project structure, testing, and platform guides. See **[DEV.md](DEV.md)** for the architecture deep-dive.

## Bug Reports & Suggestions

Use the [Issues](../../issues) page. Please include your OS version, monitor model(s), and connection type.
