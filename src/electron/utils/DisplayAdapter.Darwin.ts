import PreferenceUtils from 'src/electron/utils/PreferenceUtils';
import darkMode from 'dark-mode';
import { exec } from 'child_process';
import { DISPLAY_TYPE } from 'src/constants';
import { IDisplayAdapter } from 'src/types.d';
async function _findDisplayById(targetMonitorId: string) {
  const monitorIds = await DisplayAdapter.getMonitorList();
  for (let idx = 0; idx < monitorIds.length; idx++) {
    const monitorId = monitorIds[idx];

    if (monitorId === targetMonitorId) {
      return idx;
    }
  }

  return -1;
}

const DisplayAdapter: IDisplayAdapter = {
  getMonitorList: async () => {
    return new Promise(async (resolve, reject) => {
      try {
        const preference = await PreferenceUtils.get();
        const ddcctlBinary = preference.ddcctlBinary;

        const shellToRun = `${ddcctlBinary}`;

        exec(shellToRun, (error, stdout, stderr) => {
          const monitors = (stdout || '')
            .split('\n')
            .filter((line) => line.indexOf('D:') === 0)
            .map((line, idx) => line.replace('D:', '').trim());

          if (monitors.length > 0) {
            resolve(monitors);
          } else {
            reject({ stdout, stderr });
          }
        });
      } catch (err) {
        reject(err);
      }
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
    return new Promise(async (resolve, reject) => {
      try {
        let whichMonitor: number | undefined;

        const matchedIdx = await _findDisplayById(targetMonitorId);

        if (matchedIdx === -1) {
          return reject(`Monitor not found - ${targetMonitorId}`);
        } else {
          // NOTE here the index start from 1 for the display api (ddcctl)
          whichMonitor = matchedIdx + 1;
        }

        const preference = await PreferenceUtils.get();
        const ddcctlBinary = preference.ddcctlBinary;

        const shellToRun = `${ddcctlBinary} -d ${whichMonitor} -b ${newBrightness}`;
        exec(shellToRun, (error, stdout, stderr) => {
          if (error) {
            return reject(stderr);
          }
          resolve();
        });
      } catch (err) {
        reject(err);
      }
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
