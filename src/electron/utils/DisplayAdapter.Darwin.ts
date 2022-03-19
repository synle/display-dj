import darkMode from 'dark-mode';
import { exec } from 'child_process';
import { DISPLAY_TYPE } from 'src/constants';
import { IDisplayAdapter } from 'src/types.d';

const DisplayAdapter: IDisplayAdapter = {
  getMonitorList: async () => {
    return new Promise((resolve, reject) => {
      const shellToRun = `ddcctl`;
      exec(shellToRun, (error, stdout, stderr) => {
        const monitors = (stdout || '')
          .split('\n')
          .filter((line) => line.indexOf('D:') === 0)
          .map((line, idx) => line.replace('D:', '').trim());

        if(monitors.length > 0){
          resolve(monitors);
        } else {
          reject({stdout, stderr})
        }
      });
    });
  },
  getMonitorType: async (targetMonitorId: string) => {
    // TODO: to be implemented
    return DISPLAY_TYPE.EXTERNAL;
  },
  getMonitorBrightness: async (targetMonitorId: string) => {
    // TODO: to be implemented
    return 0;
  },
  updateMonitorBrightness: async (targetMonitorId: string, newBrightness: number) => {
    return new Promise(async(resolve, reject) => {
      let whichMonitor : number | undefined;

      const monitorIds = await DisplayAdapter.getMonitorList();
      for(let idx = 0; idx < monitorIds.length; idx++){
        const monitorId = monitorIds[idx];

        if(monitorId === targetMonitorId){
          // NOTE here the index start from 1 for the display api (ddcctl)
          whichMonitor = idx + 1;
          break;
        }
      }

      if(whichMonitor === undefined){
        return reject(`Monitor not found - ${targetMonitorId}`);
      }

      const shellToRun = `ddcctl -d ${whichMonitor} -b ${newBrightness}`;
      exec(shellToRun, (error, stdout, stderr) => {
        if (error) {
          return reject(stderr);
        }
        resolve();
      });
    });
  },
  getDarkMode: async () => {
    return darkMode.isEnabled();
  },
  updateDarkMode: async (isDarkModeOn: boolean) => {
    return darkMode.toggle(isDarkModeOn);
  },
};

export default DisplayAdapter;
