import { exec } from 'child_process';
import darkMode from 'dark-mode';
import path from 'path';
import PreferenceUtils from 'src/main/utils/PreferenceUtils';
import { executeBash } from 'src/main/utils/ShellUtils';
import { IDisplayAdapter, Monitor } from 'src/types.d';
// Source: http://chopmo.dk/2017/01/12/control-monitor-brightness-from-osx.html
// Source: https://github.com/kfix/ddcctl
// Why 2 separate packages for brightness : refer to this https://github.com/nriley/brightness/issues/11
const LAPTOP_DISPLAY_MONITOR_ID = 'built-in-mac-display';
const _cacheTime = 0;
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
const _getDdcctlBinaryForIntel = async () => path.join(process['resourcesPath'], `ddcctl`);

const _getDdcctlBinaryForM1 = async () => path.join(process['resourcesPath'], `m1ddc`);

const _getBrightnessBinary = async () => path.join(process['resourcesPath'], `brightness`);
async function _isM1Mac() {
  try {
    const preferences = await PreferenceUtils.get();
    return preferences.mode === 'm1_mac';
  } catch (err) {
    return false;
  }
}

async function _findWhichExternalDisplayById(targetMonitorId: string) {
  const monitorIds = (await _getMonitorList()).filter(
    (monitorId) => monitorId !== LAPTOP_DISPLAY_MONITOR_ID,
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
  if (getCache().whichDisplayLaptopBuiltin) {
    return getCache().whichDisplayLaptopBuiltin;
  }

  const shellToRun = `${await _getBrightnessBinary()} -l`;
  try {
    const stdout = await executeBash(shellToRun);

    for (const line of stdout.split('\n')) {
      if (line.includes(`display`) && line.includes(`built-in`) && line.includes(`ID`)) {
        let trimmed = line.replace('display', '').trim();
        trimmed = trimmed.substr(0, trimmed.indexOf(':'));
        const whichDisplay = parseInt(trimmed);
        getCache().whichDisplayLaptopBuiltin = whichDisplay;
        return whichDisplay;
      }
    }
  } catch (err) {
    // brightness binary may not be available
  }

  return 0;
}

async function _getMonitorList(): Promise<string[]> {
  if (getCache().allMonitorList) {
    return getCache().allMonitorList;
  }

  const shellToRun = `${await _getDdcctlBinaryForIntel()}`;
  return new Promise((resolve, reject) => {
    try {
      exec(shellToRun, (_error, stdout) => {
        const monitors = (stdout || '')
          .split('\n')
          .filter((line) => line.indexOf('D:') === 0)
          .map((line) => line.replace('D:', '').trim());
        monitors.unshift(LAPTOP_DISPLAY_MONITOR_ID);

        getCache().allMonitorList = monitors;

        resolve(monitors);
      });
    } catch (err) {
      reject(err);
    }
  });
}

/**
 * @param  {string} targetMonitorId
 * @param  {number} newBrightness  brightness level from 0 to 100%
 * @return {Promise<string>} get a script to execute to update brightness
 */
async function _getUpdateBrightnessShellScript(
  targetMonitorId: string,
  newBrightness: number,
): Promise<string> {
  if ((await _isM1Mac()) === true) {
    // for external display
    const whichDisplay = await _findWhichExternalDisplayById(targetMonitorId);
    if (whichDisplay === undefined) {
      throw new Error(`Display not found`);
    }
    return `${await _getDdcctlBinaryForM1()} display ${whichDisplay} set luminance ${newBrightness}`;
  }

  if (targetMonitorId === LAPTOP_DISPLAY_MONITOR_ID) {
    // for built in display
    const whichDisplay = await _findWhichBuiltinDisplayById();
    return `${await _getBrightnessBinary()} -d ${whichDisplay} ${newBrightness / 100}`;
  } else {
    // for external display
    const whichDisplay = await _findWhichExternalDisplayById(targetMonitorId);
    if (whichDisplay === undefined) {
      throw new Error(`Display not found`);
    }
    return `${await _getDdcctlBinaryForIntel()} -d ${whichDisplay} -b ${newBrightness}`;
  }
}

const DisplayAdapter: IDisplayAdapter = {
  getMonitorList: _getMonitorList,
  getMonitorType: async (targetMonitorId: string) => {
    return targetMonitorId === LAPTOP_DISPLAY_MONITOR_ID ? 'laptop_monitor' : 'external_monitor';
  },
  getMonitorBrightness: async (targetMonitorId: string) => {
    if (targetMonitorId === LAPTOP_DISPLAY_MONITOR_ID) {
      // for built in display
      const whichDisplay = await _findWhichBuiltinDisplayById();
      const shellToRun = `${await _getBrightnessBinary()} -l`;

      console.debug('getMonitorBrightness', targetMonitorId, shellToRun);

      const stdout = await executeBash(shellToRun);
      for (const line of stdout.split('\n')) {
        if (line.includes(`display ${whichDisplay}: brightness`)) {
          const brightness = Math.floor(
            parseFloat(line.substr(line.indexOf(': brightness') + ': brightness'.length + 1)) * 100,
          );

          if (brightness >= 0 && brightness <= 100) {
            return brightness;
          }
        }
      }
      throw new Error('cannot read the brightness for laptop display from brightness');
    } else {
      // for external display
      const whichDisplay = await _findWhichExternalDisplayById(targetMonitorId);
      if (whichDisplay === undefined) {
        throw new Error(`Display not found`);
      }

      const shellToRun = `${await _getDdcctlBinaryForIntel()} -d ${whichDisplay} -b \\?`;
      console.debug('getMonitorBrightness', targetMonitorId, shellToRun);

      const stdout = await executeBash(shellToRun);
      for (const line of stdout.split('\n')) {
        if (line.includes('No data after') && line.includes('tries')) {
          break;
        } else if (line.includes(`VCP control`) && line.includes('current')) {
          const brightness = parseInt(line.substr(line.indexOf('current: ') + 'current: '.length));

          if (brightness >= 0 && brightness <= 100) {
            return brightness;
          }
        }
      }

      throw new Error('cannot read the brightness for external display from ddcci');
    }
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
      } catch (err) {
        // skip monitors that can't generate shell scripts
      }
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
