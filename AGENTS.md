# Display DJ — Agent Guide

## Project Overview

Cross-platform desktop system tray application for controlling monitor brightness, contrast, dark mode, volume and audio output, keep-awake (sleep prevention), and **window tiling** (macOS + Windows + Linux/X11). Built with **Tauri v2** (Rust backend) + **React 19** (TypeScript frontend) + **Vite 8**.

All platform code (DDC/CI, gamma, WMI, DisplayServices, dark mode, volume, audio-output selection, wallpaper, slideshow) runs **in-process** under `src-tauri/src/core/`. Most modules are vendored from display-dj-cli; audio-output selection is display-dj-local. There is no sidecar process and no runtime dependency on the display-dj-cli repo — Tauri commands call `core::*` functions directly. See [`VENDORING.md`](VENDORING.md) for the upstream→vendored file map and `./scripts/check-vendor-drift.sh` for drift detection.

## Context Ownership

- This file owns repository-wide rules, validation, versioning, CI/release policy, and shared conventions.
- [ARCHITECTURE.md](ARCHITECTURE.md) maps layers, state ownership, startup, and edit locations.
- [FEATURES.md](FEATURES.md) owns detailed feature/platform constraints, the test map, and the command-string reference.
- [DEV.md](DEV.md) covers development, request lifecycles, diagnostics, platform failure history, and the dated library-upgrade log.
- [README.md](README.md) describes user behavior; [CONTRIBUTING.md](CONTRIBUTING.md) covers setup and contribution workflows.
- Read exact dependency versions from `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, and `src-tauri/Cargo.lock`.
- Read the shipped application version from `package.json`.
- Saved plans, stashed changes, archived specs, and migration results do not prove current behavior. Verify them against source and the checked commit.

### Required task-specific reading

Read the matching sections of `FEATURES.md` before editing the listed area. These constraints remain mandatory after relocation.

| Task                                                     | Required sections in FEATURES.md                                                                 |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Native commands, event callbacks, threading, or FFI      | Architecture; Windows Console-Flash Pitfall; macOS Tray Icon Pitfall; Tile Snap Event Monitoring |
| Brightness, contrast, display modes, or monitor metadata | Soft-Overlay Brightness Fallback; DPI Scaling; relevant platform observer sections               |
| Tray icon, popup placement, or Dock behavior             | Dynamic Tray Icon; macOS Dock Visibility; platform pitfalls                                      |
| Speakers, volume, loudness, or output metadata           | Audio Output Selection; Settings & About                                                         |
| Settings, components, typography, or shared controls     | Settings & About; the affected feature section                                                   |
| Tiling, Tile Snap, Exposé, z-order, or shell shortcuts   | Tile Snap Event Monitoring; Window Tiling; Window Z-Order Control                                |
| Wallpaper, profiles, or schedules                        | Wallpaper; Other Features; Command Reference                                                     |
| Commands, dependencies, tests, or vendor refreshes       | Command Reference; Dependencies; Test Map; VENDORING.md as applicable                            |

Tauri uses the published crate pinned in `src-tauri/Cargo.toml`. Do not restore the former `src-tauri/vendor/tauri` workaround.

## Backend Safety

Commands taking `State<'_, AppState>` must be async. Offload blocking platform work with `tauri::async_runtime::spawn_blocking`.
The legacy synchronous `get_preferences` reader is an existing exception, not a template for new handlers.

Never hold preference locks across `.await` or slow platform work. See the macOS Tray Icon Pitfall in `FEATURES.md` before changing logging or persistence on synchronous paths.

## Build Commands

```bash
npm install          # Install frontend dependencies
npm run dev          # Start Vite dev server (frontend only)
npm run build        # Build frontend (tsc + vite build)
npm run lint         # Oxlint (separate from build/test; see DEV.md)
npx tauri dev        # Run full app in development mode
npx tauri build      # Production build (binary + .dmg/.exe/.deb/.AppImage)
cargo check          # Check Rust compilation (from src-tauri/)
```

**VSCode debugging:** `.vscode/launch.json` provides launch configs for Tauri dev, Vite dev, Vitest (run/watch), Cargo test, Cargo check, and Tauri build. `.vscode/tasks.json` defines the `ui:dev` background task referenced by the lldb-based Tauri Dev launch entry.

