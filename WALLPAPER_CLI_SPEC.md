# Wallpaper API Spec for display-dj-cli

## Context

display-dj (the Tauri GUI app) is adding wallpaper management. The OS-level wallpaper operations should live in display-dj-cli, following the same pattern as brightness/contrast/volume/dark-mode — the GUI app handles UI, preferences, and orchestration, the CLI handles platform-specific OS calls via HTTP endpoints.

The GUI app handles: image validation, copying to cache dir, MD5 hashing, preferences/state persistence, slideshow timer, Settings UI, per-monitor matching by name/id.

The CLI handles: actually setting the wallpaper on the OS, querying current wallpaper, per-monitor enumeration for wallpaper purposes.

---

## HTTP Endpoints Needed

### 1. `GET /set_wallpaper/{fit}/{path}`

Set the desktop wallpaper on **all monitors**.

**Parameters:**
- `fit` — one of: `fill`, `fit`, `stretch`, `center`, `tile`
- `path` — absolute path to the image file (URL-encoded if needed, since it contains `/`)

**Behavior:**
- Set the wallpaper using the OS-level API
- Apply the fit/scaling mode
- Return success/failure

**Response:**
```json
{ "ok": true }
```
or on error:
```json
{ "error": "description of what went wrong" }
```

**Platform implementation:**

| Platform | API | Fit mapping |
|----------|-----|------------|
| macOS | `osascript -e 'tell application "System Events" to tell every desktop to set picture to "{path}"'` + `defaults write com.apple.desktopservices DWHScaling {value}` for fit. Or use NSWorkspace via Swift/objc. | fill→scaling factor, fit→proportional, stretch→fill, center→center, tile→tile |
| Windows | `SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, path, SPIF_UPDATEINIFILE)`. Set `HKCU\Control Panel\Desktop\WallpaperStyle` + `TileWallpaper` registry keys before calling. | fill→Style=10/Tile=0, fit→Style=6/Tile=0, stretch→Style=2/Tile=0, center→Style=0/Tile=0, tile→Style=0/Tile=1 |
| Linux | `gsettings set org.gnome.desktop.background picture-options {mode}` then `gsettings set org.gnome.desktop.background picture-uri "file://{path}"`. Fallback: `xfconf-query -c xfce4-desktop -p /backdrop/screen0/monitor0/workspace0/last-image -s "{path}"` (XFCE), then `feh --bg-{mode} "{path}"`. | fill→`zoom`, fit→`scaled`, stretch→`stretched`, center→`centered`, tile→`wallpaper` |

---

### 2. `GET /set_wallpaper_one/{monitor_index}/{fit}/{path}`

Set wallpaper on a **single monitor** by 0-based display index.

**Parameters:**
- `monitor_index` — 0-based index into the ordered display list (same ordering as `/list` or `/get_all` returns)
- `fit` — same as above
- `path` — absolute path to image

**Behavior:**
- Resolve `monitor_index` to the OS-level screen/display identifier
- Set wallpaper on that specific screen only
- Return success, or error if index is out of range

**Response:** Same as above.

**Platform implementation:**

| Platform | Per-monitor API |
|----------|----------------|
| macOS | `NSWorkspace.shared.setDesktopImageURL(_:for:options:)` with `NSScreen.screens[index]`. The `options` dict controls fit (e.g., `NSImageScaling`). Or via AppleScript: `tell application "System Events" to set picture of desktop {index+1} to "{path}"`. |
| Windows | `IDesktopWallpaper` COM interface (Win10+). Call `GetMonitorDevicePathAt(index)` to get device path, then `SetWallpaper(devicePath, imagePath)`. For fit: `SetPosition()` with `DWPOS_CENTER`, `DWPOS_TILE`, `DWPOS_STRETCH`, `DWPOS_FIT`, `DWPOS_FILL`. |
| Linux | Not supported natively by GNOME. Return error: `"per-monitor wallpaper not supported on this platform"`. The GUI app will handle this gracefully (falls back to global). |

**Why 0-based index instead of monitor name/id?** The GUI app already resolves user input (name substring, uid, raw id) to a specific monitor. It knows the monitor ordering from `/get_all`. Passing an index keeps the CLI endpoint simple and avoids duplicate matching logic. The GUI owns the "smart matching" — the CLI just needs "set wallpaper on screen N".

---

### 3. `GET /get_wallpaper`

Query the **current wallpaper** path and fit mode.

**Response:**
```json
{
  "path": "/Users/syle/.config/display-dj/wallpapers/wallpaper-abc123.jpg",
  "fit": "fill"
}
```

Or if unable to determine:
```json
{
  "path": null,
  "fit": null
}
```

**Platform implementation:**

| Platform | How to query |
|----------|-------------|
| macOS | `NSWorkspace.shared.desktopImageURL(for: NSScreen.main!)` or `osascript -e 'tell application "System Events" to get picture of desktop 1'` |
| Windows | Read `HKCU\Control Panel\Desktop\Wallpaper` registry key. Read `WallpaperStyle` + `TileWallpaper` for fit. |
| Linux | `gsettings get org.gnome.desktop.background picture-uri` + `gsettings get org.gnome.desktop.background picture-options` |

