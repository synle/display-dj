---
name: install-app
description: Download, install, and launch the latest Display DJ release for the current platform. Use when asked to install, update, reinstall, or test the released desktop app.
---

# Install Display DJ

Download the latest official release artifact, install it, and launch the app.

## Instructions

1. Detect the current platform and architecture with `uname -s` and `uname -m` or the native Windows equivalent.
2. Resolve the latest non-draft, non-prerelease GitHub release.
3. Create a unique temporary directory with `mktemp -d`. Only remove that exact directory after the installation attempt.
4. Download the matching artifact:

| Platform | Architecture | Asset pattern                         |
| -------- | ------------ | ------------------------------------- |
| macOS    | arm64        | `*_aarch64.dmg`                       |
| macOS    | x86_64       | `*_x64.dmg`                           |
| Windows  | x86_64       | `*_x64-setup.exe`                     |
| Windows  | arm64        | `*_arm64-setup.exe`                   |
| Linux    | x86_64       | `*_amd64.deb` or `*_amd64.AppImage`   |
| Linux    | arm64        | `*_arm64.deb` or `*_aarch64.AppImage` |

Use `gh release download <tag> --pattern "<pattern>" --dir <temp-dir> --clobber`.

5. Install by platform. Ask before overwriting an untracked installation or using elevated package installation.

### macOS

- Ask Display DJ to quit through the normal application quit mechanism; do not kill processes by name.
- Mount the downloaded DMG with `hdiutil attach <dmg> -nobrowse` and record the actual mount point.
- Copy `Display DJ.app` to `/Applications/Display DJ.app`.
- Detach the recorded mount point.
- Run `xattr -cr "/Applications/Display DJ.app"` to clear quarantine for unsigned builds.
- After every new build, run `tccutil reset Accessibility com.synle.display-dj` to reset Accessibility.
- Open Accessibility Settings with `open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"`.
- Launch the app with `open "/Applications/Display DJ.app"`.

### Windows

Run the downloaded setup executable. It owns upgrade and replacement behavior; do not force-kill processes by name.

### Linux

- For `.deb`, ask before running `sudo dpkg -i <deb-file>`.
- For AppImage, mark the downloaded file executable and launch it from a user-selected permanent location. Do not silently leave the installed app inside the temporary directory.

6. Remove only the unique temporary directory created in step 3.
7. Report the installed version and location. On macOS, remind the user to grant Accessibility permission in the opened System Settings pane.
