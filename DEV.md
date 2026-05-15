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
npm run tauri build      # Production build (.dmg / .exe / .deb / .AppImage)
```