**Why this endpoint?** Useful for initial state sync on app startup, and for the GUI to detect if someone changed the wallpaper outside of Display DJ.

---

### 4. `GET /get_wallpaper_supported`

Check if wallpaper operations are supported on this platform/session.

**Response:**
```json
{ "supported": true }
```

**Logic:**
- macOS: always `true`
- Windows: always `true`
- Linux: `true` if running on X11/Wayland with a supported desktop environment (GNOME, XFCE, etc.). `false` if no DE detected or running headless.

---

## Design Choices

### Path encoding

Wallpaper paths contain `/` which conflicts with URL routing. Two options:

**Option A (simple):** Use the remainder of the URL path after the known prefix. The CLI route handler strips `"/set_wallpaper/{fit}/"` and treats everything after as the file path. This is what display-dj already does internally for command parsing.

**Option B (standard):** URL-encode the path as a query parameter: `/set_wallpaper?fit=fill&path=%2FUsers%2Fsyle%2Fpic.jpg`. Cleaner routing but more verbose.

**Recommendation:** Option A for consistency with how the CLI already handles paths in other endpoints. The CLI already deals with path parameters in routes like `/set_one/{monitor_id}/{value}`.

### Fit mode validation

The CLI should validate the `fit` parameter and reject unknown values with a clear error. Valid values: `fill`, `fit`, `stretch`, `center`, `tile`. If the CLI receives an unknown fit value, return an error rather than silently defaulting — let the GUI app handle defaults.

### Error handling

Return HTTP 200 with `{ "ok": true }` on success, HTTP 400/500 with `{ "error": "message" }` on failure. The GUI app reads the response and logs errors via its debug log system.

---

## Edge Cases & Race Conditions

The CLI is responsible for handling all of these. The GUI app trusts the CLI to do the right thing.

### Race condition: Multiple slideshow starts

If the GUI sends `/wallpaper_slideshow_start` multiple times rapidly (e.g., user changes folder in Settings, triggers command from profile, etc.), only the **last one wins**. Each start must:
1. Set a cancel flag on the existing timer thread (if any)
2. Wait for the old thread to acknowledge cancellation (or use an `Arc<AtomicBool>` so the old thread exits on next tick)
3. Start the new slideshow

There must be **no window** where two slideshow timers run concurrently. Use a mutex-guarded state + atomic cancel flag pattern.

### Race condition: Manual change during slideshow

When `/set_wallpaper` or `/set_wallpaper_one` is called while a slideshow is running, the CLI must **auto-stop the slideshow** before setting the wallpaper. This means:
1. Cancel the slideshow timer
2. Set the requested wallpaper
3. Return success

The slideshow does not resume after a manual change. The GUI must explicitly re-start it.

### Race condition: Slideshow tick during manual change

If a slideshow tick fires at the exact moment a `/set_wallpaper` request is being processed, the CLI must serialize these operations (e.g., via a mutex on the wallpaper-setting code path) so they don't interleave. The manual change should win — if it arrives during a tick, the tick's wallpaper change is either skipped or immediately overwritten.

### Edge case: Slideshow folder becomes empty/deleted

If the slideshow folder is deleted or all images are removed while a slideshow is running:
- On the next tick, the CLI should detect the error (folder gone, no valid images)
- Auto-stop the slideshow
- Log a warning
- `/wallpaper_slideshow_status` should reflect `"running": false` with an error reason

### Edge case: Image deleted during slideshow

If the current image in the rotation is deleted:
- Skip it and advance to the next valid image
- If no valid images remain, auto-stop (see above)

### Edge case: New images added to folder during slideshow

For `forward`/`backward` order: re-scan the folder on each tick so new images are picked up. Re-sort and find the current position by matching the last-set image path.

For `random` order: re-scan on each full cycle (after all images shown once). New images join the next shuffle.

### Edge case: Very large folders

