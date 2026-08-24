### Supported Platforms

- **macOS** Apple Silicon (ARM64) - `.dmg`
- **macOS** Intel (x64) - `.dmg`
- **Windows** x64 / ARM64 - `.exe`
- **Linux** x64 / ARM64 - `.deb` / `.AppImage`

### Requirements

- Display control is built in (vendored platform code) — no external tools needed on macOS or Windows
- **macOS Window Tiling**: Requires Accessibility permission. Go to System Settings > Privacy & Security > Accessibility and add Display DJ. See [setup instructions](https://github.com/synle/display-dj#window-tiling-macos)
- **Linux**: Install `ddcutil` and `brightnessctl` for display control: `sudo apt install ddcutil brightnessctl i2c-tools`
