# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

display-dj is a cross-platform Electron desktop app for managing display brightness (laptop + external monitors), dark mode toggling, and volume control. Runs as a system tray application with keyboard shortcuts. Supports macOS and Windows.

## Build & Development Commands

```bash
npm start                 # Build dev + launch Electron app
npm run dev               # Watch mode (rebuilds on file changes)
npm run build             # Build both UI and Electron (concurrently)
npm run build-prod        # Production build
npm run build-ui          # Build React renderer only
npm run build-electron    # Build Electron main process only
npm run lint              # ESLint on TypeScript files
npm run format            # Prettier formatting
npm test                  # Run Jest tests
npm run test-ci           # CI test mode (CI=true)
npm run dist-darwin       # Package macOS DMG
npm run dist-win32        # Package Windows EXE
```

## Architecture

### Two-Process Electron Architecture

**Main process** (`src/main/`): Node.js backend that manages monitors, system tray, keyboard shortcuts, and IPC.
- Entry: `src/main/index.ts` — app bootstrap, window/tray creation, IPC handlers
- Simulates an Express-like REST API over Electron IPC (custom req/res objects routed through `Endpoints.ts`)

**Renderer process** (`src/renderer/`): React 17 + MUI v5 frontend.
- Entry: `src/renderer/index.tsx`
- State: React Query v3 hooks in `src/renderer/hooks.ts`
- API calls go through `src/renderer/utils/ApiUtils.ts` → IPC → main process endpoints

### IPC API Endpoints (Main Process)

- `GET /api/configs` — current app state (monitors, dark mode, volume)
- `PUT /api/configs/monitors/:id` — update single monitor brightness
- `PUT /api/configs/monitors` — batch update all monitors
- `PUT /api/configs/darkMode` — toggle dark mode
- `PUT /api/configs/volume` — change volume
- `GET /api/preferences` / `PUT /api/preferences` — user preferences

### Platform Adapter Pattern

Platform-specific code uses adapter files selected at build time by `prebuild.js`:
- `DisplayAdapter.Darwin.ts` / `DisplayAdapter.Win32.ts` — display brightness control
- `SoundUtils.Darwin.ts` / `SoundUtils.Win32.ts` — volume control
- macOS uses external binaries (`ddcctl`, `m1ddc`, `brightness`) in `src/binaries/`
- Windows uses `@hensm/ddcci` (DDC-CI protocol)

### Command System

String-based commands emitted through a global event system:
- Format: `command/category/action` (e.g., `command/changeBrightness/up`, `command/changeDarkMode/toggle`)
- Dispatched via `global.emitAppEvent` / `global.subscribeAppEvent`

### Key Types (`src/types.d.ts`)

- `Monitor`: id, name, brightness (0-100), type (laptop/external/unknown), disabled flag
- `Preference`: mode (win32/m1_mac/intel_mac), showIndividualDisplays, brightnessDelta, presets, keyBindings
- `AppConfig`: monitors array, darkMode boolean, volume, platform info

## Build System

- **Webpack 5** with separate configs: `webpack-ui.js` (renderer) and `webpack-electron.js` (main)
- **TypeScript** with separate configs: `tsconfig-ui.json` and `tsconfig-electron.json`
- **Path alias**: `src/*` resolves to `src/` directory
- **Prebuild step** (`prebuild.js`): copies platform-specific adapter files and binaries before webpack runs
- Output goes to `build/`, distribution packages to `dist/`

## Code Style

- Prettier: 100 char width, 2-space indent, trailing commas, single quotes, LF endings
- ESLint: TypeScript recommended rules
- UI slider inputs use 800ms debounce to limit API calls
- Global scope (`global`) stores mainWindow, tray, and event emitters

## Testing

- Jest with `ts-jest` transform
- Test files: `**/*+(spec|test).+(ts|tsx|js)`
- Module paths map `src/` imports
- Example: `src/main/utils/PreferenceUtils.spec.ts`
