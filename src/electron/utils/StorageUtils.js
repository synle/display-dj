import { app } from 'electron';
import fs from 'fs';
import path from 'path';

const baseDir = path.join(app.getPath('appData'), 'display-dj');

try {
  fs.mkdirSync(baseDir);
} catch (err) {}

function _getPath(fileName) {
  return path.join(baseDir, fileName);
}

const StorageUtils = {
  readJSON: (file) => {
    file = _getPath(file);

    try {
      return JSON.parse(fs.readFileSync(file, { encoding: 'utf8', flag: 'r' }).trim());
    } catch (err) {
      return {};
    }
  },
  writeJSON: (file, jsonObject) => {
    file = _getPath(file);

    fs.writeFileSync(file, JSON.stringify(jsonObject, null, 2));
  },
};

export default StorageUtils;
