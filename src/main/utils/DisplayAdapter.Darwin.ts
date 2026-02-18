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

// Maps m1ddc monitor UUID → m1ddc display number (e.g. 1, 2, 3)
const _m1ddcDisplayNumberMap: Map<string, number> = new Map();

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
  if (await _isM1Mac()) {
    // For M1, use the display number directly from m1ddc's display list output
    const displayNumber = _m1ddcDisplayNumberMap.get(targetMonitorId);
    return displayNumber;
  }

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

async function _getMonitorListM1(): Promise<string[]> {
  const shellToRun = `${await _getDdcctlBinaryForM1()} display list`;
  const stdout = await executeBash(shellToRun);
  const monitors: string[] = [];

  _m1ddcDisplayNumberMap.clear();

  // Parse lines like: [1] XZ322QU V3 (48D3AB11-720E-4436-BB8F-DD559D1EFC90)
  for (const line of (stdout || '').split('\n')) {
    const match = line.match(/^\[(\d+)\]\s+.*\(([A-F0-9-]+)\)$/);
    if (match) {
      const displayNumber = parseInt(match[1]);
      const uuid = match[2];
      _m1ddcDisplayNumberMap.set(uuid, displayNumber);
      monitors.push(uuid);
    }
  }

  return monitors;
}

async function _getMonitorList(): Promise<string[]> {
  if (getCache().allMonitorList) {
    return getCache().allMonitorList;
  }

  let monitors: string[];

  if (await _isM1Mac()) {
    monitors = await _getMonitorListM1();
  } else {
    const cmd = await _getDdcctlBinaryForIntel();
    monitors = await new Promise((resolve, reject) => {
      try {
        exec(cmd, (_error, stdout) => {
          const mons = (stdout || '')
            .split('\n')
            .filter((line) => line.indexOf('D:') === 0)
            .map((line) => line.replace('D:', '').trim());
          mons.unshift(LAPTOP_DISPLAY_MONITOR_ID);
          resolve(mons);
        });
      } catch (err) {
        reject(err);
      }
    });
  }

  getCache().allMonitorList = monitors;
  return monitors;
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
    if (await _isM1Mac()) {
      // M1 Mac: use m1ddc for external displays (no built-in display support)
      const whichDisplay = await _findWhichExternalDisplayById(targetMonitorId);
      if (whichDisplay === undefined) {
        throw new Error(`Display not found`);
      }

      const shellToRun = `${await _getDdcctlBinaryForM1()} display ${whichDisplay} get luminance`;
      console.debug('getMonitorBrightness', targetMonitorId, shellToRun);

      const stdout = await executeBash(shellToRun);
      const brightness = parseInt((stdout || '').trim());
      if (brightness >= 0 && brightness <= 100) {
        return brightness;
      }
      throw new Error('cannot read the brightness for external display from m1ddc');
    }

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
