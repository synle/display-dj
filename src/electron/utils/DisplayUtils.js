import * as ddcci from '@hensm/ddcci';
import StorageUtils from './StorageUtils';
import { spawn } from 'child_process';

const brightness = require('brightness');

const MONITOR_CONFIG_FILE_DIR = 'monitor-configs.json';

const LAPTOP_BUILT_IN_DISPLAY_ID = 'laptop-built-in';

function _getMonitorConfigs() {
  try {
    return StorageUtils.readJSON(MONITOR_CONFIG_FILE_DIR);
  } catch (err) {
    return {};
  }
}

function _setMonitorConfigs(monitors) {
  // wrap it inside key => monitor
  const res = {};
  for (const monitor of monitors) {
    res[monitor.id] = monitor;
  }
  StorageUtils.writeJSON(MONITOR_CONFIG_FILE_DIR, res);
}

function _executePowershell(shellToRun) {
  const child = spawn('powershell.exe', ['-Command', shellToRun]);

  return new Promise((resolve, reject) => {
    // TODO: add a handler for error case
    let data = '';
    child.stdout.on('data', function (msg) {
      data += msg.toString();
    });

    child.on('exit', function (exitCode) {
      if (parseInt(exitCode) !== 0) {
        //Handle non-zero exit
        reject(exitCode);
      } else {
        resolve(data);
      }
    });
  });
}

const DisplayUtils = {
  getMonitors: async () => {
    const monitors = [];

    const monitorsFromStorage = _getMonitorConfigs();

    // getting the laptop monitor if there is any
    // TODO: find a more reliable library to built in display
    // let brightness;
    // let name = 'Laptop Built-In Display';
    // try {
    //   name = monitorsFromStorage[id].name;
    // } catch (err) {}

    // try {
    //   brightness = await Math.floor((await brightness.get()) * 100);
    // } catch (err) {
    //   console.log('>> Failed to get the built-in monitor configs', err);
    // }

    monitors.push({
      id: LAPTOP_BUILT_IN_DISPLAY_ID,
      name,
      brightness,
    });

    // getting the external monitors
    let monitorCount = 0;
    const monitorIds = ddcci.getMonitorList();
    for (const id of monitorIds) {
      try {
        let name = `Monitor #${++monitorCount}`;
        try {
          name = monitorsFromStorage[id].name;
        } catch (err) {}

        monitors.push({
          id,
          name,
          brightness: await ddcci.getBrightness(id),
        });
      } catch (err) {
        console.log('>> Failed to get the external monitor configs', id, err);
      }
    }

    // persist to storage
    _setMonitorConfigs(monitors);

    return Promise.resolve(monitors);
  },
  updateMonitor: async (monitor) => {
    const monitorsFromStorage = _getMonitorConfigs();

    if (!monitor.id) {
      throw `id is required.`;
    }

    if (!monitorsFromStorage[monitor.id]) {
      throw `id not found.`;
    }

    monitor = {
      ...monitorsFromStorage[monitor.id],
      ...monitor,
    };

    monitorsFromStorage[monitor.id] = monitor;

    await DisplayUtils.updateBrightness(monitor);

    // persist to storage
    _setMonitorConfigs(Object.values(monitorsFromStorage));
  },
  getAllMonitorsBrightness: async () => {
    try {
      const monitors = await DisplayUtils.getMonitors();

      // get the min brightness of them all
      return (
        Math.min.apply(null, [...monitors.map((monitor) => parseInt(monitor.brightness))]) || 0
      );
    } catch (err) {
      return 0;
    }
  },
  updateBrightness: async (monitor) => {
    if (monitor.id === LAPTOP_BUILT_IN_DISPLAY_ID) {
      // monitor is a laptop
      // TODO: find a more reliable library to built in display
      // try {
      //   await brightness.set(Math.floor(monitor.brightness / 100));
      //   console.log('>> update laptop display brightness succeeds', monitor.brightness);
      // } catch (err) {
      //   console.log('>> update laptop display brightness failed', monitor.brightness, err);
      // }
    } else {
      // monitor is an external (DCC/CI)
      console.log('>> update external display brightness', monitor.name, monitor.brightness);
      await ddcci.setBrightness(monitor.id, monitor.brightness);
    }
  },
  updateAllBrightness: async (newBrightness, delta) => {
    newBrightness += delta;

    // making sure the range is 0 to 100
    if (newBrightness < 0 || isNaN(newBrightness)) {
      newBrightness = 0;
    } else if (newBrightness > 100) {
      newBrightness = 100;
    }

    const monitors = await DisplayUtils.getMonitors();
    const promisesChangeBrightness = [];
    for (const monitor of monitors) {
      monitor.brightness = newBrightness;
      promisesChangeBrightness.push(DisplayUtils.updateBrightness(monitor));
    }

    // persist to storage
    _setMonitorConfigs(monitors);

    await Promise.all(promisesChangeBrightness);

    return newBrightness;
  },
  getDarkMode: async () => {
    let shellToRun =
      `Get-ItemProperty -Path HKCU:/SOFTWARE/Microsoft/Windows/CurrentVersion/Themes/Personalize -Name AppsUseLightTheme`.replace(
        /\//g,
        '\\',
      );

    return new Promise(async (resolve) => {
      const msg = await _executePowershell(shellToRun);
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
  toggleDarkMode: async (isDarkModeOn) => {
    const baseShellToRun = `Set-ItemProperty -Path HKCU:/SOFTWARE/Microsoft/Windows/CurrentVersion/Themes/Personalize -Name `;

    let shellToRun;

    const darkModeValue = isDarkModeOn ? '1' : '0';

    // change the app theme
    shellToRun =
      `${baseShellToRun} SystemUsesLightTheme -Value ${darkModeValue}; ${baseShellToRun} AppsUseLightTheme -Value ${darkModeValue}`.replace(
        /\//g,
        '\\',
      );
    await _executePowershell(shellToRun);
  },
};

export default DisplayUtils;
