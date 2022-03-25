## Requirements

### Node

- Node 14+

### Windows

Requires C build tools. can be installed as part of node js executable

### Mac

This application requires `ddcctl` (for external display) and `brightness` (for integrated display). Can be installed as followed

```bash
# for external Display
brew install ddcctl

# for Mac Integrated Display
brew install brightness
```

For mac only, temporarily remove this windows only package for windows

```
"@hensm/ddcci": "^0.1.0"
```

## Getting started

After all the dependencies have been installed, the application can be started using the following command.

```bash
npm install --no-optional
npm run build
npm start
```

### Reset Jest Test Snapshots

```bash
npm test -- -u
```

## Log Files

Log files are located in

- Windows: `%AppData%\display-dj\logs.txt`
- Mac: `~/Library/Application\ Support/display-dj/logs.txt`
