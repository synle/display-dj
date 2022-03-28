const fs = require('fs');
const path = require('path');

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

async function doDistWork() {
  try {
    switch (process.platform) {
      case 'win32':
        // copy over the binary required for windows volume settings
        fs.copyFileSync(
          path.join(__dirname, `src/binaries/win32_volume_helper.exe`),
          path.join(__dirname, `dist/display-dj-win32-x64/resources/win32_volume_helper.exe`)
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
        const createDMG = require('electron-installer-dmg');
        await createDMG({
          appPath: path.join(__dirname, 'dist', 'display-dj-darwin-x64', 'display-dj.app'),
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
