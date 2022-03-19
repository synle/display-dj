import PreferenceUtils from 'src/electron/utils/PreferenceUtils';
import darkMode from 'dark-mode';
import { exec } from 'child_process';
import { IDisplayAdapter } from 'src/types.d';

// Source: http://chopmo.dk/2017/01/12/control-monitor-brightness-from-osx.html
// Source: https://github.com/kfix/ddcctl
// Why 2 separate packages for brightness : refer to this https://github.com/nriley/brightness/issues/11
const ID_BUILT_IN_DISPLAY = 'built-in-mac-display';

async function _findWhichExternalDisplayById(targetMonitorId: string) {
  const monitorIds = (await DisplayAdapter.getMonitorList()).filter(
    (monitorId) => monitorId !== ID_BUILT_IN_DISPLAY,
  );
  for (let idx = 0; idx < monitorIds.length; idx++) {
    const monitorId = monitorIds[idx];
    if (monitorId === targetMonitorId) {
      // NOTE here the index start from 1 for the display api (ddcctl)
      return idx + 1;
    }
  }
  return undefined;
}

async function _findWhichBuiltinDisplayById() {
  return new Promise(async (resolve) => {
    const shellToRun = `${await _getBrightnessBinary()} -l`;

    exec(shellToRun, (error, stdout, stderr) => {
      try {
        for (let line of stdout.split('\n')) {
          if (line.includes(`display`) && line.includes(`built-in`) && line.includes(`ID`)) {
            line = line.replace('display', '').trim();
            line = line.substr(0, line.indexOf(':'));
            const whichDisplay = parseInt(line);
            return resolve(whichDisplay);
          }
        }
      } catch (err) {}
      resolve(0);
    });
  });
}

const _getDdcctlBinary = async () => (await PreferenceUtils.get()).ddcctlBinary;

const _getBrightnessBinary = async () => (await PreferenceUtils.get()).brightnessBinary;

const DisplayAdapter: IDisplayAdapter = {
  getMonitorList: async () => {
    return new Promise(async (resolve, reject) => {
      try {
        const shellToRun = `${await _getDdcctlBinary()}`;
        exec(shellToRun, (error, stdout, stderr) => {
          const monitors = (stdout || '')
            .split('\n')
            .filter((line) => line.indexOf('D:') === 0)
            .map((line, idx) => line.replace('D:', '').trim());
          monitors.unshift(ID_BUILT_IN_DISPLAY);
          resolve(monitors);
        });
      } catch (err) {
        reject(err);
      }
    });
  },
  getMonitorType: async (targetMonitorId: string) => {
    return targetMonitorId === ID_BUILT_IN_DISPLAY ? 'laptop_monitor' : 'external_monitor';
  },
  getMonitorBrightness: async (targetMonitorId: string) => {
    return new Promise(async (resolve, reject) => {
      try {
        if (targetMonitorId === ID_BUILT_IN_DISPLAY) {
          // for built in display
          const whichDisplay = await _findWhichBuiltinDisplayById();
          const shellToRun = `${await _getBrightnessBinary()} -l`;

          exec(shellToRun, (error, stdout, stderr) => {
            for (let line of stdout.split('\n')) {
              if (line.includes(`display ${whichDisplay}: brightness`)) {
                const brightness = Math.floor(
                  parseFloat(
                    line.substr(line.indexOf(': brightness') + ': brightness'.length + 1),
                  ) * 100,
                );

                if (brightness > 0) {
                  return resolve(brightness);
                }
              }
            }
            resolve(0);
          });
        } else {
          // for external display
          const whichDisplay = await _findWhichExternalDisplayById(targetMonitorId);
          if (whichDisplay === undefined) {
            return reject(`Display not found`);
          }
          const shellToRun = `${await _getDdcctlBinary()} -d ${whichDisplay} -b \\?`;
          exec(shellToRun, (error, stdout, stderr) => {
            if (error) {
              return reject(stderr);
            }

            console.debug('getMonitorBrightness', targetMonitorId, shellToRun, stdout);

            resolve(parseInt(stdout) || 0);
          });
        }
      } catch (err) {
        reject(`getMonitorBrightness failed: ` + JSON.stringify(err));
      }
    });
  },
  updateMonitorBrightness: async (targetMonitorId: string, newBrightness: number) => {
    return new Promise(async (resolve, reject) => {
      try {
        let shellToRun;

        if (targetMonitorId === ID_BUILT_IN_DISPLAY) {
          // for built in display
          const whichDisplay = await _findWhichBuiltinDisplayById();
          shellToRun = `${await _getBrightnessBinary()} -d ${whichDisplay} ${newBrightness / 100}`;
        } else {
          // for external display
          const whichDisplay = await _findWhichExternalDisplayById(targetMonitorId);
          if (whichDisplay === undefined) {
            return reject(`Display not found`);
          }

          shellToRun = `${await _getDdcctlBinary()} -d ${whichDisplay} -b ${newBrightness}`;
        }

        console.debug('updateMonitorBrightness', targetMonitorId, shellToRun);
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
