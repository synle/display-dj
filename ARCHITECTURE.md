# Display DJ Architecture

Display DJ is a tray-first Tauri application with a React/TypeScript WebView and an in-process Rust platform layer. No HTTP sidecar runs.

Read [AGENTS.md](AGENTS.md) for repository rules and task-specific reading requirements.
[FEATURES.md](FEATURES.md) owns detailed feature/platform contracts; [DEV.md](DEV.md) owns operational details.
Read [VENDORING.md](VENDORING.md) before refreshing platform sources.

## Layers and entry points

```text
src/main.tsx → src/App.tsx → components/
                    │ invoke / listen
                    ▼
src-tauri/src/lib.rs: command registration, AppState, startup
                    │
     display.rs / dark_mode.rs / volume.rs / config.rs
     tray.rs / wallpaper.rs / keep_awake.rs / overlay.rs
                    │
                    ▼
core/: platform APIs, subprocess adapters, shared display types

tiling/mod.rs: shared layout and placement planning
     └─ tiling/{macos,windows,linux}.rs: enumerate/apply/observe
          └─ tiling/snap_overlay.rs: Windows/Linux WebView previews
```

`src-tauri/src/main.rs` enters the library's `run()`. `lib.rs::invoke_handler` is the authoritative Tauri command inventory.

| Area                                                  | Ownership                                                                                              |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `src/App.tsx`                                         | Main-panel state, backend subscriptions, cached audio polling, last-known slider patches, popup sizing |
| `src/components/SettingsPanel.tsx`                    | Three settings tabs, serialized preference-save queue, device-state controls, profile editor           |
| `src/profileSettings.ts`                              | Profile command editing and device-icon classification                                                 |
| `src/components/DpiSettings.tsx`                      | DPI loading/apply hooks, option construction, one-to-one display pairing                               |
| `src/App.css`, `components/Icons.tsx`, `Dropdown.tsx` | Shared theme, font tokens, icons, native-select presentation                                           |
| `src-tauri/src/lib.rs`                                | AppState, startup restoration, scheduler, event-loop integration                                       |
| `src-tauri/src/config.rs`                             | Preferences, sanitization, migrations, durable writes, debug logging, version/about metadata           |
| `src-tauri/src/tray.rs`                               | Native menus, popup anchoring, shortcut registration, command-string dispatch, device rescan           |
| `src-tauri/src/{display,dark_mode,volume}.rs`         | Application routing, caches, platform-result handling, frontend events                                 |
| `src-tauri/src/core/`                                 | Platform mechanisms; no Tauri types; vendored sources plus local extensions                            |
| `src-tauri/src/tiling/`                               | Shared geometry/planners, native window operations, drag observers and previews                        |
| `src-tauri/src/{wallpaper,overlay,crash_log}.rs`      | Wallpaper command parsing/cache, brightness overlays, crash records                                    |

Core-local extensions include `audio_output/`, `dpi`, `loudness`, `screen_lock`, `task_view`, `linux_desktop`, and `win_cmd`. They are not replacements to copy from the CLI upstream.

## State ownership

| State                   | Authority and lifecycle                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------- |
| User preferences        | `AppState.preferences`, loaded/sanitized in Rust and saved through `config.rs`                                      |
| Last requested sliders  | `preferences.lastKnownValues`; main-popup edits record patches, and startup restoration reapplies available targets |
| Monitor read snapshot   | `SidecarCache`, five-minute TTL; writes and rescans invalidate or replace it                                        |
| Display write handles   | `core::display::CONTROL_CACHE`; refreshed by enumeration, rebuilt after write failure                               |
| Audio endpoint snapshot | `AppState.audio_output_state`; backend probes every five seconds; frontend reads the shared snapshot                |
| Current speaker         | OS default endpoint; `lastKnownValues.selectedSpeakerId` is a separate startup resume target                        |
| Accessibility status    | `SidecarCache` plus cache-bypassing permission recheck when the popup becomes visible                               |
| Tray state/position     | `AppState.is_dark_mode`, `is_muted`, click point, tray rectangle, and focus-gain guard                              |
| Keep Awake              | `Mutex<Option<KeepAwake>>`; dropping the guard releases sleep prevention                                            |
| Window restore state    | `AppState.tiling_state`; platform window IDs and original rectangles                                                |
| Scheduled phase         | `AppState.last_scheduler_phase`; process-local, not a durable schedule history                                      |
| Slideshow resume        | Backend-owned `wallpaper.slideshowLastPath`; updated by the core slideshow change hook                              |

Monitor UIDs currently combine runtime API ID and model name. Same-name reconciliation repairs some reordered IDs but cannot uniquely identify identical physical monitors.

Audio IDs instead use native CoreAudio UIDs, Windows MMDevice endpoint IDs, or Linux sink names. Aliases, state, and ordering attach to those IDs.

## Startup and visibility

`lib.rs::run()` installs panic logging, configures the logger, loads preferences, installs plugins, and registers managed state and commands.

Setup then starts independent work:

- A background worker imports macOS crash reports, seeds tray/cache state, restores last-known values, checks Windows loudness, and resumes an enabled slideshow.
- The main setup builds the tray, starts the audio refresh thread, registers saved shortcuts, and synchronizes autostart.
- Platform Tile Snap monitors start independently of their saved enable toggle. macOS requires Accessibility; Linux retries X11 connections.
- The scheduler worker waits 60 seconds before its first check and repeats at that interval.

