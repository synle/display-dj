// @ts-nocheck
import { app } from 'electron';

// NOTE: this section applicable for windows only and used to create the shortcut for windows OS
// This is the limitation of electron-winstaller instead of electron-builder which is much cleaner
const delay = 1000;

function _handleSquirrelEventForWindows() {
  if (process.platform !== 'win32') {
    return false;
  }

  if (process.argv.length === 1) {
    return false;
  }

  const ChildProcess = require('child_process');
  const path = require('path');

  const appFolder = path.resolve(process.execPath, '..');
  const rootAtomFolder = path.resolve(appFolder, '..');
  const updateDotExe = path.resolve(path.join(rootAtomFolder, 'Update.exe'));
  const exeName = path.basename(process.execPath);

  const spawn = function(command, args) {
    let spawnedProcess, error;

    try {
      spawnedProcess = ChildProcess.spawn(command, args, {detached: true});
    } catch (error) {}

    return spawnedProcess;
  };

  const spawnUpdate = function(args) {
    return spawn(updateDotExe, args);
  };

  const squirrelEvent = process.argv[1];
  switch (squirrelEvent) {
    case '--squirrel-install':
    case '--squirrel-updated':
      spawnUpdate(['--createShortcut', exeName]);
      setTimeout(app.quit, delay);
      return true;

    case '--squirrel-uninstall':
      spawnUpdate(['--removeShortcut', exeName]);
      setTimeout(app.quit, delay);
      return true;

    case '--squirrel-obsolete':
      app.quit();
      return true;
  }
};

if (_handleSquirrelEventForWindows()) {
  process.exit();
}