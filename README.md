[![main-build](https://github.com/synle/display-dj/actions/workflows/main-build.yml/badge.svg)](https://github.com/synle/display-dj/actions/workflows/main-build.yml)

# [display-dj](https://synle.github.io/display-dj/)

![icon](https://user-images.githubusercontent.com/3792401/159400503-1524c1a0-7911-484e-8c8d-a62385856dcf.png)

`display-dj` is a cross platform desktop application that supports brightness adjustment for integrated laptop monitor as well as external monitors and dark mode toggle supporting Windows and MacOSX at the moment. Adjustment brightness will be quicker and does not require tinkering with the external monitor controls.

I published an article describing this project in details. Here's [the link to the published article about display-dj](https://www.linkedin.com/pulse/my-journey-building-display-dj-cross-platform-application-sy-le).

## The Problems

- As of right now, it requires physical controls to change the brightness of external monitors. It is very time consuming and quirky to get the brightness notch just right. Also some external monitors bury the brightness under sub-menus within the on screen display, which requires a lot of mental strength to figure out.
- There are applications out there that do these things: adjust external monitor brightness, integrated laptop monitor brightness and dark mode, but they are completely different apps and require more context switching. These applications sometimes don't support shortcuts or key bindings. And most importantly none of them are cross platform and only support either Windows or Mac OSX.
- Windows and Mac OSX have their own built in controls that allow you to adjust brightness and dark mode. But this built-in option only works for integrated displays such as your laptop monitors or certain proprietary monitors such as Apple Displays. The built-in solution does not work for third party displays.
- Another issue with built-in solution is sometimes the user interface is not intuitive and requires extra clicks and navigations to get to because they are buried deep inside a set of nested menus.

## Downloads

You can download `display-dj` at the following URL.

- [display-dj for Mac](https://github.com/synle/display-dj/releases/latest/download/display-dj-darwin.dmg)
- [display-dj for Windows](https://github.com/synle/display-dj/releases/latest/download/display-dj-setup.exe)

## Motivation

The challenge of work from home in the last 2 years with 2 young toddlers is that they can charge into your room any time of the day and playing with the light switch. This is my defense mechanism for those sudden changes in light intensity. I can toggle between 2 different modes rather quickly with a key stroke: going to the dark side vs going to the light side of the force.

![image](https://user-images.githubusercontent.com/3792401/159738107-9b6c476d-3031-4529-9213-9cc047e9bac2.png)
![image](https://user-images.githubusercontent.com/3792401/159738119-e141918b-8f38-4486-8412-6444dc383757.png)

## Screenshots / Demo

### Windows 11

![image](https://user-images.githubusercontent.com/3792401/159134000-ef989378-0e4d-4bd1-96f8-9c79110cb37e.png)

![image](https://user-images.githubusercontent.com/3792401/159134004-26547233-c46e-4e6a-b6b8-326502d08a8b.png)

![image](https://user-images.githubusercontent.com/3792401/159134011-761cadb7-5dc5-4431-a4e0-4976b379e39d.png)

### Mac OSX Monterey

![demo-mac](https://user-images.githubusercontent.com/3792401/159141171-c6c8a6a5-4b7b-4fc6-af28-c082fc1bd723.gif)

## Supported Platforms

The following version of OS has been tested and working.

- Windows 11
- Mac OSX (Monterey) - requires a separate installation of `ddcctl`

### MacOSX dependencies

This application requires `ddcctl` and `brightness` installed for it to be fully functional. You can use the following bash command to install it with `homebrew`

```bash
# for external Display
brew install ddcctl

# for Mac Integrated Display
brew install brightness
```

## Features / Configs / Preferences

### Renaming the display

By default, we will give each display a name. You can rename the display to something more friendly by clicking on the name of the display and finish by hitting Enter key.

![image](https://user-images.githubusercontent.com/3792401/159371503-32609f31-552a-49d4-97e8-285f906c7c94.png)

![image](https://user-images.githubusercontent.com/3792401/159371522-18fb1947-ba3a-4042-a2a8-68b425dde6e2.png)

![image](https://user-images.githubusercontent.com/3792401/159371549-4f92a00e-2f14-4cee-b258-1af5440122d3.png)

### Monitor Configs

At the moment, there is no UI to modify the configs. This can also be accessed via right clicking the tray icon of display-dj and choose `Open Monitor Configs`.

- `disabled`: flag can be used to hide a monitor off the list
- `sortOrder`: flag can be used to change which monitor showing up first

#### Sample configs file

Configs file are located at:

- `%AppData%\display-dj\monitor-configs.json` (for windows)
- `~/Library/Application\ Support/display-dj/monitor-configs.json` (For Mac)

```json
{
  "\\\\?\\DISPLAY#VSCB73A#5&21f33940&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}": {
    "id": "\\\\?\\DISPLAY#VSCB73A#5&21f33940&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}",
    "name": "Left",
    "brightness": 0,
    "sortOrder": 1,
    "disabled": false
  },
  "\\\\?\\DISPLAY#VSCB73A#5&23c70c64&0&UID257#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}": {
    "id": "\\\\?\\DISPLAY#VSCB73A#5&23c70c64&0&UID257#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}",
    "name": "Right",
    "brightness": 0,
    "sortOrder": 2,
    "disabled": false
  },
  "laptop-built-in": {
    "id": "laptop-built-in",
    "name": "Laptop Built-In Display",
    "brightness": 0,
    "sortOrder": 3,
    "disabled": true
  }
}
```

### Default Key Bindings

Below are default Key Bindings, you can modify the default keybinding in `preferences.json`, refer to the sample preferences file section.

Key Bindings' command can be an array in case you want to mix and match multiple commands. In this case, `Shift + F1`, minimize brightness and also turn off darkmode.

| Keys           | Command                                         |
| -------------- | ----------------------------------------------- |
| Shift + Escape | Toggle Dark Mode                                |
| Shift + F1     | Change brightness to 10% and turn dark mode off |
| Shift + F2     | Change brightness to 100% and turn dark mode on |
| Shift + F3     | Change brightness to 0%                         |
| Shift + F4     | Change brightness to 50%                        |
| Shift + F5     | Change brightness to 100%                       |

### Preferences / Key Bindings

At the moment, there is no UI to modify the preferences. This can also be accessed via right clicking the tray icon of display-dj and choose `Open App Preferences`.

- `ddcctlBinary`: applicable to mac system only. ddcctl binary used for display-dj.
- `showIndividualDisplays`: flag can be used to show a single brightness control for all displays or individual ones.
- `brightnessDelta`: a delta value / step value for brightness adjustment (applicable only for keyboard shortcut).
- `keyBindings`: a list of shortcuts / bindings to adjust brightness based on the keyboard shortcuts. Below is a list of all supported commands. Refer to [this list for a set of all supported commands](https://github.com/synle/display-dj/blob/main/src/types.d.ts#L3)

```bash
# brightness commands
command/changeBrightness/down
command/changeBrightness/up
command/changeBrightness/0
command/changeBrightness/10
command/changeBrightness/50
command/changeBrightness/100

# dark mode commands
command/changeDarkMode/toggle
command/changeDarkMode/dark
command/changeDarkMode/light
```

#### Sample preferences file

Preferences file are located at:

- Windows: `%AppData%\display-dj\preferences.json`
- Mac: `~/Library/Application\ Support/display-dj/preferences.json`

```json
{
  "ddcctlBinary": "/usr/local/bin/ddcctl",
  "showIndividualDisplays": false,
  "brightnessDelta": 50,
  "keyBindings": [
    {
      "key": "Shift+Escape",
      "command": "command/changeDarkMode/toggle"
    },
    {
      "key": "Shift+F1",
      "command": ["command/changeDarkMode/dark", "command/changeBrightness/10"]
    },
    {
      "key": "Shift+F2",
      "command": ["command/changeDarkMode/light", "command/changeBrightness/100"]
    }
    {
      "key": "Shift+F3",
      "command": "command/changeBrightness/0"
    },
    {
      "key": "Shift+F4",
      "command": "command/changeBrightness/50"
    },
    {
      "key": "Shift+F5",
      "command": "command/changeBrightness/100"
    }
  ]
}
```

## TODO

- [x] Basic MVP.
- [x] Create a basic build pipeline.
- [x] Support external monitors brightness.
- [x] Support laptop brightness.
- [ ] Support for windows 10.
- [ ] Support for linux (is this possible?).
- [x] Support for mac (possible with some quirks).
- [ ] Support for vertical task bar or top placement taskbar.
- [x] Properly run the app on startup.
- [ ] Change brightness based on time of day.
- [ ] Change dark mode based on time of day.
- [ ] Allow creation of display profiles so the profile can be selected from context menu or a shortcut. This profile should apply all individual display brightness.
- [x] Shortcut key for dark mode change.
- [x] Keyboard shortcuts to be dynamically managed in the preference file.
- [x] Windows build - Properly package the build as `exe` file instead of plain zipped files.
- [x] Mac build - Properly package the build as `dmg` file instead of plain zipped files.
- [x] Properly hookup the icons for mac.
- [ ] Properly set up CI/CD pipeline for releases and main page content.
- [x] MacOSX - Fix an issue where the positioning of the app is misplaced in the main display regardless of mouse click.
- [x] Support changing contrast (is this possible?).

## Contributing?

If you are interested in contributing, you can refer to this doc to get started

- [CONTRIBUTING.md](https://github.com/synle/display-dj/blob/main/CONTRIBUTING.md)

## Known issues

Due to the complexity and quirks of `ddc/ci` protocol, unfortunately it's nearly impossible to support every single monitor out there. So if you run into issue where this app doesn't work, we will not guarantee support.

## Suggestion?

Use the following link to file a bug or a suggestion. Please indicate which OS and monitor.

- [File a bug or a suggestion?](https://github.com/synle/display-dj/issues/new)
