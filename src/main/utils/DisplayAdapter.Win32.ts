import path from 'path';
import cp from 'child_process';
import * as ddcci from '@hensm/ddcci';
import { executePowershell } from 'src/main/utils/ShellUtils';
import { IDisplayAdapter, Monitor } from 'src/types.d';
// source: https://github.com/hensm/node-ddcci

/**
 * get current laptop brightness. more info here
 * https://docs.microsoft.com/en-us/windows/win32/wmicoreprov/wmimonitorbrightness
 */
function _getBrightnessBuiltin(): Promise<number> {
  return new Promise(async (resolve, reject) => {
    let shellToRun = `(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightness).CurrentBrightness`;
    const brightness = parseInt(await executePowershell(shellToRun));
    resolve(brightness);
  });
}

/**
 * set current laptop brightness. more info here
 * https://docs.microsoft.com/en-us/windows/win32/wmicoreprov/wmisetbrightness-method-in-class-wmimonitorbrightnessmethods
 */
async function _setBrightnessBuiltin(newBrightness: number): Promise<void> {
  let shellToRun = `(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightnessMethods).WmiSetBrightness(1,${newBrightness})`;
  await executePowershell(shellToRun);
}

function _getBrightnessDccCi(targetMonitorId: string): Promise<number> {
  return new Promise(async (resolve, reject) => {
    let retry = 3;
    let error;

    while (--retry > 0) {
      try {
        const res = await ddcci.getBrightness(targetMonitorId);
        resolve(res);
      } catch (err) {
        error = err;
      }
    }

    reject('Failed to get brightness: ' + error);
  });
}

async function _setBrightnessDccCi(targetMonitorId: string, newBrightness: number): Promise<void> {
  // await ddcci.setBrightness(targetMonitorId, newBrightness);
  const scriptPath = path.join(process['resourcesPath'], `win32_ddcci.js`)

  const childProcess = cp.fork(scriptPath);

  childProcess.on('message', function(m) {
    console.log('PARENT got message:', m);
  });

  childProcess.send([targetMonitorId, newBrightness]);
}

const DisplayAdapter: IDisplayAdapter = {
  getMonitorList: async () => {
    return ddcci.getMonitorList();
  },
  getMonitorType: async (targetMonitorId: string) => {
    try {
      const brightness = await _getBrightnessDccCi(targetMonitorId);

      if (brightness >= 0 && brightness <= 100) {
        return 'external_monitor';
      }

      throw 'invalid brightness number from ddcci';
    } catch (err) {
      try {
        const brightness = await _getBrightnessBuiltin();

        if (brightness >= 0 && brightness <= 100) {
          return 'laptop_monitor';
        }
      } catch (err) {}
    }

    return 'unknown_monitor';
  },
  getMonitorBrightness: async (targetMonitorId: string) => {
    try {
      return _getBrightnessDccCi(targetMonitorId);
    } catch (err) {
      try {
        return await _getBrightnessBuiltin();
      } catch (err) {}
    }
    return 50;
  },
  updateMonitorBrightness: async (targetMonitorId: string, newBrightness: number) => {
    // monitor is an external (DCC/CI)
    try {
      await _setBrightnessDccCi(targetMonitorId, newBrightness);
      console.trace(`Update brightness with ddcci successfully`, targetMonitorId, newBrightness);
    } catch (err) {
      // monitor is a laptop
      console.trace(`Update brightness failed with ddcci, trying builtin method`, targetMonitorId, newBrightness);
      try {
        await _setBrightnessBuiltin(newBrightness);
        console.trace(`Update brightness with ddcci successfully`, targetMonitorId, newBrightness);
      } catch (err) {
        console.trace(`Update brightness failed`, targetMonitorId, newBrightness);
      }
    }
  },
  batchUpdateMonitorBrightness: async (monitors: Monitor[]) => {
    const promisesChangeBrightness = [];

    for (const monitor of monitors) {
      promisesChangeBrightness.push(
        new Promise(async (resolve) => {
          try {
            await DisplayAdapter.updateMonitorBrightness(monitor.id, monitor.brightness);
            console.trace('Update monitor brightness', monitor.name, monitor.brightness);
          } catch (err) {
            console.error('Failed to update monitor brightness', monitor.name, monitor.id);
          }
          resolve(monitor.id);
        }),
      );
    }

    await Promise.all(promisesChangeBrightness);
  },
  getDarkMode: async () => {
    let shellToRun =
      `Get-ItemProperty -Path HKCU:/SOFTWARE/Microsoft/Windows/CurrentVersion/Themes/Personalize -Name AppsUseLightTheme`.replace(
        /\//g,
        '\\',
      );

    return new Promise(async (resolve) => {
      const msg = await executePowershell(shellToRun);
      const lines = msg
        .toString()
        .split('\n')
        .map((s) => s.trim());

      for (const line of lines) {
        if (line.includes('AppsUseLightTheme')) {
          return resolve(line.includes('0'));
        }
      }

      resolve(false);
    });
  },
  updateDarkMode: async (isDarkModeOn: boolean) => {
    const baseShellToRun = `Set-ItemProperty -Path HKCU:/SOFTWARE/Microsoft/Windows/CurrentVersion/Themes/Personalize -Name `;

    let shellToRun;

    // NOTE: this looks kinda flipped, but the value we set here is lightMode
    const darkModeValue = isDarkModeOn ? '0' : '1';

    // change the app theme
    shellToRun =
      `${baseShellToRun} SystemUsesLightTheme -Value ${darkModeValue}; ${baseShellToRun} AppsUseLightTheme -Value ${darkModeValue}`.replace(
        /\//g,
        '\\',
      );
    await executePowershell(shellToRun);
  },
};

export default DisplayAdapter;
