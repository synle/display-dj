const fs = require('fs');
const { exec } = require('child_process');

const DEST_IMPL_DISPLAY_UTILS = `src/main/utils/DisplayAdapter.ts`;
let packages = [];
let files = [];

switch (process.platform) {
  case 'win32':
    files.push([`src/main/utils/DisplayAdapter.Win32.ts`, DEST_IMPL_DISPLAY_UTILS]);
    packages = ['@hensm/ddcci', 'electron-winstaller'];
    break;
  case 'darwin':
    files.push([`src/main/utils/DisplayAdapter.Darwin.ts`, DEST_IMPL_DISPLAY_UTILS]);
    packages = ['dark-mode', 'electron-installer-dmg'];
    break;
}

// install extra dependencies
exec(`npm install ${packages.join(' ')}`);

// copy over the impls
for(const [source, dest] of files){
  fs.copyFileSync(source, dest);
}
