# [display-dj](https://synle.github.io/display-dj/)

A windows application that allows quick toggle of dark mode and manages display brightness of individual display quickly

## Problems

- Back in the old days, it requires physical controls to change the brightness of external monitors. It is very time consuming and quirky to get it just right.
- As of right now, there is standard called DDC/CI that can be used to update information related to brightness, etc...
- There are apps out there that does these 2 things: adjust brightness and darkmode toggle, but they are completely different apps and require more context switching.

## Solutions

This application is at the moment for windows only and can be used to adjust external monitors brightness individually or all at once using software instead of you tinkering around with physical controls on your external monitors.

## Supported Platforms

- Windows 11

## Screenshots

![image](https://user-images.githubusercontent.com/3792401/158890109-50c68910-dd79-45f4-8da8-b41346219fc4.png)

![image](https://user-images.githubusercontent.com/3792401/158890188-6074254d-87df-4d74-92be-7ad8f825e25e.png)

## Monitor Configs

At the moment, there is no UI to modify the configs, configs file are located at `%AppData%\display-dj\monitor-configs.json`

- `disabled` flag can be used to hide a monitor off the list
- `sortOrder` flag can be used to change which monitor showing up first

```
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

## TODO

- [x] Basic MVP
- [x] Create a basic build pipeline
- [x] Support external monitors brightness
- [x] Support laptop brightness
- [ ] Support for windows 10
- [ ] Support for linux (is this possible?)
- [ ] Support for mac (is this possible?)
- [ ] Support for vertical task bar or top placement taskbar
- [x] Properly run the app on startup
- [ ] Change brightness based on time of day
- [ ] Change dark mode based on time of day
- [ ] Shortcut key for dark mode change
- [ ] Properly package the build as `msi` or `exe` file instead of plain zipped files.
