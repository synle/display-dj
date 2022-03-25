import { app } from 'electron';
import fs from 'fs';
import path from 'path';

export const MONITOR_CONFIG_FILE_PATH = _getPath('monitor-configs.json');
export const PREFERENCE_FILE_PATH = _getPath('preferences.json');
export const LOG_FILE_PATH = _getPath('logs.txt');

function _getPath(fileName: string) {
  const baseDir = path.join(app === undefined ? __dirname : app.getPath('appData'), 'display-dj');

  try {
    fs.mkdirSync(baseDir);
  } catch (err) {}

  return path.join(baseDir, fileName);
}

const StorageUtils = {
  readJSON: (file: string) => {
    try {
      return JSON.parse(fs.readFileSync(file, { encoding: 'utf8', flag: 'r' }).trim());
    } catch (err) {
      return undefined;
    }
  },
  writeJSON: (file: string, jsonObject: any) => {
    fs.writeFileSync(file, JSON.stringify(jsonObject, null, 2));
  },
  write: (file: string, text: string) => {
    fs.writeFileSync(file, text);
  },
  append: (file: string, text: string) => {
    fs.appendFileSync(file, text);
  },
  delete: (file: string) => {
    try {
      fs.unlinkSync(file);
    } catch (err) {}
  },
  /**
   * return value in mb
   */
  size: (file: string) => {
    try {
      return fs.statSync(file).size / 1000000;
    } catch (err) {
      return undefined;
    }
  },
};

export default StorageUtils;
