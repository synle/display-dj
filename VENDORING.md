# Vendoring: `src-tauri/src/core/` ← `synle/display-dj-cli`

All files under `src-tauri/src/core/` are vendored (copied) from
[`synle/display-dj-cli`](https://github.com/synle/display-dj-cli),
adapted to fit display-dj's `crate::AppState` + `core::PlatformImpl`
shape. **Not** a git submodule, **not** a Cargo dependency — plain
source committed into this repo.

The vendor commit is `7b1bc23 feat!: vendor display-dj-cli, drop HTTP
sidecar (v7.0.0)` (2026-05-03). The display-dj-cli upstream is purely
a source-of-record reference: builds and releases of display-dj do
not download, fetch, or otherwise depend on it at runtime.

The CLI source layout is monolithic: shared types, theme, volume,
wallpaper, slideshow, and the high-level display helpers all live in
`src/main.rs`. When vendoring, those concerns were split into
focused modules under `src-tauri/src/core/`. The three platform
files (`macos.rs`, `linux.rs`, `windows.rs`) were copied 1:1 with
only the `crate::` → `super::` namespace adjustment.

## Per-file provenance

The "Last-synced SHA" column records the upstream `synle/display-dj-cli`
commit on `main` that each vendored snapshot corresponds to. Run
`./scripts/check-vendor-drift.sh` to verify these are still accurate.

| Vendored path                     | Upstream path    | Last-synced SHA | Notes                                                                                                                                                                  |
| --------------------------------- | ---------------- | --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/core/mod.rs`       | `src/main.rs`    | `92bbadb`       | Extracted shared types (`DisplayInfo`, `DisplayControl`, `Platform`, VCP consts, `matches_display`) + added `PlatformImpl` cfg-alias.                                  |
| `src-tauri/src/core/macos.rs`     | `src/macos.rs`   | `92bbadb`       | Copied 1:1; only adjustment is `crate::` → `super::`.                                                                                                                  |
| `src-tauri/src/core/windows.rs`   | `src/windows.rs` | `92bbadb`       | Copied 1:1; only adjustment is `crate::` → `super::`.                                                                                                                  |
| `src-tauri/src/core/linux.rs`     | `src/linux.rs`   | `92bbadb`       | Copied 1:1; only adjustment is `crate::` → `super::`.                                                                                                                  |
| `src-tauri/src/core/theme.rs`     | `src/main.rs`    | `92bbadb`       | Extracted `get_dark_mode` / `set_dark_mode` per-OS impls (macOS AppleScript, Windows registry, Linux gsettings/dconf).                                                 |
| `src-tauri/src/core/volume.rs`    | `src/main.rs`    | `92bbadb`       | Extracted `get_volume` / `set_volume` / `set_mute` per-OS impls + `VolumeInfo`.                                                                                        |
| `src-tauri/src/core/wallpaper.rs` | `src/main.rs`    | `92bbadb`       | Extracted `set_wallpaper` / `set_wallpaper_one` per-OS impls + `SlideshowState` and `slideshow_*` helpers.                                                             |
| `src-tauri/src/core/display.rs`   | `src/main.rs`    | `92bbadb`       | Distilled high-level fan-out helpers (`list_all`, `set_all_brightness`, `set_one_brightness`, contrast variants) from CLI's `cmd_get` / `cmd_set_all` / `cmd_set_one`. |

The recorded SHAs above reflect the latest upstream commit at which
the vendored snapshots are byte-identical (modulo the
`crate::` → `super::` namespace tweak on the platform files). The
original vendor pass at display-dj `7b1bc23` (2026-05-03) corresponds
to upstream `a4fc5c4`; `src/` on cli `main` has not changed since,
so the recorded baseline has been rolled forward to current upstream
HEAD via `./scripts/check-vendor-drift.sh --update`.

## How to refresh

1. Decide which upstream commit you want to sync to:
   ```bash
   git -C /Users/syle/git/display-dj-cli log main --oneline -n 20
   ```
2. For each row above, copy the upstream file into the vendored
   path. Re-apply local adaptations:
   - In platform files: `s|crate::|super::|` for the imports row.
   - In extracted-from-`main.rs` files (`mod.rs`, `theme.rs`,
     `volume.rs`, `wallpaper.rs`, `display.rs`): carry over only the
     relevant section from `main.rs`; do not pull in CLI-only code
     paths (`cmd_*`, `serve_*`, HTTP server, slideshow CLI plumbing).
3. Run `cd src-tauri && cargo check && cargo test` and `npm test`.
4. Run `./scripts/check-vendor-drift.sh --update` to record the new
   upstream SHAs into this file.
5. Commit the vendored changes and the updated `VENDORING.md` in the
   same commit.

## How to check drift

```bash
./scripts/check-vendor-drift.sh
```

Reports any vendored file whose upstream version has moved since the
recorded SHA. Exits non-zero on drift, so the script is suitable for
wiring into CI.

Override the upstream checkout location with `DISPLAY_DJ_CLI_PATH`
(defaults to `/Users/syle/git/display-dj-cli`).
