# Wallpaper API Spec — ARCHIVED (historical)

> **This document is historical.** It was the original design spec for HTTP wallpaper endpoints in [display-dj-cli](https://github.com/synle/display-dj-cli) when display-dj (the Tauri GUI) talked to the CLI as a sidecar over HTTP. It is preserved for reference only and no longer reflects how display-dj works.

## Current state (v7.0.0+)

In v7.0.0 the CLI sidecar was removed. All wallpaper functionality was vendored directly into the Tauri backend at `src-tauri/src/core/wallpaper.rs` (mirrored from `display-dj-cli`'s implementation).

The current API is a set of plain Rust functions, called in-process via `tauri::async_runtime::spawn_blocking` from the `wallpaper.rs` Tauri-command wrappers:

| Function                                                         | Replaces (old HTTP endpoint)                                       |
| ---------------------------------------------------------------- | ------------------------------------------------------------------ |
| `core::wallpaper::set_wallpaper(fit, path)`                      | `GET /set_wallpaper/{fit}/{path}`                                  |
| `core::wallpaper::set_wallpaper_one(monitor_index, fit, path)`   | `GET /set_wallpaper_one/{monitor_index}/{fit}/{path}`              |
| `core::wallpaper::get_wallpaper()`                               | `GET /get_wallpaper`                                               |
| `core::wallpaper::is_wallpaper_supported()`                      | `GET /get_wallpaper_supported`                                     |
| `core::wallpaper::slideshow_start(interval, order, fit, folder)` | `GET /wallpaper_slideshow_start/{interval}/{order}/{fit}/{folder}` |
| `core::wallpaper::slideshow_stop()`                              | `GET /wallpaper_slideshow_stop`                                    |
| `core::wallpaper::slideshow_status()`                            | `GET /wallpaper_slideshow_status`                                  |

The user-facing `command/wallpaper/...` strings (and the Settings UI) are unchanged — only the underlying transport changed from HTTP to direct function calls.

For the current architecture, see [DEV.md](DEV.md) and the "Wallpaper" section of [CLAUDE.md](CLAUDE.md).
