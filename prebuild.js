const fs = require('fs');
const { exec } = require('child_process');

const DEST_IMPL_DISPLAY_UTILS = `src/main/utils/DisplayAdapter.ts`;
const DEST_IMPL_SOUND_UTILS = `src/main/utils/SoundUtils.ts`;

let packages = [];
let files = [];

switch (process.platform) {
  case 'win32':
    packages = `
      @hensm/ddcci
      electron-winstaller
    `;
    files.push([`src/main/utils/DisplayAdapter.Win32.ts`, DEST_IMPL_DISPLAY_UTILS]);
    files.push([`src/main/utils/SoundUtils.Win32.ts`, DEST_IMPL_SOUND_UTILS]);
    break;
  case 'darwin':
    packages = `
      dark-mode
      loudness
      electron-installer-dmg
    `;
    files.push([`src/main/utils/DisplayAdapter.Darwin.ts`, DEST_IMPL_DISPLAY_UTILS]);
    files.push([`src/main/utils/SoundUtils.Darwin.ts`, DEST_IMPL_SOUND_UTILS]);
    break;
}

// install extra dependencies
packages = packages.split(/[ \n]+/).map( s=> s.trim()).filter(s => s);
exec(`npm install ${packages.join(' ')}`);

// copy over the impls
for(const [source, dest] of files){
  fs.copyFileSync(source, dest);
}
