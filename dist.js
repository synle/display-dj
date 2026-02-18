// NOTE: this is the script that generates the bundle setup file
const fs = require('fs');
const path = require('path');
const child_process = require('child_process');

/**
 * @param {String} sourceDir: /some/folder/to/compress
 * @param {String} outPath: /path/to/created.zip
 * @returns {Promise}
 */
function _zipDirectory(sourceDir, outPath) {
  const archive = archiver('zip', { zlib: { level: 9 } });
  const stream = fs.createWriteStream(outPath);

  return new Promise((resolve, reject) => {
    archive
      .directory(sourceDir, false)
      .on('error', (err) => reject(err))
      .pipe(stream);

    stream.on('close', () => resolve());
    archive.finalize();
  });
}

function getOptionalPackageToInstall(){
  switch (process.platform) {
    case 'win32':
      return ['@hensm/ddcci']

    case 'darwin':
      return [];
  }
}

async function doDistWork() {
  try{
    fs.writeFileSync(
      `./dist/display-dj-win32-x64/resources/package.json`,
      `{}`
    )
  } catch(err){
    console.log('Failed to create the dummy package.json')
  }

  try{
    const packagesToInstall = getOptionalPackageToInstall();
    if(packagesToInstall.length > 0){
      child_process.execSync(
        `npm i --no-package-lock --no-save ${packagesToInstall.join(' ')}`,
        {
          cwd: './dist/display-dj-win32-x64/resources',
          stdio:[0,1,2]
        }
      );
    }
  } catch(err){
    console.log('Failed to install optional dependencies')
  }

  try {
    switch (process.platform) {
      case 'win32':
        // copy over the additional required binaries
        fs.copyFileSync(
          path.join(__dirname, `src/binaries/win32_volume_helper.exe`),
          path.join(__dirname, `dist/display-dj-win32-x64/resources/win32_volume_helper.exe`)
        );

        fs.copyFileSync(
          path.join(__dirname, `src/binaries/win32_ddcci.js`),
          path.join(__dirname, `dist/display-dj-win32-x64/resources/win32_ddcci.js`)
        );

        const electronInstaller = require('electron-winstaller');
        electronInstaller.createWindowsInstaller({
          appDirectory: path.join(__dirname, 'dist', 'display-dj-win32-x64'),
          outputDirectory: path.join(__dirname, 'dist'),
          iconUrl: path.join(__dirname, 'src', 'assets', 'icon.ico'),
          setupIcon: path.join(__dirname, 'src', 'assets', 'icon.ico'),
          loadingGif: path.join(__dirname, 'src', 'assets', 'installing.gif'),
          name: 'DisplayDJ',
          authors: 'Sy Le',
          exe: 'display-dj.exe',
          setupExe: 'display-dj-setup.exe',
          noMsi: true,
        });
        break;

      case 'darwin':
        const distArch = process.env.DIST_ARCH || 'x64';
        const darwinDistDir = `display-dj-darwin-${distArch}`;

        // copy over the additional required binaries
        if (distArch === 'x64') {
          fs.copyFileSync(
            path.join(__dirname, `src/binaries/darwin_ddcctl`),
            path.join(__dirname, `dist/${darwinDistDir}/display-dj.app/Contents/Resources`, `ddcctl`)
          );
        }

        fs.copyFileSync(
          path.join(__dirname, `src/binaries/darwin_m1ddc`),
          path.join(__dirname, `dist/${darwinDistDir}/display-dj.app/Contents/Resources`, `m1ddc`)
        );

        fs.copyFileSync(
          path.join(__dirname, `src/binaries/darwin_brightness`),
          path.join(__dirname, `dist/${darwinDistDir}/display-dj.app/Contents/Resources`, `brightness`)
        );

        const createDMG = require('electron-installer-dmg');
        await createDMG({
          appPath: path.join(__dirname, 'dist', darwinDistDir, 'display-dj.app'),
          name: 'display-dj',
          icon: path.join(__dirname, 'src', 'assets', 'icon.png'),
          overwrite: true,
        });
        break;
    }
  } catch (err) {
    console.log('Err', err);
  }
}

doDistWork();
