# TODO - Feature Improvements

## Quick Wins

- [x] **Volume presets via keyboard shortcuts** — Add intermediate volume levels (e.g., 25%, 50%, 75%) alongside the existing mute/unmute shortcuts (Shift+F6/F7).
- [ ] **Tray icon reflecting state** — Change the tray icon dynamically based on brightness level or dark/light mode (e.g., dim icon when brightness is low, moon/sun icon for dark/light).

## Medium Effort

- [x] **Brightness scheduling / Night mode** — Auto-dim at sunset or on a schedule (e.g., 9 PM = dark mode + 20% brightness, 7 AM = light mode + 100%).
- [ ] **Profiles** — Named presets that bundle any combination of settings. Each profile has a name and all fields are optional (if absent, that value stays unchanged): all-display brightness, per-monitor brightness (`{ [monitorId]: number }`), dark/light mode, and volume. Examples: "Night Coding" = brightness 15% + dark mode; "Presentation" = external 100% + laptop 50% + light mode + volume 30%; "Mute" = volume 0% only. Profiles can be triggered from the tray menu and bound to keyboard shortcuts. Replaces the old `BrightnessPreset` / `VolumePreset` types in `types.d.ts`.
- [ ] **Brightness/volume increment shortcuts** — Add "brightness up/down by delta" and "volume up/down by delta" commands. `brightness_delta` config exists but isn't wired to shortcuts.
- [x] **Settings UI in the app** — Add a settings panel for min brightness, keyboard shortcuts, and monitor sort/disable/rename instead of editing JSON files manually.
- [x] **Launch at login** — Auto-start the app on system boot. Tauri has an `autostart` plugin for this.

## Larger Features

- [ ] **Multi-monitor drag-and-drop reordering** — Allow drag-to-reorder monitors in the expanded view instead of editing `sort_order` in JSON.
- [ ] **Ambient light adaptation** — Auto-adjust brightness based on ambient light sensor (macOS supports this via IOKit).
- [ ] **Blue light filter / Color temperature** — A "night shift" style warm color temperature control, if display-dj CLI supports gamma/color adjustments.
- [ ] **Keyboard shortcut editor UI** — Visual editor for hotkeys instead of editing `preferences.json`. Show current bindings, let users record new ones.
