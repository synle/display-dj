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
npm install
npm start
```
