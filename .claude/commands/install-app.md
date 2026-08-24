---
description: Download the latest release, install, and launch Display DJ for the current platform
allowed-tools: [Bash, Read, Glob, Grep]
---

# Setup Display DJ

Download the latest release artifact from GitHub, install it, and launch the app.

## Instructions

1. **Kill running instances** (fire-and-forget, ignore errors):
   ```bash
   pkill -f "Display DJ" 2>/dev/null || true
   ```
2. Determine the current platform and architecture by running `uname -s` and `uname -m`.
3. Get the latest release tag: `gh release list --limit 1 --json tagName --jq '.[0].tagName'`
4. Download the correct artifact based on platform:

| Platform | Architecture | Asset pattern                       |
| -------- | ------------ | ----------------------------------- |
| macOS    | arm64        | `*_aarch64.dmg`                     |
| macOS    | x86_64       | `*_x64.dmg`                         |
| Windows  | x86_64       | `*_x64-setup.exe`                   |
| Linux    | x86_64       | `*_amd64.deb` or `*_amd64.AppImage` |

Use: `gh release download <tag> --pattern "<pattern>" --dir /tmp/display-dj-release --clobber`

5. Install based on platform:

### macOS

```bash
# Mount DMG
hdiutil attach /tmp/display-dj-release/<dmg_file> -nobrowse

# Copy to Applications
cp -R "/Volumes/Display DJ/Display DJ.app" "/Applications/Display DJ.app"

# Unmount
hdiutil detach "/Volumes/Display DJ"

# Strip Apple quarantine (required for unsigned builds)
xattr -cr "/Applications/Display DJ.app"

# Reset Accessibility permission (required after each new build)
tccutil reset Accessibility com.synle.display-dj

# Open Accessibility settings so user can re-grant permission
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"

# Launch the app
open "/Applications/Display DJ.app"
```

### Windows

```bash
# Just run the installer exe — it handles everything
start <exe_file>
```

### Linux

```bash
# For .deb
sudo dpkg -i <deb_file>

# For .AppImage
chmod +x <appimage_file>
./<appimage_file>
```

6. Clean up: `rm -rf /tmp/display-dj-release`
7. Report to the user that the app is installed and running. On macOS, remind them to grant Accessibility permission in the System Settings window that was opened.
