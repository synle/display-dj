const fs = require('fs');
const { exec } = require('child_process');

let source;
const dest = `src/electron/utils/DisplayAdapter.ts`;
let packages = [];

switch (process.platform) {
  case 'win32':
    source = `src/electron/utils/DisplayAdapter.Win32.ts`;
    packages = ['@hensm/ddcci'];
    break;
  case 'darwin':
    packages = ['dark-mode'];
    source = `src/electron/utils/DisplayAdapter.Darwin.ts`;
    break;
}

// install extra dependencies
exec(`npm install ${packages.join(' ')}`);

fs.copyFileSync(source, dest);
