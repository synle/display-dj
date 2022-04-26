import path from 'path';
import { executePowershell } from 'src/main/utils/ShellUtils';
import { IDisplayAdapter, Monitor } from 'src/types.d';
import cp from 'child_process';
// source: https://github.com/hensm/node-ddcci
const _getDdcciScript = async () => path.join(process['resourcesPath'], `win32_ddcci.js`);

let LAPTOP_DISPLAY_MONITOR_ID = '';
let EXTERNAL_DISPLAY_MONITOR_IDS = new Set<string>()

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
  return _sendMessageToBackgroundScript('getBrightness', targetMonitorId);
}

async function _getMonitorList(){
  return _sendMessageToBackgroundScript('getMonitorList');
}

async function _setBrightnessDccCi(targetMonitorId: string, newBrightness: number): Promise<void> {
  return _sendMessageToBackgroundScript('setBrightness', targetMonitorId, newBrightness);
}

async function _sendMessageToBackgroundScript(command : 'getBrightness'| 'setBrightness' | 'getMonitorList', ...extra: any) : Promise<any>{
  return new Promise(async (resolve, reject) => {
    const childProcess = cp.fork(await _getDdcciScript());

    childProcess.on('message', function(response: any) {
      console.debug(`ddcci child process returned`, command, response);
      const {success, data} = response;
      success === true ? resolve(data) : reject();
    });

    childProcess.send([process['resourcesPath'], command, ...extra]);
  })
}

const DisplayAdapter: IDisplayAdapter = {
  getMonitorList: async () => {
    return _getMonitorList();
  },
  getMonitorType: async (targetMonitorId: string) => {
    if(LAPTOP_DISPLAY_MONITOR_ID === targetMonitorId){
      return 'laptop_monitor';
    }

    if(EXTERNAL_DISPLAY_MONITOR_IDS.has(targetMonitorId)){
      return 'external_monitor';
    }

    // try parsing as an external display
    try {
      const brightness = await _getBrightnessDccCi(targetMonitorId);

      if (brightness >= 0 && brightness <= 100) {
        EXTERNAL_DISPLAY_MONITOR_IDS.add(targetMonitorId);
        return 'external_monitor';
      }

      throw 'invalid brightness number from ddcci';
    } catch (err) {}

    // try parsing as a built in laptop
    try {
      const brightness = await _getBrightnessBuiltin();

      if (brightness >= 0 && brightness <= 100) {
        LAPTOP_DISPLAY_MONITOR_ID = targetMonitorId;
        return 'laptop_monitor';
      }
    } catch (err) {}

    return 'unknown_monitor';
  },
  getMonitorBrightness: async (targetMonitorId: string) => {
    try {
      if(LAPTOP_DISPLAY_MONITOR_ID === targetMonitorId){
         return _getBrightnessBuiltin();
      }
      return _getBrightnessDccCi(targetMonitorId);
    } catch (err) {}
    return 50;
  },
  updateMonitorBrightness: async (targetMonitorId: string, newBrightness: number) => {
    // monitor is an external (DCC/CI)
    try {
      await _setBrightnessDccCi(targetMonitorId, newBrightness);
    } catch (err) {
      // monitor is a laptop
      console.trace(`Update brightness failed with ddcci, trying builtin method`, targetMonitorId, newBrightness);
      try {
        await _setBrightnessBuiltin(newBrightness);
      } catch (err) {
        console.error(`Update brightness failed`, targetMonitorId, newBrightness);
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