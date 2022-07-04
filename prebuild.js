const fs = require('fs');
const path = require('path');
const { exec } = require('child_process');

const DEST_IMPL_DISPLAY_UTILS = `src/main/utils/DisplayAdapter.ts`;
const DEST_IMPL_SOUND_UTILS = `src/main/utils/SoundUtils.ts`;

let packages = [];
let files = [];

switch (process.platform) {
  case 'win32':
    const DEV_ELECTRON_WIN32_RESOURCE_PATH = `node_modules/electron/dist/resources`;

    packages = `
      @hensm/ddcci
      electron-winstaller
      electron-squirrel-startup
    `;
    files.push([`src/main/utils/DisplayAdapter.Win32.ts`, DEST_IMPL_DISPLAY_UTILS]);
    files.push([`src/main/utils/SoundUtils.Win32.ts`, DEST_IMPL_SOUND_UTILS]);
    files.push([`src/binaries/win32_volume_helper.exe`, path.join(DEV_ELECTRON_WIN32_RESOURCE_PATH ,`win32_volume_helper.exe`)]);
    files.push([`src/binaries/win32_ddcci.js`, path.join(DEV_ELECTRON_WIN32_RESOURCE_PATH ,`win32_ddcci.js`)]);
    break;
  case 'darwin':
    const DEV_ELECTRON_DARWIN_RESOURCE_PATH = `node_modules/electron/dist/Electron.app/Contents/Resources`;

    packages = `
      dark-mode
      electron-installer-dmg
    `;
    files.push([`src/main/utils/DisplayAdapter.Darwin.ts`, DEST_IMPL_DISPLAY_UTILS]);
    files.push([`src/main/utils/SoundUtils.Darwin.ts`, DEST_IMPL_SOUND_UTILS]);
    files.push([`src/binaries/darwin_brightness`, path.join(DEV_ELECTRON_DARWIN_RESOURCE_PATH, `brightness`)]);
    files.push([`src/binaries/darwin_m1ddc`, path.join(DEV_ELECTRON_DARWIN_RESOURCE_PATH, `m1ddc`)]);
    files.push([`src/binaries/darwin_ddcctl`, path.join(DEV_ELECTRON_DARWIN_RESOURCE_PATH, `ddcctl`)]);
    break;
}

// install extra dependencies
packages = packages.split(/[ \n]+/).map( s=> s.trim()).filter(s => s);
exec(`npm install ${packages.join(' ')}`);

// copy over the impls
for(const [source, dest] of files){
  fs.copyFileSync(source, dest);
}
