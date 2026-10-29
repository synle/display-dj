# Display DJ — Agent Guide

## Project Overview

Display DJ is a cross-platform Tauri v2 tray app with React 19, TypeScript, and Vite 8.
It controls displays, theme, audio, sleep prevention, wallpaper, and window tiling on macOS, Windows, and Linux/X11.

Platform work runs in-process under `src-tauri/src/core/`; no sidecar or runtime dependency on display-dj-cli remains.
Read `VENDORING.md` before refreshing copied platform sources or local extensions.

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

Before setup or debugging, read `CONTRIBUTING.md` → Quick Start for commands, prerequisites, and VS Code launch configurations.
`package.json` owns npm scripts; run Cargo commands from `src-tauri/`.

## Local Install from Release

Before installing, updating, reinstalling, or testing a released app, read `.agents/skills/install-app/SKILL.md` and follow its platform-specific installation and post-install steps.

Repository skills live in `.agents/skills/<name>/SKILL.md`.
Keep one canonical copy per skill with portable `name` and `description` frontmatter.

## Versioning

`package.json` owns the application version.
Bump it with `npm version <version> --no-git-tag-version`, then follow Formatting below.
Never commit while `package.json` and `src-tauri/tauri.conf.json` versions differ.

`build.rs`, Tauri bundle metadata, and the official release workflow read the synchronized `tauri.conf.json` version.
The UI header, installer filenames, and release tag must stay aligned.
The unpublished Cargo crate stays at `0.0.0`; `v*` tags trigger `release-official.yml`.

`build.rs` calls `tauri_build::try_build()` with `WindowsAttributes`.
Its `expose_app_version()` helper emits `APP_VERSION` and `BUILD_DATE`.
Preserve `asInvoker` integrity and the Common Controls v6 dependency in `src-tauri/windows-app-manifest.xml`.

## Testing

Read `FEATURES.md` → Test Map before selecting tests.
Frontend specs sit beside components; Rust uses inline modules and `src-tauri/tests/smoke.rs`.

```bash
npm test
npm run test:watch             # Interactive watch mode, not a completion check
cd src-tauri && cargo test
```

### Inline snapshots

Prefer `toMatchInlineSnapshot` over `toEqual` or `toMatchObject` for small, deterministic objects and ordered lists when the complete output is the contract.
Keep scalar, boundary, DOM-semantic, and interaction assertions explicit.
Retain partial matchers when unrelated fields are intentionally outside the contract; do not snapshot entire DOM trees or nondeterministic platform data.

Generate or intentionally reset inline snapshots with `npm run test:update-snapshots -- <test-file>`; omit the filter only for an intentional suite-wide update.
Review every generated value against the intended behavior; never accept changed snapshots merely to make failures pass.
Run `npm run format`, then rerun the affected tests without update mode before committing.

### CI

Read CI Workflows below before changing CI or interpreting its results.

### Coverage thresholds (where they live)

Read configured thresholds before changing coverage policy; do not mirror their current values in this guide.

- Frontend: `vite.config.ts` → `test.coverage.thresholds` for lines, statements, branches, and functions.
  Run `npm run test:coverage`; any axis below its floor fails. Reports land in `coverage/`.
- Backend: `.github/workflows/build.yml` → Rust coverage step for `--fail-under-lines`, `--fail-under-functions`, and `--fail-under-regions`.
  Run `cd src-tauri && cargo llvm-cov --lib --summary-only`.
  HTML report directory: `src-tauri/target/llvm-cov-target/html/`.

Floors trail main-branch measurements by approximately 10 percentage points.
When raising thresholds, measure current coverage, set floors approximately 10 points below it, update both files, and commit.
Never lower floors without preserving that gap; workflow comments retain the history.

## Formatting

After every repository change, run `npm run format` before finishing or committing, including Rust-only changes.
Then run `npm run format:check` to verify both formatters without writing.

The format command runs oxfmt, then `postformat` synchronizes the version through `scripts/sync-version.mjs` and invokes `npm run format:rust`.
The `&&` sequence stops on failure.
`format:rust` runs `cargo fmt --manifest-path src-tauri/Cargo.toml`; use it for Rust-only formatting, not instead of the required full wrapper.

Read `.oxfmtrc.json` for formatter configuration.
Schemas under `src-tauri/gen/` remain Git-tracked but excluded from oxfmt.
Regenerate them through Tauri tooling; do not hand-edit them.

## Required Steps for Every Feature Change

1. **Tests**: Add frontend `*.test.tsx` tests for frontend changes and Rust `#[cfg(test)]` tests for Rust changes.
   Mock hardware, platform, or external work at the API boundary.
   Run `npm test` and `cd src-tauri && cargo test` before finishing.
2. **Formatting and version sync**: Follow Formatting and Versioning above.
3. **Documentation**: Update `AGENTS.md`, `DEV.md`, `README.md`, and `CONTRIBUTING.md` for new commands, preferences, UI components, or architecture changes.
   Update `ARCHITECTURE.md` when layers, state ownership, startup, or edit locations change.
   Update affected `FEATURES.md` sections when feature contracts or platform constraints change.
