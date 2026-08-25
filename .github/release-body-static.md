### Supported Platforms

- **macOS** Apple Silicon (ARM64) - `.dmg`
- **macOS** Intel (x64) - `.dmg`
- **Windows** x64 / ARM64 - `.exe`
- **Linux** x64 / ARM64 - `.deb` / `.AppImage` **(BETA)**

> ⚠️ **Linux support is in beta.** Builds are produced and smoke-tested on X11
> desktops, but Linux is not yet battle-tested — features may break, be
> incomplete, or behave differently across desktop environments and Wayland.
> Use at your own discretion; issue reports welcome.

### Requirements

- Display control is built in (vendored platform code) — no external tools needed on macOS or Windows
- **macOS Window Tiling**: Requires Accessibility permission. Go to System Settings > Privacy & Security > Accessibility and add Display DJ. See [setup instructions](https://github.com/synle/display-dj#window-tiling-macos)
- **Linux**: Install `ddcutil` and `brightnessctl` for display control: `sudo apt install ddcutil brightnessctl i2c-tools`
