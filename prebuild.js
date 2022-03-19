const fs =require('fs');
const { exec } = require('child_process');

let source;
const dest = `src/electron/utils/DisplayAdapter.ts`

switch(process.platform){
  case 'win32':
  source = `src/electron/utils/DisplayAdapter.Win32.ts`
  exec(`npm install @hensm/ddcci`, (error) => {
    console.log('>> installed @hensm/ddcci', error)
  });
  break;
  case 'darwin':
  source = `src/electron/utils/DisplayAdapter.Darwin.ts`
  break;
}


fs.copyFileSync(source, dest);
