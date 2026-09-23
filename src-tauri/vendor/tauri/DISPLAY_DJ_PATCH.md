# Display DJ Tauri patch

This directory contains the published `tauri` 2.11.5 crate with only the dependency changes from
https://github.com/tauri-apps/tauri/pull/16088:

- `tray-icon` 0.24 to 0.25
- `muda` 0.19 to 0.20
- matching renamed feature flags

The update selects `tray-icon` 0.25.1, which fixes macOS 27 left-click events being swallowed while
a tray menu is attached:
https://github.com/tauri-apps/tray-icon/pull/365

Remove this vendored crate and restore the crates.io dependency after a stable Tauri release includes
`tray-icon` 0.25.1 or newer.
