const fs = require('fs');
const { exec } = require('child_process');

let source;
const dest = `src/electron/utils/DisplayAdapter.ts`;
let packages = [];

switch (process.platform) {
  case 'win32':
    source = `src/electron/utils/DisplayAdapter.Win32.ts`;
    packages = ['@hensm/ddcci', 'electron-winstaller'];
    break;
  case 'darwin':
    source = `src/electron/utils/DisplayAdapter.Darwin.ts`;
    packages = ['dark-mode'];
    break;
}

// install extra dependencies
exec(`npm install ${packages.join(' ')}`);

fs.copyFileSync(source, dest);
