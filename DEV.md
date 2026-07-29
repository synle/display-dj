# display-dj

Cross-platform desktop app for monitor brightness, contrast, dark mode, volume, and window tiling control. Built with Tauri v2 (Rust backend) and React 19 + TypeScript + Vite 6 (WebView frontend). All platform code (DDC/CI, gamma, WMI, DisplayServices, wallpaper) is vendored in-process under `src-tauri/src/core/`.

## Quick Start

Prerequisites: Node.js 20+, [Rust toolchain](https://rustup.rs/) (stable), and platform deps (Linux: see AGENTS for `apt` packages).

Install dependencies:

```bash
npm install
```

Run the full app in dev mode (Vite + Rust backend):

```bash
npm run tauri dev
```

Other useful commands:

```bash
npm run dev              # Frontend only (Vite at localhost:1420)
npm test                 # Run frontend tests
npm run test:coverage    # Frontend tests with coverage gate
cd src-tauri && cargo test                # Backend tests
cd src-tauri && cargo llvm-cov --lib --summary-only   # Backend coverage
npm run tauri build      # Production build (.dmg / .exe / .deb / .AppImage)
```

## Coverage thresholds

CI enforces coverage floors that trail the current main-branch measurement by ~10pp. Read the actual numbers from the source of truth — they are NOT mirrored anywhere else:

- **Frontend (Vitest, v8 provider)** → `vite.config.ts` → `test.coverage.thresholds` (`lines`, `statements`, `branches`, `functions`). Reports in `coverage/`.
- **Backend (cargo-llvm-cov)** → `.github/workflows/build.yml`, `Rust coverage` step, `--fail-under-lines` / `--fail-under-functions` / `--fail-under-regions` flags. HTML reports in `src-tauri/target/llvm-cov-target/html/`.

Raising thresholds: measure current %, set floor ~10pp below, update both files. Never lower without keeping the ~10pp safety gap (history is tracked in comments above each set of thresholds).

## Platform pitfalls

### macOS — Chromium-style apps & `AXFocusedApplication` (Brave / Chrome / Edge / Arc)

**Symptom:** Ctrl+Shift+Arrow (and every other `command/tile/*` keybinding) silently no-ops when a Brave / Chrome / Edge / Arc window is focused. Same shortcut works on every other macOS app and on the Windows / Linux builds.

**Root cause:** The old `tiling::macos::get_focused_window` used `AXUIElementCreateSystemWide()` → `AXFocusedApplication` → `AXFocusedWindow`. Chromium-based browsers run their renderers in sandboxed child processes whose AX trees aren't fully wired to the system-wide AX server. The system-wide `AXFocusedApplication` lookup returns **`AXError = -25212`** (`kAXErrorCannotComplete`) even though the browser is clearly focused, and the old code bailed silently — no log line, no UI feedback. Windows uses `GetForegroundWindow()` (OS-level HWND) and Linux uses `_NET_ACTIVE_WINDOW` (window-manager-maintained), neither of which has a per-process AX-subelement layer, which is why this bug is macOS-only.

**Fix (v7.0.24+):**

1. **Fallback path** — when the system-wide AX chain fails, `get_focused_window_via_frontmost_app` asks `NSWorkspace.frontmostApplication` for the PID, builds an `AXUIElementCreateApplication(pid)`, then reads `AXFocusedWindow` from that application-scoped element. Same approach AeroSpace and Rectangle use.
2. **Tile without a `CGWindowID`** — `_AXUIElementGetWindow` (the private bridge from AX → CGWindowID) returns `wid == 0` for some Chromium AX elements. The window_id is only used as the HashMap key for the restore-state feature; the actual `AXPosition` / `AXSize` writes go straight to the AX element. The new `execute_tile` treats `window_id` as optional — proceeds without restore-state when it's missing instead of bailing.
3. **Diagnostic logs** — every AX failure now logs both the numeric code and a description via `ax_error_description`. The Chromium-signature error `-25212` is explicitly tagged `cannotComplete (Chromium-style apps trigger this on system-wide AX lookups)` so future regressions are immediately grep-able.

**Where the code lives:** `src-tauri/src/tiling/macos.rs` — `get_focused_window` (primary + fallback), `get_focused_window_via_systemwide`, `get_focused_window_via_frontmost_app`, `ax_error_description`, `execute_tile`. Tests for the AX-error mapping in `src-tauri/src/tiling/macos.rs` `tests` module (`test_ax_error_description_*`).

**Verifying the fix in live logs:** Focus Brave, hit Ctrl+Shift+Right. Look for:

```
tiling: AXFocusedApplication failed with AXError=-25212 (cannotComplete (Chromium-style apps …))
tiling: system-wide AX focused-window lookup failed, falling back to NSWorkspace.frontmostApplication …
tiling: rightThird on display 0 -> (…) wid=Some(…)
```

If the third line shows `wid=None`, the tile still works — we proceeded without restore-state on purpose.

## Crash Logging

Every Rust panic and (on macOS) every native crash macOS records for the app is appended to a single file: `{config_dir}/display-dj/crash.log` (same folder as `preferences.json` / `debug.log` — "Open App Folder" in the tray menu reveals it).

**Why two sources in one file:** Rust panics are caught by an in-process `std::panic::set_hook`, which sees the full stack and context. Native crashes (SIGSEGV, SIGABRT from C-side asserts, etc.) bypass Rust entirely — macOS dumps them to `~/Library/Logs/DiagnosticReports/display-dj-*.ips`. Both kinds matter, so the importer in `crash_log.rs::import_macos_native_crashes` runs at every launch, summarizes any new `.ips` file, and writes it to the same `crash.log`. Reader sees both crash modes in chronological order without having to open Console.app.

### Record shape

Rust panic block:

```
========== RUST PANIC ==========
timestamp:    2026-05-20T19:50:16.123-07:00
app_version:  7.0.31
build_date:   ...
is_dev_build: false
os:           macos aarch64
thread:       main
location:     src/tiling/macos.rs:2761:42
payload:      index out of bounds: the len is 1 but the index is 2

backtrace:
   0: std::backtrace::Backtrace::force_capture
   1: ...

recent debug.log tail (last 80 lines):
[2026-05-20 19:50:14.901] tile_snap: ...

preferences snapshot:
{ ...full preferences.json verbatim... }
========== END RUST PANIC ==========
```

macOS native crash block (parsed from `.ips`):

```
========== MACOS NATIVE CRASH ==========
source_file:           display-dj-2026-05-20-195016.ips
incident_id:           ...
crashed_app_version:   7.0.26
os:                    macOS 26.5 (25F71)  cpu: ARM-64
exception_type:        EXC_CRASH
signal:                SIGABRT
termination:           Abort trap: 6
asi:                   {"libsystem_c.dylib":["abort() called"]}
faulting_thread:       'main' (idx=0)
frames (top 40):
   0: __pthread_kill +8  (imageIndex=4, imageOffset=38376)
   1: pthread_kill +296  ...
========== END MACOS NATIVE CRASH ==========
```

### File rotation

`crash_log.rs::rotate_if_needed` trims at the next `==========` boundary when the file crosses ~2 MB so the newest entries are always preserved. No background thread; rotation happens at the next append. Crash logging is **never gated on `debug_logging`** — diagnostic value is high, volume is low.

### Tauri commands

- `get_crash_log` — returns the file contents as `String` (empty on fresh install). Used by the About panel to render the latest entry.
- `open_crash_log` — opens `crash.log` in the OS default editor. Creates an empty file first if missing, so the button never fails.

### Post-mortem: v7.0.26 SIGABRT in `GlobalObserverHandler` (2026-05-20)

Three identical SIGABRTs in the field. macOS DiagnosticReports showed the abort in the NSEvent global monitor block (`GlobalObserverHandler` → `DispatchEventToHandlers` → display-dj code → `abort`). Root cause:

1. `tiling/macos.rs::start_tile_snap` registers an NSEvent global monitor whose ObjC block calls into Rust.
2. The Rust handler is wrapped in `std::panic::catch_unwind` (per AGENTS "Tile Snap Event Monitoring") because panics can't unwind through an ObjC block.
3. **`[profile.release].panic = "abort"`** in `Cargo.toml` made `catch_unwind` inert — any panic skipped the catch and went straight to `abort()`.
4. Some edge case (likely a transient out-of-bounds index on `state.displays` when a monitor was disconnected mid-drag) panicked the handler, hit `abort`, and the whole app died.

**Fix (v7.0.29):** `panic = "unwind"` in release + defensive `state.displays.get(display_idx)` in `handle_snap_event`.

**Rule going forward:** Any code path that wraps a Rust closure in a foreign-language callback (ObjC block, C function pointer, Win32 callback, X11 handler) **must rely on `panic = "unwind"`** — otherwise the `catch_unwind` is a documentation comment, not a safety net.

**Why crash logging is now mandatory:** the only reason we caught this was because the user happened to share their macOS DiagnosticReports `.ips` file. Future panics write directly to `crash.log` with full context (backtrace, debug.log tail, preferences snapshot) so triage doesn't depend on the user knowing where the OS hid the crash.
