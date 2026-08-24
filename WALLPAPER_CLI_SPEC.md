# Wallpaper API Spec — ARCHIVED (historical)

> Historical design spec for HTTP wallpaper endpoints in [display-dj-cli](https://github.com/synle/display-dj-cli), from when display-dj talked to the CLI as a sidecar over HTTP. Reference only.

## Current state (v7.0.0+)

The CLI sidecar was removed in v7.0.0; all wallpaper functionality is vendored at `src-tauri/src/core/wallpaper.rs`. Current API (in-process, called via `spawn_blocking` from the `wallpaper.rs` wrappers):

| Function                                                         | Replaces (old HTTP endpoint)                                       |
| ---------------------------------------------------------------- | ------------------------------------------------------------------ |
| `core::wallpaper::set_wallpaper(fit, path)`                      | `GET /set_wallpaper/{fit}/{path}`                                  |
| `core::wallpaper::set_wallpaper_one(monitor_index, fit, path)`   | `GET /set_wallpaper_one/{monitor_index}/{fit}/{path}`              |
| `core::wallpaper::get_wallpaper()`                               | `GET /get_wallpaper`                                               |
| `core::wallpaper::is_wallpaper_supported()`                      | `GET /get_wallpaper_supported`                                     |
| `core::wallpaper::slideshow_start(interval, order, fit, folder)` | `GET /wallpaper_slideshow_start/{interval}/{order}/{fit}/{folder}` |
| `core::wallpaper::slideshow_stop()`                              | `GET /wallpaper_slideshow_stop`                                    |
| `core::wallpaper::slideshow_status()`                            | `GET /wallpaper_slideshow_status`                                  |

The user-facing `command/wallpaper/...` strings (and the Settings UI) are unchanged — only the transport changed. Current architecture: [DEV.md](DEV.md) and the Wallpaper section of [AGENTS.md](AGENTS.md).