## Local Install from Release

Repository skills live in `.agents/skills/<name>/SKILL.md` with portable `name` and `description` frontmatter. Keep one canonical copy per skill in this shared cross-agent directory.

Use the [install-app skill](.agents/skills/install-app/SKILL.md) (`/skill:install-app` in clients that support that syntax) to download and install the latest release for the current platform. The repo uses skills, not prompt commands, for these multi-step workflows.

### macOS post-install steps (required)

```bash
xattr -cr "/Applications/Display DJ.app"                       # Strip Apple quarantine (unsigned builds)
tccutil reset Accessibility com.synle.display-dj               # Reset Accessibility (required after each new build for tiling)
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
open "/Applications/Display DJ.app"
```

### Windows post-install steps

Run the matching `*_x64-setup.exe` or `*_arm64-setup.exe` installer for the Windows architecture.

## Versioning

The **single source of truth** is root `package.json` → `"version"`. Bump it with
`npm version <version> --no-git-tag-version`, then run `npm run format`. The `postformat` hook
runs `scripts/sync-version.mjs`, which mirrors the number into `src-tauri/tauri.conf.json`.
`npm run format` is mandatory before every commit, including Rust-only changes; never commit
while the versions in `package.json` and `src-tauri/tauri.conf.json` differ.

`build.rs`, Tauri bundle metadata, and `release-official.yml` continue reading the synchronized
`tauri.conf.json` value, so the UI header, `.dmg`, `.exe`, `.deb`, `.AppImage`, and release tag
stay aligned without maintaining the number manually in two files.

`Cargo.toml` remains pinned at `0.0.0` because the crate is not published. Release versioning is driven by git tags (`v*` triggers `release-official.yml`).

`build.rs` calls `tauri_build::try_build()` with `WindowsAttributes` plus the `expose_app_version()` helper that emits `APP_VERSION` and `BUILD_DATE` env vars. The custom `windows-app-manifest.xml` keeps Windows at `asInvoker` integrity and must retain the Common Controls v6 dependency.

## Testing

```bash
npm test                     # Frontend tests (Vitest)
npm run test:watch           # Frontend tests in watch mode
cd src-tauri && cargo test   # Backend tests (Rust)
```

