const fs = require('fs');
const { exec } = require('child_process');

let source;
const dest = `src/main/utils/DisplayAdapter.ts`;
let packages = [];

switch (process.platform) {
  case 'win32':
    source = `src/main/utils/DisplayAdapter.Win32.ts`;
    packages = ['@hensm/ddcci', 'electron-winstaller'];
    break;
  case 'darwin':
    source = `src/main/utils/DisplayAdapter.Darwin.ts`;
    packages = ['dark-mode', 'electron-installer-dmg'];
    break;
}

// install extra dependencies
exec(`npm install ${packages.join(' ')}`);

fs.copyFileSync(source, dest);
