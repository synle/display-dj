# TODO - Feature Improvements

## Quick Wins

- [x] **Volume presets via keyboard shortcuts** — Add intermediate volume levels (e.g., 25%, 50%, 75%) alongside the existing mute/unmute shortcuts (Shift+F6/F7).
- [ ] **Tray icon reflecting state** — Change the tray icon dynamically based on brightness level or dark/light mode (e.g., dim icon when brightness is low, moon/sun icon for dark/light).

## Medium Effort

- [x] **Brightness scheduling / Night mode** — Auto-dim at sunset or on a schedule (e.g., 9 PM = dark mode + 20% brightness, 7 AM = light mode + 100%).
- [x] **Profiles** — Named presets that bundle any combination of commands (brightness, dark mode, volume). Stored in `preferences.json`, triggered from tray menu, keyboard shortcuts, and UI buttons. 3 defaults: Presentation, Focus, Daylight.
- [x] **Settings UI in the app** — Add a settings panel for min brightness, keyboard shortcuts, and monitor sort/disable/rename instead of editing JSON files manually.
- [x] **Launch at login** — Auto-start the app on system boot. Tauri has an `autostart` plugin for this.

## Larger Features

- [x] **Multi-monitor reordering** — Up/down reorder buttons inline with each monitor in the expanded view instead of editing `sort_order` in JSON.
- [x] **Consolidate monitor configs into preferences** — Eliminated `monitor-configs.json`. Monitor metadata (labels, sort order) now stored in `preferences.json` as a `monitorConfigs` array. Each monitor has a stable composite UID (`{api_id}::{api_model_name}`) that survives reconnections. Unplugged monitors persist in config so labels and sort order are preserved across plug/unplug cycles. Includes one-time migration from old format.
- [x] **Debug logging tray submenu** — Replaced "Open Debug Log" menu item with a "Debug" submenu containing Enable Logging, Disable Logging, and Open Debug Log. Removed debug logging checkbox from Settings panel.
- [ ] **Ambient light adaptation** — Auto-adjust brightness based on ambient light sensor. See [research notes](#ambient-light-adaptation-research) below.
- [ ] **Blue light filter / Color temperature** — A "night shift" style warm color temperature control, if display-dj CLI supports gamma/color adjustments.
- [ ] **Keyboard shortcut editor UI** — Visual editor for hotkeys instead of editing `preferences.json`. Show current bindings, let users record new ones.

## New Feature Ideas

### Quick Wins

- [ ] **Tray tooltip showing current state** — On hover, show brightness %, volume %, and dark/light mode in the tray tooltip text instead of a static label. Tauri's `TrayIconBuilder` supports `.tooltip()` — update it on each state change. Gives users a glance at current levels without opening the popup.
- [ ] **Per-monitor quick presets** — Add small 0/25/50/75/100% buttons below each monitor slider in expanded view for one-tap brightness setting. Avoids the imprecision of dragging a slider when you just want a round number. Render as a row of compact pill buttons styled to match the existing UI.
- [ ] **Export / Import settings** — Backup and restore `preferences.json` to/from a user-chosen file. Useful when migrating to a new machine, syncing a work and home setup, or sharing a multi-monitor config with a teammate. Use Tauri's `dialog` plugin for the native file picker.
- [ ] **Confirm before Reset to Default** — Show a confirmation dialog before wiping all settings on "Reset to Default" to prevent accidental resets. Currently one mis-click in the tray menu destroys all customizations (shortcuts, profiles, monitor names). A simple "Are you sure?" dialog via Tauri's `dialog::ask` would prevent this.
- [ ] **Volume presets in UI** — Quick mute/25%/50%/75%/100% buttons below the volume slider, mirroring the per-monitor preset idea. Keyboard shortcuts already support volume presets (Shift+F10/F11/F12), but the UI has no equivalent — this closes that gap for mouse-driven users.

### Medium Effort

- [ ] **Scheduled profiles** — Let users assign a time-of-day schedule to any profile (not just night mode). E.g., "Focus" at 9 AM, "Presentation" at 2 PM, "Daylight" at 6 PM. Generalizes night mode into a full schedule system. The existing 60-second timer loop in `lib.rs` already checks time — extend it to evaluate a list of `(time, profile_index)` entries. Profiles already bundle arbitrary commands, so this reuses all existing execution logic.
- [ ] **Idle-based dimming** — Auto-dim brightness after a configurable period of inactivity (e.g., 5 min idle → 10% brightness). Restore on mouse/keyboard activity. Saves energy and extends external monitor life, especially for DDC/CI monitors that don't have their own idle dimming. On macOS, poll `CGEventSourceSecondsSinceLastEventType`; on Windows, `GetLastInputInfo`; on Linux, `xprintidle` or `org.freedesktop.ScreenSaver.GetSessionIdleTime`.
- [ ] **Battery-aware brightness** — On laptops, auto-reduce brightness when unplugged or below a battery threshold. Configurable in settings with a battery % trigger and a target brightness level. The `battery` Rust crate provides cross-platform charge level and AC/battery state. Could pair with profiles — e.g., activate "Focus" profile when unplugged.
- [ ] **Brightness fade transitions** — Smooth animated transitions when changing brightness (e.g., fade from 100% to 20% over 500ms) instead of instant jumps. Makes night mode and profile switches feel less jarring, especially in dark rooms where a sudden brightness change is blinding. Implement as a series of small DDC/CI steps on a timer in the backend — the frontend slider can stay instant for responsiveness.
- [ ] **Monitor grouping** — Group monitors (e.g., "Desk Left", "Desk Right") and control grouped monitors together with a single slider. Useful for users with 3+ monitors where some share a purpose (e.g., two side monitors for reference, one center for focus). Store group assignments in `monitor-configs.json` and render a group slider above the individual sliders in the expanded view.
- [ ] **Notification on profile/schedule activation** — Show a system notification when a profile or scheduled change activates, so the user knows why brightness/dark mode just changed. Without this, night mode kicking in at 9 PM can feel like a bug if the user forgot they enabled it. Use Tauri's `notification` plugin — already available in the v2 plugin ecosystem.

### Larger Features

- [ ] **Per-app dark mode rules** — Auto-toggle dark/light mode based on the foreground application (e.g., always light mode when Figma is active, dark mode for terminal/IDE). Poll the active window at a low frequency (every 5-10s) using `NSWorkspace.frontmostApplication` on macOS, `GetForegroundWindow` on Windows, or `_NET_ACTIVE_WINDOW` on Linux. Store rules as a list of `(app_name, dark_mode: bool)` in preferences.
- [ ] **Remote control via local web UI** — Expose a lightweight HTTP interface on the local network so users can adjust brightness/volume/dark mode from a phone or tablet. The display-dj sidecar already runs an HTTP server — extend it (or add a second endpoint) with a simple HTML page served at `http://<local-ip>:<port>/`. Useful for home theater setups or adjusting a docked laptop from across the room.
- [ ] **CLI companion commands** — Allow controlling the running app from the terminal (e.g., `display-dj2 set-brightness 50`, `display-dj2 activate-profile Focus`). Useful for scripting, automation, and integration with tools like Raycast, Alfred, or shell aliases. Implement by having the CLI send HTTP requests to the running app's sidecar server, or use Tauri's single-instance plugin to forward args to the running instance.
- [ ] **Display hot-plug handling** — Detect when monitors are connected/disconnected and auto-apply saved configs (brightness, name, sort order) for recognized monitors without manual refresh. Currently, plugging in a monitor requires reopening the popup to see it. Use a platform event listener or poll `get_all` on a timer, diff against the known monitor list, and emit `monitors-changed` when the set changes. Recognize returning monitors by their display ID and restore their last-known brightness and name.

### Cross-Platform Tiling Window Manager

**Goal:** Build a cross-platform tiling window manager supporting macOS, Windows, and Linux.

#### Platform Support Matrix

| Platform                    | API / Mechanism                                          | Rust Crate                           | Difficulty  | Notes                                                                                                                                                                    |
| --------------------------- | -------------------------------------------------------- | ------------------------------------ | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **macOS**                   | Accessibility API (`AXUIElement`)                        | `accessibility-sys`, `core-graphics` | Medium      | Proven by AeroSpace, GlazeWM. Needs Accessibility permission. No SIP disable needed.                                                                                     |
| **Windows**                 | Win32 (`EnumWindows`, `SetWindowPos`, `SetWinEventHook`) | `windows` (official Microsoft)       | Medium      | Proven by komorebi, GlazeWM. No elevation needed. Invisible DWM borders need compensation.                                                                               |
| **Linux X11**               | X11/XCB + EWMH protocol                                  | `x11rb`                              | Easy        | Most permissive — any client can manipulate any window. Covers all X11 desktops.                                                                                         |
| **Linux Wayland: Sway**     | i3-compatible IPC (JSON over Unix socket)                | `swayipc`                            | Medium      | Full move/resize/layout control. But Sway users already have tiling built in.                                                                                            |
| **Linux Wayland: Hyprland** | `hyprctl` IPC socket                                     | `hyprland`                           | Medium      | Good IPC. But Hyprland users already have tiling built in.                                                                                                               |
| **Linux Wayland: KDE**      | D-Bus + KWin scripts (JS/TS)                             | `zbus`                               | Medium-Hard | Needs a KWin script sidecar. Polonium is the reference implementation.                                                                                                   |
| **Linux Wayland: GNOME**    | GNOME Shell extension (JS) only                          | `zbus` + JS extension sidecar        | Hard        | No external IPC. Must write a JS extension that runs inside gnome-shell. Breaks on every major GNOME release. Largest user base but most hostile to external WM control. |

#### Key Findings

- **Only one cross-platform tiling WM exists:** GlazeWM (Rust, macOS + Windows). Nothing covers all three OSes.
- **GlazeWM's architecture is the model:** Rust workspace with a `wm-platform` crate defining traits (`NativeWindow`, `NativeMonitor`), `#[cfg(target_os)]`-gated platform backends.
- **Wayland has no universal window management protocol.** Each compositor has its own IPC, requiring separate backends.
- **The GNOME problem:** GNOME deliberately prevents external window management. Requires a GNOME Shell JS extension sidecar communicating with the Rust daemon via D-Bus. Fragile and high-maintenance.
- **The Sway/Hyprland irony:** Easiest Wayland backends to implement, but those users already chose those compositors for tiling. The users who need tiling most (GNOME, KDE) are the hardest to support.
- **Global hotkeys:** `global-hotkey` crate (from Tauri) covers macOS + Windows + Linux X11. Wayland has no standard global hotkey API — compositor must handle it.

#### Recommended Priority Order

1. **Phase 1:** macOS + Windows + Linux X11 — covers most users, all proven approaches
2. **Phase 2:** KDE Wayland — moderate effort, users actually want tiling help
3. **Phase 3:** Evaluate GNOME Wayland — largest user base but highest maintenance cost. Users who want tiling on GNOME already use Forge/PaperWM as native extensions.
4. **Skip or deprioritize:** Sway + Hyprland — those users already have tiling built into their compositor

#### Tiling Layouts & Requirements

##### Supported Layouts

| Layout                   | Position            | Size                    |
| ------------------------ | ------------------- | ----------------------- |
| **Left half**            | Left edge           | 50% width, 100% height  |
| **Right half**           | Right edge          | 50% width, 100% height  |
| **Top half**             | Top edge            | 100% width, 50% height  |
| **Bottom half**          | Bottom edge         | 100% width, 50% height  |
| **Left third**           | Left edge           | 33% width, 100% height  |
| **Center third**         | Centered            | 33% width, 100% height  |
| **Right third**          | Right edge          | 33% width, 100% height  |
| **Top third**            | Top edge            | 100% width, 33% height  |
| **Middle third**         | Centered vertically | 100% width, 33% height  |
| **Bottom third**         | Bottom edge         | 100% width, 33% height  |
| **Left two-thirds**      | Left edge           | 67% width, 100% height  |
| **Right two-thirds**     | Right edge          | 67% width, 100% height  |
| **Top-left quarter**     | Top-left corner     | 50% width, 50% height   |
| **Top-right quarter**    | Top-right corner    | 50% width, 50% height   |
| **Bottom-left quarter**  | Bottom-left corner  | 50% width, 50% height   |
| **Bottom-right quarter** | Bottom-right corner | 50% width, 50% height   |
| **Maximize**             | Full screen         | 100% width, 100% height |

##### Tiling Preferences (stored in app preferences)

All tiling settings live in the app's `preferences.json` alongside existing display-dj settings.

| Setting                        | Default | Description                                                                                                                                                     |
| ------------------------------ | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`tilingHalfRatio`**          | `50`    | Percentage for half splits. Affects halves and quarter corners.                                                                                                 |
| **`tilingThirdRatio`**         | `33`    | Percentage for third splits. Center/middle = `100 - 2×third`.                                                                                                   |
| **`tilingGap`**                | `0`     | Gap in pixels between tiled windows and screen edges. 0 = flush, 8-16 = nice spacing.                                                                           |
| **`tilingEdgeTriggerSize`**    | `5`     | Pixel width of the hot zone along screen edges/corners for mouse snapping. Larger = easier to trigger.                                                          |
| **`tilingCornerTriggerSize`**  | `50`    | Pixel size of the corner hot zone (square). Must be larger than edge trigger so corners take priority.                                                          |
| **`tilingAnimationDuration`**  | `150`   | Milliseconds for the snap animation. 0 = instant.                                                                                                               |
| **`tilingShowPreview`**        | `true`  | Show a translucent overlay preview when dragging to an edge/corner before dropping.                                                                             |
| **`tilingExcludedApps`**       | `[]`    | List of app names/bundle IDs to never tile (e.g., system dialogs, floating utilities). These windows are ignored by both keyboard shortcuts and mouse snapping. |
| **`tilingRespectDockMenuBar`** | `true`  | Tile within usable screen area (excluding dock/taskbar/menu bar). When false, tile over the full screen.                                                        |
| **`tilingMonitorOrder`**       | `[]`    | Ordered list of monitor IDs defining the cycle order for repeat-to-cycle-displays. Empty = use OS default left-to-right order.                                  |
| **`tilingRestoreOnUntile`**    | `true`  | When a tiled window is dragged away from its tiled position, restore its original size/position from before it was tiled.                                       |
| **`tilingEnabled`**            | `true`  | Master toggle to enable/disable all tiling features.                                                                                                            |

Examples for split ratios:

- `tilingHalfRatio: 60` → "left half" = 60% width, "right half" = 40%. Quarters use 60%/40% splits.
- `tilingThirdRatio: 40` → "left third" = 40%, "center third" = 20% (100 - 40 - 40), "right third" = 40%. "Left two-thirds" = 60%.

The center/middle third is always the remainder: `100% - (2 × third ratio)`. The setting controls the outer slices.

##### Tiling Commands

Each tiling action is a command that can be bound to a keyboard shortcut, just like existing display-dj commands (brightness, volume, dark mode). Users bind them in `preferences.json` under `keyBindings`.

| Command                     | Description                                           |
| --------------------------- | ----------------------------------------------------- |
| `tile_left_half`            | Tile focused window to left half                      |
| `tile_right_half`           | Tile focused window to right half                     |
| `tile_top_half`             | Tile focused window to top half                       |
| `tile_bottom_half`          | Tile focused window to bottom half                    |
| `tile_left_third`           | Tile focused window to left third                     |
| `tile_center_third`         | Tile focused window to center third                   |
| `tile_right_third`          | Tile focused window to right third                    |
| `tile_top_third`            | Tile focused window to top third                      |
| `tile_middle_third`         | Tile focused window to middle third                   |
| `tile_bottom_third`         | Tile focused window to bottom third                   |
| `tile_left_two_thirds`      | Tile focused window to left two-thirds                |
| `tile_right_two_thirds`     | Tile focused window to right two-thirds               |
| `tile_top_left_quarter`     | Tile focused window to top-left corner                |
| `tile_top_right_quarter`    | Tile focused window to top-right corner               |
| `tile_bottom_left_quarter`  | Tile focused window to bottom-left corner             |
| `tile_bottom_right_quarter` | Tile focused window to bottom-right corner            |
| `tile_maximize`             | Maximize focused window                               |
| `tile_restore`              | Restore focused window to pre-tiled size and position |

All commands follow the repeat-to-cycle-displays behavior: if the window is already in the target layout, the command moves it to the next display in the same layout.

##### Repeat-to-Cycle-Displays Behavior

When a keyboard shortcut is triggered and the window is **already in that exact layout on the current display**, the window moves to the **next display** in the same layout position. Repeated presses cycle through all connected displays and then wrap back to the first. Cycle order follows `tilingMonitorOrder` if set, otherwise OS default left-to-right.

Example with 3 monitors:

1. Press "left half" → window snaps to left half of **Monitor 1**
2. Press "left half" again → window moves to left half of **Monitor 2**
3. Press "left half" again → window moves to left half of **Monitor 3**
4. Press "left half" again → window moves back to left half of **Monitor 1**

This means every keyboard shortcut doubles as a "move to next monitor" command when the window is already in the target layout. No separate "send to monitor" hotkeys needed.

##### Mouse Edge Snapping

Dragging a window to a screen edge or corner triggers a snap preview (like macOS Sequoia / Windows 11 Snap). On drop, the window tiles to that zone. Preview overlay is controlled by `tilingShowPreview`.

| Mouse Position          | Resulting Layout     |
| ----------------------- | -------------------- |
| **Left edge**           | Left half            |
| **Right edge**          | Right half           |
| **Top edge**            | Maximize             |
| **Top-left corner**     | Top-left quarter     |
| **Top-right corner**    | Top-right quarter    |
| **Bottom-left corner**  | Bottom-left quarter  |
| **Bottom-right corner** | Bottom-right quarter |

Corner detection uses `tilingCornerTriggerSize` (larger zone) and takes priority over edge detection (`tilingEdgeTriggerSize`). This prevents top-left corner from accidentally triggering maximize.

Mouse snapping does **not** trigger the repeat-to-cycle-displays behavior — that is keyboard-only. Mouse snapping always targets the display the cursor is on.

When a tiled window is dragged away from its snapped position and `tilingRestoreOnUntile` is enabled, the window returns to its original size/position from before it was tiled.

##### Nice to Have: Exposé-Style Window Spread (not required for Phase 1)

A command that lays out all open windows in a grid so you can see everything at once — like macOS Exposé / Mission Control, but using tiling positions instead of fancy animations.

**Command:** `tile_expose`

**Behavior:**

1. **Group windows by app** — e.g., all VS Code windows together, all Chrome windows together, all Terminal windows together.
2. **Count total windows** across all groups.
3. **Compute grid size** — find the best-fit grid (rows × columns) for the total window count on the current display. Aim for roughly square cells. E.g., 6 windows → 3×2 grid, 9 windows → 3×3, 5 windows → 3×2 with one empty cell.
4. **Lay out top-left to bottom-right** — place windows one by one into the grid, filling left-to-right, top-to-bottom. Windows within the same app group are placed adjacent to each other.
5. **Save all original positions** — so `tile_restore` (or pressing `tile_expose` again) can put everything back.

**Example** — 8 windows (3 VS Code, 3 Chrome, 2 Terminal) on a 1920×1080 display:

```
┌──────────┬──────────┬──────────┐
│ VS Code 1│ VS Code 2│ VS Code 3│   row 1 (640×540 each)
├──────────┼──────────┼──────────┤
│ Chrome 1 │ Chrome 2 │ Chrome 3 │   row 2
├──────────┼──────────┴──────────┤
│Terminal 1│ Terminal 2│          │   row 3 (last cell empty)
└──────────┴──────────┴──────────┘
```

**Grid sizing heuristic:**

- `cols = ceil(sqrt(window_count))`
- `rows = ceil(window_count / cols)`
- Cell size = `(screen_width / cols)` × `(screen_height / rows)`
- Respects `tilingGap` between cells

**Toggle behavior:** Pressing `tile_expose` when already in exposé mode restores all windows to their original positions (same as `tile_restore` but for all windows at once).

#### Reference Projects

| Project       | Platform        | Language | Stars | Key Takeaway                                                                   |
| ------------- | --------------- | -------- | ----- | ------------------------------------------------------------------------------ |
| **GlazeWM**   | macOS + Windows | Rust     | ~12k  | Only cross-platform tiling WM. Architecture reference.                         |
| **komorebi**  | Windows         | Rust     | ~15k  | Best Rust Win32 tiling reference.                                              |
| **AeroSpace** | macOS           | Swift    | ~20k  | Best macOS approach (no SIP, no private APIs, virtual workspace emulation).    |
| **yabai**     | macOS           | C        | ~29k  | Most feature-complete macOS WM but uses private APIs and optional SIP disable. |
| **Forge**     | GNOME Wayland   | JS       | —     | GNOME Shell extension reference for tiling.                                    |
| **Polonium**  | KDE Wayland     | TS/JS    | —     | KWin script reference for tiling.                                              |

### Ambient Light Adaptation Research

**TLDR:** Feasible but fragile on macOS (undocumented IOKit, could break any update), solid on Windows (official UWP API), hit-or-miss on Linux (sysfs, hardware-dependent). No cross-platform Rust crate exists — needs per-platform code. Should be opt-in with graceful degradation.

#### macOS — IORegistry `CurrentLux` property (best approach)

No public Apple API exists. The most portable approach scans all IOServices for a `CurrentLux` property:

- Enumerate via `IOServiceGetMatchingServices` with `IOServiceMatching("IOService")`
- Check each service for `IORegistryEntryCreateCFProperty(service, "CurrentLux")`
- Returns lux as a float — confirmed working on M3 Pro (macOS Sequoia) without entitlements or permissions

The underlying service name varies by hardware (`AppleLMUController` on Intel, `AppleSPUVD6286` on M3, etc.), so scanning beats hardcoding. Only works on Macs with built-in displays (MacBooks, iMacs) — no sensor on Mac Mini/Studio/Pro or external monitors.

**Risk:** Entirely undocumented private API surface. Apple could add TCC restrictions or change the property name at any time.

Use the `io-kit-sys` crate for IOKit FFI bindings from Rust.

#### Windows — WinRT `LightSensor` API

The cleanest API of the three platforms — works from regular desktop apps (no UWP required), no permissions or manifest entries needed, no user prompts:

- `LightSensor::GetDefault()` — returns the sensor or null if no hardware
- `ReadingChanged` event for lux values, or `GetCurrentReading()` for one-shot
- Rust: `windows` crate with `Devices_Sensors` feature flag

**The catch: hardware availability.** ALS is not a Windows hardware requirement. OEMs include it optionally:

- **~60-70% of premium laptops** (Surface, ThinkPad X1, XPS 13/15, HP EliteBook) — most reliable
- **~20-40% of mid-range laptops** — inconsistent, depends on SKU
- **~5-10% of budget laptops** — almost never
- **0% of desktops** — no desktop monitor exposes ALS through the Windows sensor framework
- **~60-80% of 2-in-1 convertibles/tablets** — higher due to tablet-mode use case

`LightSensor::GetDefault()` returning null is the expected case for many users. No WMI fallback exists — the deprecated COM `ISensorManager` accesses the same hardware and will also return nothing.

#### Linux — sysfs IIO subsystem (weakest platform)

Read `/sys/bus/iio/devices/iio:device*/in_illuminance_raw` — a simple file read (no root required, files are 0644). Multiply by `in_illuminance_scale` for real lux. Just `std::fs::read_to_string` from Rust.

**Reality check: this is the least reliable platform for ALS.** Even when laptop hardware has a sensor, the Linux kernel driver often doesn't support it or requires manual ACPI hacks to enable:

- **ThinkPads** — some models need manual ACPI calls to enable the `acpi-als` driver
- **Dell XPS** — mixed, some 2020+ models work with recent kernels, many don't
- **Framework laptops** — gen 2+ work well (kernel 6.1+), one of the most Linux-friendly
- **HP EliteBook** — rarely works, HP ACPI tables don't expose the sensor to Linux
- **Surface under linux-surface patches** — community-maintained, not upstream

The most telling evidence: **both** major Linux auto-brightness projects were designed webcam-first:

- **Clight** (789 stars) — tagline: "turns your webcam into a light sensor." ALS was added later as an enhancement. Supports a priority chain: ALS → USB light sensor → PipeWire screen capture → webcam → custom scripts.
- **wluma** (909 stars, written in Rust) — supports `[als.iio]`, `[als.webcam]`, `[als.time]`, `[als.none]`. Webcam is the expected path for most users.

The alternative to direct sysfs is **iio-sensor-proxy**, a D-Bus daemon (`net.hadess.SensorProxy`) used by GNOME/KDE for auto-brightness. But it only wraps IIO — if the IIO driver doesn't work, it doesn't either.

#### ALS hardware prevalence summary

| Platform | ALS reliability                                  | Camera fallback role                |
| -------- | ------------------------------------------------ | ----------------------------------- |
| macOS    | Works on all MacBooks/iMacs (undocumented IOKit) | Only needed for Mac Mini/Studio/Pro |
| Windows  | ~50% of laptops, 0% desktops                     | Essential fallback                  |
| Linux    | ~10-20% of laptops with working drivers          | **Primary path** for most users     |

**Camera fallback is a necessity on all platforms, not a nice-to-have.** On Linux especially, the camera is the realistic default — ALS is the bonus.

#### Alternative: Camera-based ambient detection (no ALS required)

Use the webcam as a light sensor — works on any machine with a camera, including desktops with external webcams where no ALS exists. Capture a single frame to an in-memory buffer, compute average luminance (a single number), and **immediately drop the buffer. No image is ever written to disk, cached, or persisted — only the computed luminance value is kept.**

**When to capture (two trigger points only, no background polling):**

1. **App startup / login** — one frame during the login flurry when the user isn't watching the camera indicator. Sets initial brightness for the session.
2. **Tray popup opened** — one frame while the user is already interacting with the app. Show a suggestion ("It looks dark — dim to 20%?") or auto-adjust if opted in.

**Why this works:**

- Green camera light is brief and occurs at moments the user expects activity (login, opening the app) — not random background flashes
- No background polling — zero CPU/battery cost when idle
- Cross-platform — every laptop has a webcam, camera APIs are well-established and official
- Works on Mac Mini/Studio/Pro with external webcams (where there's no ALS)
- For a "dark vs light" binary decision, average frame luminance is good enough

**Downsides:**

- Requires camera permission (but use case is clear and defensible)
- Only adjusts at login and when popup is opened — not real-time like ALS
- Camera contention if user is on a video call (capture should gracefully skip if camera is busy)
- Accuracy is lower than a real ALS — measures light reflected off the room, not light hitting the screen

**Privacy contract:** Frame is captured to memory only, luminance is computed, buffer is dropped. Nothing is stored, written to disk, or sent anywhere. The camera is active for a fraction of a second.

#### Implementation notes

- Make strictly opt-in in settings
- Try ALS first, fall back to camera-based detection, fall back to schedule (night mode)
- Detect sensor/camera availability at startup, hide feature if absent
- For ALS path: poll every ~2-5 seconds (no event-based API on macOS)
- For camera path: capture only at startup and popup open — no background polling
- Keep ALS and camera code isolated for easy updates when platform APIs change
