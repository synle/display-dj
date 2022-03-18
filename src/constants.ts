import { app } from 'electron';
import fs from 'fs';
import path from 'path';

function _getPath(fileName: string) {
  const baseDir = path.join(app.getPath('appData'), 'display-dj');

  try {
    fs.mkdirSync(baseDir);
  } catch (err) {}

  return path.join(baseDir, fileName);
}

export const LAPTOP_BUILT_IN_DISPLAY_ID = 'laptop-built-in';

export const MONITOR_CONFIG_FILE_DIR = _getPath('monitor-configs.json');

export const DISPLAY_TYPE = {
  LAPTOP: 'laptop_monitor',
  EXTERNAL: 'external_monitor',
};
