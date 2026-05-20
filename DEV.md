# display-dj

Cross-platform desktop app for monitor brightness, contrast, dark mode, volume, **keyboard backlight (beta)**, and window tiling control. Built with Tauri v2 (Rust backend) and React 19 + TypeScript + Vite 6 (WebView frontend). All platform code (DDC/CI, gamma, WMI, DisplayServices, wallpaper, IOHIDEventSystem / vendor-WMI keyboard backlight) is vendored in-process under `src-tauri/src/core/`.

## Quick Start

Prerequisites: Node.js 20+, [Rust toolchain](https://rustup.rs/) (stable), and platform deps (Linux: see CLAUDE.md for `apt` packages).

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