These workers do not form one serial readiness barrier. A shown popup can initially use cached state while refresh work continues.

The main WebView starts hidden in `tauri.conf.json`. Tray clicks and **Show Window** share `show_popup_window`; focus loss normally hides it.

Popup placement uses the physical tray-click point and saved tray rectangle. Overlapping mixed-DPI monitor bounds use nearest-edge selection rather than first-match selection.

`App.tsx` caps content-driven height and provides internal scrolling. Resizes reuse the saved anchor; DPI changes request a fresh tray anchor.

## Main request flows

### Slider write

```text
Slider → App handler → update lastKnownValues immediately
                   ├─ coalesce record_last_known_values patch (200ms)
                   └─ invoke brightness / contrast / volume command
                          → application routing → core platform operation
```

Brightness/contrast sends use trailing debounce; volume uses leading/trailing throttle. Backend brightness and volume paths serialize writes and skip superseded requests.

Brightness also skips recently repeated identical values. Per-monitor brightness mode chooses hardware-only, overlay-only, or automatic routing.

Main sliders retain requested values on hardware failure. They are neither live-readback indicators nor confirmation that hardware accepted a write; no main status bar or Mixed state renders.

### Settings save

```text
Settings edit → newest full snapshot → 100ms debounce
              → one in-flight save_preferences
              → sanitize → preserve backend-owned fields
              → synced temporary files → atomic replacements → publish memory
              → apply changed shortcuts / autostart / slideshow settings
```

Primary and backup receive the same saved snapshot, not separate revisions. Malformed primary JSON is quarantined before backup/default recovery.

Close and unmount request a final queue flush. Save failures retain pending work but have no visible save-status badge.

### Device refresh and selection

Popup show and explicit **Refresh Displays & Speakers** run a background rescan. Main section labels invoke the same refresh command.

Audio additionally has continuous five-second backend probing. Only changed snapshots emit `audio-output-changed` and schedule a tray-menu rebuild.

The frontend's visible-main-panel poll is a cache fallback, not a second five-second OS enumerator. It stops while hidden or on another panel.

Output selection validates the exact current endpoint and enabled state, switches through the platform adapter, then refreshes endpoint/volume/tray state.

Expanded popup rows own aliases and reordering. Settings owns enabled/disabled/hidden controls and hidden-output recovery; it has no reorder arrows.

### Shortcuts, profiles, and schedules

`tray::execute_command` dispatches `command/...` strings in-process. Profiles run command lists; menu items, shortcuts, and schedules reuse this dispatcher where applicable.

Shortcut callbacks dispatch `command/system/...` on release and other commands on press. This prevents a press/release cycle from running a z-order action twice.

Night scheduling chooses custom phase commands, then a named profile, then legacy brightness/theme fallback. Only the legacy fallback confirms write results before marking the phase applied.

### Tiling and Tile Snap

`tiling/mod.rs` owns shared geometry and placement planners; platform modules enumerate windows and apply results. App Exposé reserves wholly consumed displays for the target app.

Focused tiling normalizes maximize/fullscreen state before saving restored bounds. It leaves minimized windows alone outside Exposé.

macOS observes NSEvent mouse events and uses native overlays. Windows observes WinEvent move/size lifecycle events; Linux polls X11 pointer/window geometry.

Windows and Linux share click-through WebView previews. Per-display event names isolate previews; capability label matching enables their event listeners.

Windows does not install a global low-level mouse hook. Native clicks and double-click maximize/restore stay owned by Windows.

## Platform and evidence boundaries

- Stateful mutations remain async; blocking platform work belongs off the event loop. Existing synchronous readers and direct filesystem paths are not proof that all work is nonblocking.
- Windows runs at normal integrity. Moving elevated windows requires explicit administrator launch; optional elevation/native-Snap probes do not gate startup.
- Windows Rust CI compiles tests with `--no-run`; a green build is not interactive window, audio, or input verification.
- Linux tiling/Tile Snap requires X11. Arch/KDE remains super alpha; Wayland support is feature-specific and does not include general window management.
- Soft-overlay brightness needs `monitor_rect`, currently absent from macOS/Linux display enumeration. Missing geometry returns an error, not successful dimming.
- DPI changes are platform-specific: macOS display modes, undocumented Windows scaling packets, and Linux XRandR transforms. The beta UI is not the full rollback-capable display-configuration roadmap.
- Crash logs contain raw preference snapshots and identifiers. Diagnostics are not automatically privacy-safe support bundles.

## Build and context boundaries

`package.json` owns the application version; `npm run format` synchronizes it into `tauri.conf.json`. `build.rs` reads that synchronized value; Cargo package version stays `0.0.0`.

The `TAURI_RELEASE` environment flag controls clean official version strings. Other builds currently use a DEV timestamp suffix, including the beta workflow.

Use the repository's `.agents/skills/` install/release workflows. Release mode defaults to official; only an explicit beta/test request selects beta.

Historical plans, `WALLPAPER_CLI_SPEC.md`, and `DONE.eslint.md` preserve earlier decisions and measurements. They do not override current source or establish present validation status.
