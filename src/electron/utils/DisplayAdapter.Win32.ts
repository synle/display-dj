import * as ddcci from '@hensm/ddcci';
import { executePowershell } from 'src/electron/utils/ShellUtils';
import { DISPLAY_TYPE } from 'src/constants';
import { DisplayAdapter } from 'src/types.d';

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

function _getBrightnessDccCi(idToUse: string): Promise<number> {
  return new Promise(async (resolve, reject) => {
    let retry = 3;
    let error;

    while (--retry > 0) {
      try {
        const res = await ddcci.getBrightness(idToUse);
        resolve(res);
      } catch (err) {
        error = err;
      }
    }

    reject('Failed to get brightness: ' + error);
  });
}

async function _setBrightnessDccCi(idToUse: string, newBrightness: number): Promise<void> {
  await ddcci.setBrightness(idToUse, newBrightness);
}

const DisplayUtils: DisplayAdapter = {
  getMonitorList: async () => {
    return ddcci.getMonitorList();
  },
  getMonitorType: async (idToUse: string) => {
    try {
      await _getBrightnessDccCi(idToUse);
    } catch (err) {
      try {
        await _getBrightnessBuiltin();
        return DISPLAY_TYPE.LAPTOP;
      } catch (err) {}
    }
    return DISPLAY_TYPE.EXTERNAL;
  },
  getMonitorBrightness: async (idToUse: string) => {
    try {
      return _getBrightnessDccCi(idToUse);
    } catch (err) {
      try {
        return await _getBrightnessBuiltin();
      } catch (err) {}
    }
    return 50;
  },
  updateMonitorBrightness: async (monitorId: string, newBrightness: number) => {
    // monitor is an external (DCC/CI)
    try {
      await _setBrightnessDccCi(monitorId, newBrightness);
    } catch (err) {
      // monitor is a laptop
      try {
        await _setBrightnessBuiltin(newBrightness);
      } catch (err) {}
    }
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

export default DisplayUtils;