Read [FEATURES.md → Test Map](FEATURES.md#test-map) before selecting tests. Frontend specs sit beside components; Rust tests use inline modules and `src-tauri/tests/smoke.rs`.

### CI

GitHub Actions (`build.yml`) targets pushes and PRs to `main` and `v2`, plus manual dispatch, across six macOS/Windows/Linux architecture combinations. Frontend tests execute on all six; Windows Rust tests compile with `--no-run`, while macOS and Linux execute them. PRs get a comment with per-platform build artifact links. A single rolled-up check named **`Main Build`** (job id `main_build`) aggregates the matrix + coverage into one green/red status on the commit — require this name in branch protection instead of every matrix permutation.

### Coverage thresholds (where they live)

Coverage floors trail the current main-branch measurement by ~10pp. The actual numbers are NOT mirrored in this doc — read them from the source of truth before changing:

- **Frontend (Vitest, v8 provider)** — thresholds in `vite.config.ts` under `test.coverage.thresholds` (`lines`, `statements`, `branches`, `functions`). Run locally via `npm run test:coverage`; CI step `Frontend coverage (Vitest)` fails the build when any axis dips below its floor. Reports land in `coverage/`.
- **Backend (cargo-llvm-cov)** — thresholds inline on the `Rust coverage (cargo-llvm-cov)` step in `.github/workflows/build.yml` as `--fail-under-lines`, `--fail-under-functions`, `--fail-under-regions` flags. Run locally with `cd src-tauri && cargo llvm-cov --lib --summary-only`. HTML report lands in `src-tauri/target/llvm-cov-target/html/`.

When raising thresholds: measure current %, set floor ~10pp below, update both files, commit. Never lower without leaving the same ~10pp safety gap (the comments above the flags track the history).

## Formatting

After any repository change, always run `npm run format` before committing. It formats frontend files with oxfmt, then runs the npm `postformat` hook: sync the package version into `src-tauri/tauri.conf.json`, then format Rust with `cargo fmt --manifest-path src-tauri/Cargo.toml`. The `&&` sequence stops on failure. Run `npm run format:check` afterward to verify both formatters without modifying files. Use `npm run format:rust` for Rust-only formatting. Config lives in `.oxfmtrc.json`; generated schemas (`src-tauri/gen/`) are excluded from oxfmt but remain tracked in Git. Regenerate them through Tauri tooling; do not hand-edit them.

## Required Steps for Every Feature Change

1. **Tests**: Frontend → `*.test.tsx`, Rust → `#[cfg(test)]` modules. Mock at the API boundary if hardware/platform/external. Run `npm test` and `cd src-tauri && cargo test` before finishing.
2. **Formatting and version sync**: Always run `npm run format` before finishing, regardless of changed file type; `postformat` must leave `package.json` and `src-tauri/tauri.conf.json` versions identical.
3. **Documentation**: Update `AGENTS.md`, `DEV.md`, `README.md`, and `CONTRIBUTING.md` for new commands, preferences, UI components, or architecture changes. Update `ARCHITECTURE.md` when layers, state ownership, startup, or edit locations change. Update affected `FEATURES.md` sections when feature contracts or platform constraints change.
4. **Method comments**: `///` for Rust, `/** */` JSDoc for TS/React. Every public function, Tauri command, React component, non-trivial helper, and test must have one.

## Key Conventions

- All Rust structs sent to frontend use `#[serde(rename_all = "camelCase")]`.
- Tauri commands are snake_case in Rust, called with snake_case strings from the frontend.
- Frontend parameter objects use camelCase (Serde converts).
- `CommandValue` uses `#[serde(untagged)]` so keybindings accept both `"string"` and `["array"]`.
- Preferences use `#[serde(default)]` for forward-compatible config loading.
- Preferences writes are serialized: temporary primary and backup files receive the same new snapshot and are synced before atomic replacement. The backup is a valid saved snapshot, not a separate previous-version history. Malformed primary JSON is quarantined before backup recovery or default fallback.
- Monitor UIDs are `{api_id}::{api_model_name}`, not guaranteed physical serial identities. `dedupe_moved_configs` folds stale same-name entries into live entries after ID reordering; unmatched disconnected metadata remains. Settings filters stale rows after a successful nonempty monitor load. Do not treat this name-based reconciliation as unambiguous identity for identical monitors.
- **All user-editable numerics are bounded on both sides by `Preferences::sanitize()`** (`config.rs`), called from `load_preferences()` (disk → memory) and the `save_preferences` command (frontend → memory + disk). `preferences.json` is a documented hand-editing surface, so treat every numeric in it as untrusted input. Add new numeric preferences to `sanitize()` in the same commit that introduces them.
- Brightness is clamped to `[effective_min_brightness(), 100]`, where `effective_min_brightness()` is itself clamped to `[5, 100]`. The **upper** bound is load-bearing: `u32::clamp` panics when `min > max`, so an unbounded `minBrightness` would panic every brightness path.
- Contrast is DDC-only (`Option<u32>` / `number | null`); a live `null` only gates visibility — the rendered value comes from `lastKnownValues`. The slider is hidden by default and toggled via `showContrast` in Settings.
- Keep Awake uses the `keepawake` crate (v0.6) — guard stored as `Mutex<Option<KeepAwake>>` in `AppState`. Creating enables; dropping (set to `None`) releases. Works on macOS (IOKit), Windows (`SetThreadExecutionState`), Linux (D-Bus). The `set_keep_awake` command is `async` (tray pitfall).

## Dependency Upgrade Boundaries

Read the dated library-upgrade log in `DEV.md` before changing dependency pins. Manifests and lockfiles, not historical "latest" labels, define installed versions.

- Keep Tauri JavaScript packages and matching Rust plugins on compatible major/minor releases. A frontend build alone does not validate Tauri bundling.
- Check every CI workflow's Node version before accepting a new package engine range.
- Preserve reqwest's explicit native-TLS features and disabled ALPN; do not silently adopt new TLS defaults.
- Keep the shared `ddc` traits compatible with both native DDC backends.
- Verify Windows and Linux binding changes on their target platforms before release.
- Preserve wallpaper cache hashes and fallible ZIP filename handling. See `FEATURES.md` for dependency roles and `DEV.md` for migration tests and rejected candidates.

## CI Workflows

All three workflows share the same **6-entry build matrix** — keep them in sync when adding a target:

| Runner             | Rust target                 | Ships                             |
| ------------------ | --------------------------- | --------------------------------- |
| `macos-latest`     | `aarch64-apple-darwin`      | `_aarch64.dmg`                    |
| `macos-latest`     | `x86_64-apple-darwin`       | `_x64.dmg`                        |
| `windows-latest`   | `x86_64-pc-windows-msvc`    | `_x64-setup.exe`                  |
| `windows-11-arm`   | `aarch64-pc-windows-msvc`   | `_arm64-setup.exe`                |
| `ubuntu-22.04`     | `x86_64-unknown-linux-gnu`  | `_amd64.deb`, `_amd64.AppImage`   |
| `ubuntu-22.04-arm` | `aarch64-unknown-linux-gnu` | `_arm64.deb`, `_aarch64.AppImage` |

The arm64 rows are **native** builds on GitHub-hosted arm64 runners (free and unlimited on public
repos), not cross-compiles. Two gotchas when editing these workflows:

- Platform conditionals must not compare against a bare label. `if: matrix.platform == 'ubuntu-22.04'`
  silently skips the arm64 runner — use `startsWith(matrix.platform, 'ubuntu-')`, and key the
  Windows test branch on `runner.os` so it covers `windows-latest` and `windows-11-arm` alike.
- The arm64 filename suffix is **not** uniform: `.deb` and NSIS use `arm64`, `.AppImage` uses
  `aarch64`. The upload globs are extension-based (`*.deb`, `*.AppImage`, `*.exe`) so they match either.

- **`build.yml`** — tests + builds on all platforms for every push/PR. PR comment with artifact download links. Rolled-up `Main Build` check (job `main_build`, `needs: [build, coverage]`, `if: always()`) collapses the 6-platform matrix + coverage into one green/red status — use this name in branch protection. Rust tests run with `--no-run` on both Windows runners (Dxva2/WebView2 can't load in CI); they execute on macOS ARM/Intel and Linux x64/arm64.
- **Release routing** — use the [release skill](.agents/skills/release/SKILL.md) (`/skill:release` in clients that support that syntax); every release request defaults to official. Use beta only when the user explicitly says beta, prerelease, or test release/build; never infer beta from branch, SHA, CI state, or uncertainty. The release skill proceeds without confirmation prompts.
- **Release notes** — always summarize commits since the last non-draft, non-prerelease release for the user. Do not create or set a changeset, changelog, generated notes file, or custom `release_notes` / `notes` input; leave those inputs unset so the shared release workflow generates the changelog.
- **`release-official.yml`** — `v*` tag or manual dispatch. Uses `synle/workflows/actions/release/` shared actions (`begin-release` → Tauri matrix build → `end-release`). Generates changelog from commits since last tag + `.github/release-body-static.md`. Sets `TAURI_RELEASE=true` (clean version). Trigger through `/skill:release`.
- **`release-beta.yml`** — manual dispatch only and explicit opt-in. Same flow with `mode: beta`. Draft prerelease tagged `release-beta-<date>-<sha>`. It does not set `TAURI_RELEASE`; current `build.rs` therefore emits a `[DEV - MM/DD/YYYY HH:MM]` suffix, not a beta-SHA suffix. Trigger through `/skill:release` with an explicit beta/test request.

## GitHub Raw File URLs

Always use the `?raw=1` blob URL format:

```
https://github.com/{owner}/{repo}/blob/head/{path}?raw=1
```

Do **not** use:

- `https://api.github.com/repos/{owner}/{repo}/contents/{path}` (Contents API)
- `https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}`

## Git / PR Merge Policy

- Always **squash and merge** PRs. Never merge commits or rebase merges. One commit per PR.
- **Always rebase before pushing** (`git pull --rebase` before `git push`).
- You may `git merge origin/main` or `git merge origin/master` locally to sync, but PR merges must be squash.
