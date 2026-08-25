# display-dj

Cross-platform desktop app for monitor brightness, contrast, dark mode, volume, and window tiling control. Built with Tauri v2 (Rust backend) + React 19 + TypeScript + Vite 6 (WebView frontend). All platform code (DDC/CI, gamma, WMI, DisplayServices, wallpaper) is vendored in-process under `src-tauri/src/core/`.

## Quick Start

Prerequisites: Node.js 20+, [Rust toolchain](https://rustup.rs/) (stable), and platform deps (Linux: see CONTRIBUTING).

```bash
npm install
npm run tauri dev       # Full app (Vite + Rust backend)
npm run dev             # Frontend only (localhost:1420)
npm test                # Frontend tests
npm run test:coverage   # Frontend coverage gate
cd src-tauri && cargo test                            # Backend tests
cd src-tauri && cargo llvm-cov --lib --summary-only   # Backend coverage
npm run tauri build     # Production build (.dmg / .exe / .deb / .AppImage)
```

## Coverage thresholds

CI enforces floors that trail main-branch measurement by ~10pp. Actual numbers are NOT mirrored here — read from source of truth:

- **Frontend** → `vite.config.ts` → `test.coverage.thresholds`. Reports in `coverage/`.
- **Backend** → `.github/workflows/build.yml`, `Rust coverage` step (`--fail-under-lines/-functions/-regions`). HTML in `src-tauri/target/llvm-cov-target/html/`.

Raising: measure current %, set floor ~10pp below, update both files. Never lower without keeping the ~10pp gap.

## Platform pitfalls

### macOS — Chromium apps & `AXFocusedApplication` (Brave / Chrome / Edge / Arc)

**Symptom:** all `command/tile/*` shortcuts silently no-op when a Chromium browser is focused; other apps and OSes fine.

**Root cause:** system-wide AX lookup (`AXUIElementCreateSystemWide()` → `AXFocusedApplication` → `AXFocusedWindow`) returns `-25212` (`kAXErrorCannotComplete`) for Chromium — its sandboxed renderers aren't fully wired to the system-wide AX server. Windows (`GetForegroundWindow`) and Linux (`_NET_ACTIVE_WINDOW`) don't traverse AX trees, hence macOS-only.

**Fix (v7.0.24+):**

1. Fallback: on failure, get PID via `NSWorkspace.frontmostApplication`, then read `AXFocusedWindow` from an app-scoped element (`get_focused_window_via_frontmost_app`). Same approach as AeroSpace/Rectangle.
2. `_AXUIElementGetWindow` can return `wid == 0` for Chromium elements — `execute_tile` treats window_id as optional and proceeds without restore-state.
3. All AX failures log code + description via `ax_error_description`; `-25212` is tagged "Chromium-style" so regressions are grep-able.

Code: `src-tauri/src/tiling/macos.rs`. Verify live: focus Brave, hit Ctrl+Shift+Right, expect a log line like `AXFocusedApplication failed with AXError=-25212 … falling back to NSWorkspace.frontmostApplication`. If you instead see `set AXPosition(…) failed`, the lookup worked but the app refused the move (fullscreen-locked or undecorated window) — different problem.

### Linux — running locally & first-build notes (verified on Mint 22.2 / XFCE / X11)

**Running the binary from a terminal or agent shell:** the app is a tray daemon with no window. A plain `./target/release/display-dj &` dies when the parent shell/session closes (no crash in the log — it just vanishes). Run fully detached:

```bash
cd src-tauri
setsid nohup ./target/release/display-dj > /tmp/display-dj.log 2>&1 < /dev/null &
```

Logs go to stdout (plus `debug.log` in the config dir when enabled). A clean startup ends with `startup probe + cache pre-warm complete` and `register_shortcuts: done — N registered, 0 failed`.

**Tray icon invisible on XFCE:** the panel needs the **Status Notifier/Indicator** plugin (`xfce4-indicator-plugin`, installed by default on Mint). The icon may also land under the notification-area collapse arrow (`^`). Right-click is reliable for the menu; left-click doesn't always fire (AppIndicator limitation, see Known Limitations in CONTRIBUTING).

**GTK GL warning at startup** (`Disabled hardware acceleration because GTK failed to initialize GL`) — common in VMs/remote-desktop sessions. Tauri falls back to software rendering; harmless unless you see visual glitches, in which case try launching with `WEBKIT_DISABLE_COMPOSITING_MODE=1`.

**Z-order self-test on X11:** launch with `DISPLAY_DJ_ZORDER_SELFTEST=1`; five seconds after startup it runs all 6 z-order commands on whatever window is focused and logs each step with a `[zorder-selftest]` prefix. Verified working on XFCE/xfwm4 — this exercises the same focused-window resolution and move dispatch as tiling.

**Generated schemas:** the first Tauri build on each platform regenerates `src-tauri/gen/schemas/*` and creates a platform file (`linux-schema.json` — tracked). Local builds may also reformat `capabilities.json` / `acl-manifests.json` (pretty vs compact) when the local tauri CLI version differs from the last committer's — content-equivalent churn; revert rather than commit formatting-only diffs of those two.

**Brightness paths on Linux (`core::linux.rs`):** built-in panels write `/sys/class/backlight/<device>/brightness` directly, falling back to the `brightnessctl` CLI on permission failure (user needs the `video` group); external monitors shell out to `ddcutil` over `i2c-dev` (needs `ddcutil`, `i2c-tools`, `i2c-dev` loaded + user in `i2c` group); gamma dimming uses `xrandr` on X11. Full setup + verify commands in CONTRIBUTING "Platform Setup".

**Linux verification status (as of v7.2.0):** build (`.deb` + `.AppImage`), global shortcuts, tray icon, z-order commands, and Exposé layout math all pass on Mint 22.2/XFCE/X11 with only a built-in eDP panel. DDC/CI against real external monitors and Tile Snap are untested on Linux (Tile Snap is macOS-only by design).

## Crash Logging

Every Rust panic plus every macOS native crash (`.ips` from `~/Library/Logs/DiagnosticReports/`) lands in `{config_dir}/display-dj/crash.log` (same folder as `preferences.json`; "Open App Folder" reveals it). Panics are captured by an in-process `std::panic::set_hook`; native crashes are summarized into the same file by `crash_log.rs::import_macos_native_crashes` at each launch, so both crash modes appear chronologically without Console.app.

Each record carries timestamp, app version, OS/arch, thread, location, payload, backtrace, the last 80 lines of `debug.log`, and a full preferences snapshot (native crashes add incident id, exception type/signal, top 40 frames). `crash_log.rs::rotate_if_needed` trims at the next `==========` boundary past ~2 MB, newest preserved. Never gated on `debug_logging`.

Tauri commands: `get_crash_log` (contents as string), `open_crash_log` (opens in default editor, creates empty file if missing). Used by the About panel.

### Post-mortem: v7.0.26 SIGABRT in `GlobalObserverHandler`

Three field crashes: abort inside the NSEvent global-monitor ObjC block. `catch_unwind` around the Rust handler was inert because `[profile.release].panic = "abort"` — panics skipped the catch and hit `abort()`. **Fix (v7.0.29):** `panic = "unwind"` in release + defensive `state.displays.get()` bounds check.

**Rule:** any Rust closure wrapped in a foreign callback (ObjC block, C fn pointer, Win32 callback) requires `panic = "unwind"` or `catch_unwind` is documentation, not a safety net.
