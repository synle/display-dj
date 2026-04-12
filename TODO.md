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

- [x] **Multi-monitor reordering** — Up/down reorder buttons inline with each monitor in the expanded view instead of editing `sort_order` in JSON.
- [ ] **Ambient light adaptation** — Auto-adjust brightness based on ambient light sensor. See [research notes](#ambient-light-adaptation-research) below.
- [ ] **Blue light filter / Color temperature** — A "night shift" style warm color temperature control, if display-dj CLI supports gamma/color adjustments.
- [ ] **Keyboard shortcut editor UI** — Visual editor for hotkeys instead of editing `preferences.json`. Show current bindings, let users record new ones.

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

#### Windows — UWP `LightSensor` API

The most legitimate of the three platforms:
- `Windows.Devices.Sensors.LightSensor.GetDefault()` to get the sensor
- Subscribe to `ReadingChanged` events for lux values
- No special permissions required

Use the `windows` crate (Microsoft official Rust bindings) for WinRT access.

#### Linux — sysfs IIO subsystem

Read `/sys/bus/iio/devices/iio:device*/in_illuminance_raw` — a simple file read returning a raw integer. Device path varies by hardware and many laptops don't expose ALS at all. Just `std::fs::read_to_string` from Rust.

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
