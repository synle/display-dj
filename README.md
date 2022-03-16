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

![image](https://user-images.githubusercontent.com/3792401/158028380-e2347d2e-129a-456e-a49c-fe1350ab4fca.png)

![image](https://user-images.githubusercontent.com/3792401/158028372-d3fadbf7-d6c6-421c-8598-538f0c3c9bcd.png)

![image](https://user-images.githubusercontent.com/3792401/158028393-1db3c6ef-6d09-447d-bd24-0ed3697b5c9b.png)

## TODO

- [x] Basic MVP
- [x] Create a basic build pipeline
- [ ] Support laptop brightness (needs to add wmi-bridge c binding code)
- [ ] Support for windows 10
- [ ] Support for vertical task bar or top placement taskbar
- [ ] Properly run the app on startup
- [ ] Change brightness based on time of day
- [ ] Change dark mode based on time of day
- [ ] Shortcut key for dark mode change
