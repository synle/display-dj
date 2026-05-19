# display-dj

Cross-platform desktop app for monitor brightness, contrast, dark mode, volume, and window tiling control. Built with Tauri v2 (Rust backend) and React 19 + TypeScript + Vite 6 (WebView frontend). All platform code (DDC/CI, gamma, WMI, DisplayServices, wallpaper) is vendored in-process under `src-tauri/src/core/`.

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
