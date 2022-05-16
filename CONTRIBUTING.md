## Requirements

### Node

- Node 14+

### Windows

Requires C build tools. can be installed as part of node js executable

### Mac

This application requires `ddcctl` (for external display) and `brightness` (for integrated display). But now these are included as part of the build and don't require additional steps.

## Getting started

### Running the app in test mode

After all the dependencies have been installed, the application can be started using the following command.

```bash
npm install --no-optional ; npm start
```

### Running the unit tests

```bash
npm run build ; npm test
```

### Reset Jest Test Snapshots

```bash
npm test -- -u
```

## Log Files

Log files are located in

- Windows: `%AppData%\display-dj\logs.txt`
- Mac: `~/Library/Application\ Support/display-dj/logs.txt`
