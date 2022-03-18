import { exec } from 'child_process';
import { DISPLAY_TYPE } from 'src/constants';
import { DisplayAdapter } from 'src/types.d';

const DisplayUtils: DisplayAdapter = {
  getMonitorList: async () => {
    return new Promise((resolve, reject) => {
      const shellToRun = `ddcctl`;
      exec(shellToRun, (error, stdout, stderr) => {
        const monitors = (stdout || '')
          .split('\n')
          .filter((line) => line.indexOf('D:') === 0)
          .map((line, idx) => line.replace('D:', '').trim());
        resolve(monitors);
      });
    });
  },
  getMonitorType: async (idToUse: string) => {
    // TODO: to be implemented
    return DISPLAY_TYPE.EXTERNAL;
  },
  getMonitorBrightness: async (idToUse: string) => {
    // TODO: to be implemented
    return 0;
  },
  updateMonitorBrightness: async (monitorId: string, newBrightness: number) => {
    return new Promise((resolve, reject) => {
      const shellToRun = `ddcctl -d ${monitorId} -b ${newBrightness}`;
      exec(shellToRun, (error, stdout, stderr) => {
        if (error) {
          return reject(stderr);
        }
        resolve();
      });
    });
  },
  getDarkMode: async () => {
    // TODO: to be implemented
    return true;
  },
  updateDarkMode: async (isDarkModeOn: boolean) => {
    // TODO: to be implemented
  },
};

export default DisplayUtils;
