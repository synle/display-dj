const archiver = require('archiver');
const path = require('path');
const fs = require('fs');

/**
 * @param {String} sourceDir: /some/folder/to/compress
 * @param {String} outPath: /path/to/created.zip
 * @returns {Promise}
 */
function zipDirectory(sourceDir, outPath) {
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

zipDirectory(
  path.join(__dirname, 'dist', 'display-dj-win32-x64'),
  path.join(__dirname, 'dist', 'display-dj.zip'),
);
