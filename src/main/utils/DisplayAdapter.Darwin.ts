import darkMode from 'dark-mode';
import path from 'path';
import { executeBash } from 'src/main/utils/ShellUtils';
import { IDisplayAdapter, Monitor } from 'src/types.d';
import { exec } from 'child_process';
// Source: http://chopmo.dk/2017/01/12/control-monitor-brightness-from-osx.html
// Source: https://github.com/kfix/ddcctl
// Why 2 separate packages for brightness : refer to this https://github.com/nriley/brightness/issues/11
const ID_BUILT_IN_DISPLAY = 'built-in-mac-display';
let _cacheTime = 0;
let _cache: Record<string, any> = {
  whichDisplayLaptopBuiltin: undefined,
  allMonitorList: undefined,
};

function getCache() {
  if (Date.now() - _cacheTime <= 3000) {
    // clear cache
    _cache = {};
  }

  return _cache;
}
async function _findWhichExternalDisplayById(targetMonitorId: string) {
  const monitorIds = (await _getMonitorList()).filter(
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
    if (getCache().whichDisplayLaptopBuiltin) {
      return resolve(getCache().whichDisplayLaptopBuiltin);
    }

    const shellToRun = `${await _getBrightnessBinary()} -l`;
    try {
      const stdout = await executeBash(shellToRun);

      for (let line of stdout.split('\n')) {
        if (line.includes(`display`) && line.includes(`built-in`) && line.includes(`ID`)) {
          line = line.replace('display', '').trim();
          line = line.substr(0, line.indexOf(':'));
          const whichDisplay = parseInt(line);
          getCache().whichDisplayLaptopBuiltin = whichDisplay;
          return resolve(whichDisplay);
        }
      }
    } catch (err) {}

    resolve(0);
  });
}

async function _getMonitorList(): Promise<string[]> {
  return new Promise(async (resolve, reject) => {
    if (getCache().allMonitorList) {
      return resolve(getCache().allMonitorList);
    }

    try {
      const shellToRun = `${await _getDdcctlBinary()}`;
      exec(shellToRun, (error, stdout, stderr) => {
        const monitors = (stdout || '')
          .split('\n')
          .filter((line) => line.indexOf('D:') === 0)
          .map((line, idx) => line.replace('D:', '').trim());
        monitors.unshift(ID_BUILT_IN_DISPLAY);

        getCache().allMonitorList = monitors;

        resolve(monitors);
      });
    } catch (err) {
      reject(err);
    }
  });
}

async function _getUpdateBrightnessShellScript(
  targetMonitorId: string,
  newBrightness: number,
): Promise<string> {
  return new Promise(async (resolve, reject) => {
    try {
      if (targetMonitorId === ID_BUILT_IN_DISPLAY) {
        // for built in display
        const whichDisplay = await _findWhichBuiltinDisplayById();
        return resolve(`${await _getBrightnessBinary()} -d ${whichDisplay} ${newBrightness / 100}`);
      } else {
        // for external display
        const whichDisplay = await _findWhichExternalDisplayById(targetMonitorId);
        if (whichDisplay === undefined) {
          return reject(`Display not found`);
        }

        return resolve(`${await _getDdcctlBinary()} -d ${whichDisplay} -b ${newBrightness}`);
      }
    } catch (err) {
      reject(err);
    }
  });
}

const _getDdcctlBinary = async () => path.join(process['resourcesPath'], `ddcctl`);

const _getBrightnessBinary = async () => path.join(process['resourcesPath'], `brightness`);

const DisplayAdapter: IDisplayAdapter = {
  getMonitorList: _getMonitorList,
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

          console.debug('getMonitorBrightness', targetMonitorId, shellToRun);

          const stdout = await executeBash(shellToRun);
          for (let line of stdout.split('\n')) {
            if (line.includes(`display ${whichDisplay}: brightness`)) {
              const brightness = Math.floor(
                parseFloat(line.substr(line.indexOf(': brightness') + ': brightness'.length + 1)) *
                  100,
              );

              if (brightness >= 0 && brightness <= 100) {
                return resolve(brightness);
              }
            }
          }
          reject('cannot read the brightness for laptop display from brightness');
        } else {
          // for external display
          const whichDisplay = await _findWhichExternalDisplayById(targetMonitorId);
          if (whichDisplay === undefined) {
            return reject(`Display not found`);
          }

          const shellToRun = `${await _getDdcctlBinary()} -d ${whichDisplay} -b \\?`;
          console.debug('getMonitorBrightness', targetMonitorId, shellToRun);

          const stdout = await executeBash(shellToRun);
          for (let line of stdout.split('\n')) {
            if (line.includes('No data after') && line.includes('tries')) {
              break;
            } else if (line.includes(`VCP control`) && line.includes('current')) {
              const brightness = parseInt(
                line.substr(line.indexOf('current: ') + 'current: '.length),
              );

              if (brightness >= 0 && brightness <= 100) {
                return resolve(brightness);
              }
            }
          }

          reject('cannot read the brightness for external display from ddcci');
        }
      } catch (err) {
        reject(`getMonitorBrightness failed: ` + JSON.stringify(err));
      }
    });
  },
  updateMonitorBrightness: async (targetMonitorId: string, newBrightness: number) => {
    const shellToRun = await _getUpdateBrightnessShellScript(targetMonitorId, newBrightness);
    console.debug('updateMonitorBrightness', targetMonitorId, shellToRun);
    await executeBash(shellToRun);
  },
  batchUpdateMonitorBrightness: async (monitors: Monitor[]) => {
    const shellsToRun = [];

    for (const monitor of monitors) {
      try {
        shellsToRun.push(await _getUpdateBrightnessShellScript(monitor.id, monitor.brightness));
      } catch (err) {}
    }

    await executeBash(shellsToRun.join('; '));
  },
  getDarkMode: async () => {
    return darkMode.isEnabled();
  },
  updateDarkMode: async (isDarkModeOn: boolean) => {
    return darkMode.toggle(isDarkModeOn);
  },
};

export default DisplayAdapter;