If a folder has thousands of images, the scan should be efficient:
- Filter by extension during directory listing (don't load file contents)
- Don't validate file size on every tick — only validate when actually setting the wallpaper
- If setting fails (corrupt file), skip and advance

### Edge case: Network/external drives

If the slideshow folder is on a network drive or external disk that becomes unavailable:
- Treat like "folder deleted" — log error, auto-stop slideshow
- Don't hang or block waiting for the drive to reconnect

### Edge case: Concurrent `/set_wallpaper` requests

If two `/set_wallpaper` calls arrive simultaneously (e.g., GUI sends all-monitors + per-monitor in quick succession), serialize them. Last-write-wins is fine — no need for transactional guarantees.

### Edge case: Invalid image in `/set_wallpaper`

If the path doesn't exist, isn't an image, or is too small:
- Return an error response
- Don't change the current wallpaper
- Don't stop any running slideshow (only successful manual changes stop the slideshow)

---

## Slideshow Endpoints

The CLI owns the slideshow timer and state. Only **one active slideshow** at a time — starting a new one cancels the old one.

### 5. `GET /wallpaper_slideshow_start/{interval}/{order}/{fit}/{folder_path}`

Start a wallpaper slideshow cycling through images in a folder.

**Parameters:**
- `interval` — minutes between wallpaper changes (minimum 5)
- `order` — cycling order: `forward` (alphabetical A→Z, wraps), `backward` (Z→A, wraps), `random` (shuffle, no repeat until all shown)
- `fit` — fit mode applied to each image: `fill`, `fit`, `stretch`, `center`, `tile`
- `folder_path` — absolute path to folder containing images

**Behavior:**
1. Cancel any currently running slideshow
2. Scan folder for valid image files (jpg, jpeg, png, bmp, tiff, tif, gif, heic, webp)
3. Sort/shuffle based on `order`
4. Set the first image immediately
5. Start a background timer that advances to the next image every `interval` minutes
6. Wrap around when reaching the end (forward/backward) or reshuffle (random)

**Response:**
```json
{
  "ok": true,
  "total_images": 12,
  "current_image": "/Users/syle/Pictures/nature/alpine.jpg"
}
```

Error cases:
```json
{ "error": "folder not found: /bad/path" }
{ "error": "no valid images found in folder" }
{ "error": "interval must be at least 5 minutes" }
```

### 6. `GET /wallpaper_slideshow_stop`

Stop the active slideshow. The current wallpaper stays as-is.

**Response:**
```json
{ "ok": true, "was_running": true }
```

### 7. `GET /wallpaper_slideshow_status`

Query the current slideshow state.

**Response (running):**
```json
{
  "running": true,
  "folder": "/Users/syle/Pictures/nature",
  "interval_minutes": 30,
  "order": "forward",
  "fit": "fill",
  "current_image": "/Users/syle/Pictures/nature/sunset.jpg",
  "current_index": 3,
  "total_images": 12
}
```

**Response (not running):**
```json
{
  "running": false
}
```

### Auto-stop on manual wallpaper change

When `/set_wallpaper` or `/set_wallpaper_one` is called directly, the CLI **automatically stops** any running slideshow. This avoids race conditions — a manual wallpaper change is an explicit override. The GUI app doesn't need to call `/wallpaper_slideshow_stop` separately.

---

## Rust Libraries Needed

| Crate | Purpose | Notes |
|-------|---------|-------|
| (none new for macOS) | `std::process::Command` for osascript | Already available |
| (none new for Windows) | `windows` crate — `SystemParametersInfoW` is in `Win32_UI_WindowsAndMessaging` | May already be a dependency. For per-monitor: add `Win32_UI_Shell` feature for `IDesktopWallpaper` COM interface. |
| (none new for Linux) | `std::process::Command` for gsettings/xfconf-query/feh | Already available |

No new crate dependencies are expected for wallpaper setting. The slideshow timer uses `std::thread` + `std::sync::Arc<AtomicBool>` for cancellation (all in std).

---

## Priority / Phasing

The GUI app is implementing wallpaper in phases. The CLI endpoints can be added incrementally:

1. **First:** `/set_wallpaper/{fit}/{path}` + `/get_wallpaper` + `/get_wallpaper_supported` — covers Phase 1 (basic wallpaper change)
2. **Second:** `/set_wallpaper_one/{monitor_index}/{fit}/{path}` — covers Phase 1.5 (per-monitor)
3. **Third:** `/wallpaper_slideshow_start` + `/wallpaper_slideshow_stop` + `/wallpaper_slideshow_status` — covers Phase 2 (slideshow)
4. **No CLI work needed for Phase 3 (remote zip)** — download/extract lives in the GUI app, then calls `/wallpaper_slideshow_start` on the extracted folder

The GUI app will initially implement the OS calls locally (behind a clean interface), then swap to HTTP calls once the CLI endpoints are available. No rush — the local implementation works fine as a stopgap.

---

## Endpoint Summary

| # | Endpoint | Phase | Purpose |
|---|----------|-------|---------|
| 1 | `GET /set_wallpaper/{fit}/{path}` | 1 | Set wallpaper on all monitors |
| 2 | `GET /set_wallpaper_one/{monitor_index}/{fit}/{path}` | 1.5 | Set wallpaper on one monitor |
| 3 | `GET /get_wallpaper` | 1 | Query current wallpaper path + fit |
| 4 | `GET /get_wallpaper_supported` | 1 | Check platform support |
| 5 | `GET /wallpaper_slideshow_start/{interval}/{order}/{fit}/{folder_path}` | 2 | Start slideshow |
| 6 | `GET /wallpaper_slideshow_stop` | 2 | Stop slideshow |
| 7 | `GET /wallpaper_slideshow_status` | 2 | Query slideshow state |
