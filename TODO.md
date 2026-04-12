# TODO - Feature Improvements

## Quick Wins

- [ ] **Brightness presets synced with dark/light mode** — `types.d.ts` already defines `BrightnessPreset` with `syncedWithMode`. Wire it up so switching to dark mode auto-sets brightness (e.g., dark = 10%, light = 100%).
- [x] **Volume presets via keyboard shortcuts** — Add intermediate volume levels (e.g., 25%, 50%, 75%) alongside the existing mute/unmute shortcuts (Shift+F6/F7).
- [ ] **Tray icon reflecting state** — Change the tray icon dynamically based on brightness level or dark/light mode (e.g., dim icon when brightness is low, moon/sun icon for dark/light).

## Medium Effort

- [ ] **Brightness scheduling / Night mode** — Auto-dim at sunset or on a schedule (e.g., 9 PM = dark mode + 20% brightness, 7 AM = light mode + 100%).
- [ ] **Per-monitor brightness presets** — The TODO in `types.d.ts` notes presets only apply to all monitors. Allow per-monitor presets (e.g., laptop at 40%, external at 80%).
- [ ] **Brightness/volume increment shortcuts** — Add "brightness up/down by delta" and "volume up/down by delta" commands. `brightness_delta` config exists but isn't wired to shortcuts.
- [ ] **Monitor profiles** — Save/restore named configurations (e.g., "Presentation mode" = external at 100% + laptop disabled, "Night coding" = all at 15% + dark mode). Quick switch from tray menu.
- [ ] **Settings UI in the app** — Add a settings panel for min brightness, keyboard shortcuts, and monitor sort/disable/rename instead of editing JSON files manually.
- [ ] **Launch at login** — Auto-start the app on system boot. Tauri has an `autostart` plugin for this.

## Larger Features

- [ ] **Multi-monitor drag-and-drop reordering** — Allow drag-to-reorder monitors in the expanded view instead of editing `sort_order` in JSON.
- [ ] **Ambient light adaptation** — Auto-adjust brightness based on ambient light sensor (macOS supports this via IOKit).
- [ ] **Blue light filter / Color temperature** — A "night shift" style warm color temperature control, if display-dj CLI supports gamma/color adjustments.
- [ ] **Keyboard shortcut editor UI** — Visual editor for hotkeys instead of editing `preferences.json`. Show current bindings, let users record new ones.
