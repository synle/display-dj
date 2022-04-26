// NOTE: this is the script that generates the bundle setup file
const fs = require('fs');
const path = require('path');
const util = require('util');

const child_process = require('child_process');
const exec = util.promisify(require('child_process').exec);

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
        // copy over the additional required binaries
        fs.copyFileSync(
          path.join(__dirname, `src/binaries/win32_volume_helper.exe`),
          path.join(__dirname, `dist/display-dj-win32-x64/resources/win32_volume_helper.exe`)
        );

        fs.copyFileSync(
          path.join(__dirname, `src/binaries/win32_ddcci.js`),
          path.join(__dirname, `dist/display-dj-win32-x64/resources/win32_ddcci.js`)
        );

        fs.writeFileSync(
          `./dist/display-dj-win32-x64/resources/package.json`,
          `{}`
        )

        // install the ddci module
        child_process.execSync(
          `npm i --no-package-lock --no-save  @hensm/ddcci`,
          {
            cwd: './dist/display-dj-win32-x64/resources',
            stdio:[0,1,2]
          }
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
        // copy over the additional required binaries
        fs.copyFileSync(
          path.join(__dirname, `src/binaries/darwin_ddcctl`),
          path.join(__dirname, `dist/display-dj-darwin-x64/display-dj.app/Contents/Resources`, `ddcctl`)
        );

        fs.copyFileSync(
          path.join(__dirname, `src/binaries/darwin_brightness`),
          path.join(__dirname, `dist/display-dj-darwin-x64/display-dj.app/Contents/Resources`, `brightness`)
        );

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


/**
 * https://stackoverflow.com/questions/13786160/copy-folder-recursively-in-node-js
 *
 * @param  {[type]} src  [description]
 * @param  {[type]} dest [description]
 * @return {[type]}      [description]
 */
function copyNestedDir(src, dest) {
  var exists = fs.existsSync(src);
  var stats = exists && fs.statSync(src);
  var isDirectory = exists && stats.isDirectory();
  if (isDirectory) {
    fs.mkdirSync(dest);
    fs.readdirSync(src).forEach(function(childItemName) {
      copyNestedDir(path.join(src, childItemName),
                        path.join(dest, childItemName));
    });
  } else {
    fs.copyFileSync(src, dest);
  }
};


doDistWork();