4. **Method comments**: Use Rust `///` or TS/React `/** */` JSDoc.
   Document every public function, Tauri command, React component, non-trivial helper, and test.

## Key Conventions

- Use `#[serde(rename_all = "camelCase")]` for Rust structs sent to the frontend.
- Tauri command names stay snake_case in Rust and frontend invoke strings; frontend parameter objects use camelCase.
- `CommandValue` uses `#[serde(untagged)]` to accept a string or string array.
- Preferences use `#[serde(default)]` for forward-compatible loading.
- Serialize preference writes. Sync temporary primary and backup files with the same new snapshot before atomic replacement.
  The backup is a valid saved snapshot, not previous-version history.
  Quarantine malformed primary JSON before backup recovery or default fallback.
- Monitor UIDs are `{api_id}::{api_model_name}`, not guaranteed physical serial identities.
  `dedupe_moved_configs` folds stale same-name entries into live entries after ID reordering; unmatched disconnected metadata remains.
  Settings filters stale rows only after a successful nonempty monitor load.
  Do not treat name-based reconciliation as unambiguous identity for identical monitors.
- Bound every user-editable numeric on both sides in `Preferences::sanitize()` (`config.rs`).
  Loading preferences and `save_preferences` both call it.
  Treat hand-edited `preferences.json` values as untrusted; add new numeric preferences to sanitization in the same commit.
- Clamp brightness to `[effective_min_brightness(), 100]`; clamp the effective minimum to `[5, 100]`.
  Keep the upper bound: `u32::clamp` panics when `min > max`.
- Contrast is DDC-only (`Option<u32>` / `number | null`); clamp it to `[0, 100]`.
  Live `null` gates visibility; rendered values come from `lastKnownValues`.
  Contrast stays hidden by default and is enabled through `showContrast` in Settings.
- Keep Awake stores a `keepawake` guard in `AppState` as `Mutex<Option<KeepAwake>>`.
  Creating the guard enables sleep prevention; dropping it with `None` releases it.
  Platform mechanisms are IOKit, `SetThreadExecutionState`, and D-Bus on macOS, Windows, and Linux respectively.
  `set_keep_awake` remains async.

## Dependency Upgrade Boundaries

Read the dated library-upgrade log in `DEV.md` before changing dependency pins. Manifests and lockfiles, not historical "latest" labels, define installed versions.

- Keep Tauri JavaScript packages and matching Rust plugins on compatible major/minor releases. A frontend build alone does not validate Tauri bundling.
- Check every CI workflow's Node version before accepting a new package engine range.
- Preserve reqwest's explicit native-TLS features and disabled ALPN; do not silently adopt new TLS defaults.
- Keep the shared `ddc` traits compatible with both native DDC backends.
- Verify Windows and Linux binding changes on their target platforms before release.
- Preserve wallpaper cache hashes and fallible ZIP filename handling. See `FEATURES.md` for dependency roles and `DEV.md` for migration tests and rejected candidates.

## CI Workflows

Before changing CI, read `.github/workflows/build.yml`, `.github/workflows/release-official.yml`, and `.github/workflows/release-beta.yml`.
Keep their build matrices synchronized when adding a target.
Read runner labels and target triples from those workflows instead of maintaining another inventory.

- Build runs on pushes and PRs to `main` and `v2`, plus manual dispatch.
  It runs frontend tests on all six platform/architecture combinations.
  Rust tests execute on macOS and Linux; both Windows jobs compile them with `--no-run` because CI cannot load Dxva2/WebView2 test binaries.
  PRs receive per-platform artifact links.
- Require the rolled-up **Main Build** check (`main_build`) in branch protection, not every matrix permutation.
  It depends on `[build, coverage]` and uses `if: always()` to aggregate failures.
- ARM64 matrix rows use native runners, not cross-compilation.
  Linux conditionals must cover both architectures: use `startsWith(matrix.platform, 'ubuntu-')`, not bare-label equality.
  Key Windows test branching on `runner.os` to cover both Windows runners.
- ARM64 suffixes differ: Debian and NSIS use `arm64`; AppImage uses `aarch64`.
  Keep upload globs extension-based (`*.deb`, `*.AppImage`, `*.exe`) so both architectures match.

For every release request, read `.agents/skills/release/SKILL.md` before selecting a mode or triggering a workflow.
That skill owns official-by-default routing, explicit beta/test opt-in, required summaries, confirmation behavior, and the prohibition on custom changesets or notes inputs.

Official releases use `v*` tags or manual dispatch; beta releases use manual dispatch.
Both workflows call the shared begin-release/end-release actions around Tauri matrix builds.
Official notes include `.github/release-body-static.md`.
Beta drafts use `release-beta-<date>-<sha>` tags.

Official builds set `TAURI_RELEASE=true` for clean version strings.
The beta workflow does not set it; `build.rs` therefore emits a `[DEV - MM/DD/YYYY HH:MM]` suffix, not a beta-SHA suffix.

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
