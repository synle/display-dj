# ESLint / Oxlint Migration — Rule Ledger

**Repo**: [synle/display-dj](https://github.com/synle/display-dj) · **Path**: `/Users/syle/git/display-dj`
**Purpose**: Record of every lint rule added, excluded, or disabled to keep `npm run lint` green, plus what a future agent should tighten next.
**Status**: ✅ **DONE** — all tracked items resolved as of 2026-08-23 (`vite@8` migration era).

## TLDR

Old ESLint setup was replaced by oxlint (no config file, defaults only). This ledger documents that setup, the stricter `.oxlintrc.json` added later, the one rule disabled and why, every code fix required to go green, and the rules still left off for a future pass.

## Setup history

| Change                                              | Commit                | Notes                                                                                                                                                                               |
| --------------------------------------------------- | --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Removed `.eslintrc` / `.eslintignore`, added oxlint | `7f9aa1c`             | Old config extended `eslint:recommended` + `plugin:@typescript-eslint/recommended`; ignored `*.spec.*`, `build`, `dist`. eslint itself was never in devDependencies (stale config). |
| Bare oxlint, no config                              | `7f9aa1c` – `dc45133` | Default = correctness category only, plugins `typescript`/`unicorn`/`oxc` on.                                                                                                       |
| Added `.oxlintrc.json` (this change)                | current               | correctness + suspicious + perf as errors, react plugin on, one rule disabled.                                                                                                      |

## Current configuration (`.oxlintrc.json`)

```json
{
  "$schema": "./node_modules/oxlint/configuration_schema.json",
  "ignorePatterns": ["src/binaries", "dist", "coverage", "src-tauri/target"],
  "plugins": ["typescript", "unicorn", "oxc", "react"],
  "categories": {
    "correctness": "error",
    "suspicious": "error",
    "perf": "error"
  },
  "rules": {
    "react/react-in-jsx-scope": "off"
  }
}
```

Run via `npm run lint` → `npx oxlint .` (ignore patterns moved into the config; `--ignore-pattern=src/binaries` flag removed from the script).

## Rules added

| Rule / category                     | Status                 | Why                                                                                  |
| ----------------------------------- | ---------------------- | ------------------------------------------------------------------------------------ |
| `correctness` (was already default) | ✅ enabled, clean      | Baseline.                                                                            |
| `suspicious` category               | ✅ enabled, clean      | Catches probable bugs (shadowing, bad comparisons). Zero findings at adoption.       |
| `perf` category                     | ✅ enabled, clean      | Unnecessary allocations/spread hot spots. Zero findings at adoption.                 |
| `react` plugin (incl. hooks rules)  | ✅ enabled after fixes | Was OFF during bare-oxlint era; enabling surfaced 6 real issues — all fixed (below). |

## Rules disabled / excluded

| Rule / path                               | Status                          | Reason                                                                                                                                                                                       |
| ----------------------------------------- | ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `react/react-in-jsx-scope`                | ⛔ disabled in `.oxlintrc.json` | Repo uses React 19 automatic JSX runtime (`jsx: "react-jsx"`); rule predates it and fired 422 false positives. No jsx-runtime setting exists in oxlint yet — re-enable when oxlint adds one. |
| `src/binaries/**`                         | 🚫 ignored                      | Vendored helper binaries, not app source. Carried over from original `npm run lint` flag.                                                                                                    |
| `dist/`, `coverage/`, `src-tauri/target/` | 🚫 ignored                      | Build artifacts.                                                                                                                                                                             |

## Code fixes required by the new rules (all ✅ applied)

| Finding                                                                  | File                    | Fix                                                                                                                                                                                                                                                                                         |
| ------------------------------------------------------------------------ | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `unicorn(consistent-function-scoping)`                                   | `AboutPanel.tsx`        | Hoisted `pad()` to module scope.                                                                                                                                                                                                                                                            |
| `unicorn(consistent-function-scoping)`                                   | `AccessibilityGate.tsx` | Hoisted `openSystemSettings()` to module scope.                                                                                                                                                                                                                                             |
| `unicorn(no-array-sort)`                                                 | `SettingsPanel.tsx`     | `[...configs].sort()` → `.toSorted()`; bumped tsconfig `lib` to `ES2023`.                                                                                                                                                                                                                   |
| `react(no-deriving-state-in-effects)` + `react(set-state-in-effect)`     | `Slider.tsx`            | Replaced setState-in-effect prop sync with React's adjust-state-during-render pattern (`prevPropValue` guard).                                                                                                                                                                              |
| `react(exhaustive-effect-dependencies)` + `react-hooks(exhaustive-deps)` | `App.tsx`               | Added stable `fetchAllState` callback to mount-effect deps.                                                                                                                                                                                                                                 |
| `react(set-state-in-effect)`                                             | `App.tsx:126`           | Suppressed with block-scoped `// oxlint-disable react/set-state-in-effect` + justification comment. **False positive**: the fetch fns are async; every setState runs after `await invoke`. `disable-next-line` did not cover diagnostics reported at inner callee columns; block form does. |
| `react(no-array-index-key)`                                              | `ProfileButtons.tsx`    | Key is now data-dependent: `profile.name                                                                                                                                                                                                                                                    |     | 'unnamed-${i}'` (fallback keeps uniqueness for unnamed profiles; activation stays index-based by design). |

## Left off deliberately — future tightening candidates (status: ☐ open)

Rules/categories NOT enabled, with known findings if switched on. For a future agent:

| Candidate                                                              | Status      | Known blockers found when trialed                                                                      |
| ---------------------------------------------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------ |
| `pedantic` category                                                    | ☐ open, off | `eslint(max-lines-per-function)` — `SettingsPanel.tsx` (~79-line fn vs limit 50) would need splitting. |
| `style` / `restriction` categories                                     | ☐ open, off | Not trialed; expect churn (naming, import style).                                                      |
| `eslint(radix)`                                                        | ☐ open, off | `SettingsPanel.tsx` has two `parseInt` calls without radix (~lines 426, 448 pre-fix numbering).        |
| `import`, `promise`, `node`, `jsdoc`, `jsx-a11y`, `react-perf` plugins | ☐ open, off | Not trialed.                                                                                           |

## Verification

- `npm run lint` → 0 errors, 0 warnings, exit 0
- `npm test` → 21 files / 200 tests passing
- `npm run build` (tsc + vite build) → green
