import * as ddcci from '@hensm/ddcci';
import StorageUtils from './StorageUtils';

const MONITOR_CONFIG_FILE_DIR = 'monitor-configs.json';

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
  const spawn = require('child_process').spawn;
  const child = spawn('powershell.exe', ['-Command', shellToRun]);

  return new Promise((resolve) => {
    // TODO: add a handler for error case
    let data;
    child.stdout.on('data', function (msg) {
      data = msg.toString();
    });

    child.stdout.on('end', function () {
      resolve(data);
    });
  });
}

const DisplayUtils = {
  getMonitors: async () => {
    const monitors = [];

    const monitorIds = ddcci.getMonitorList();

    const monitorsFromStorage = _getMonitorConfigs();

    let monitorCount = 0;
    for (const id of monitorIds) {
      let name = `Monitor #${++monitorCount}`;

      try {
        name = monitorsFromStorage[id].name;
      } catch (err) {}

      monitors.push({
        id,
        name,
        brightness: await ddcci.getBrightness(id),
      });
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

    await ddcci.setBrightness(monitor.id, monitor.brightness);

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
  adjustAllBrightness: async (newBrightness, delta) => {
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
      promisesChangeBrightness.push(ddcci.setBrightness(monitor.id, monitor.brightness));
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
    shellToRun = `powershell.exe -Command "${shellToRun}"`;

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
    const promisesExec = [];

    let shellToRun;

    // change the app theme
    shellToRun = `${baseShellToRun} AppsUseLightTheme -Value ${isDarkModeOn ? '1' : '0'}`.replace(
      /\//g,
      '\\',
    );
    shellToRun = `powershell.exe -Command "${shellToRun}"`;
    promisesExec.push(_executePowershell(shellToRun));

    // change the system theme
    shellToRun = `${baseShellToRun} SystemUsesLightTheme -Value ${
      isDarkModeOn ? '1' : '0'
    }`.replace(/\//g, '\\');
    shellToRun = `powershell.exe -Command "${shellToRun}"`;
    promisesExec.push(_executePowershell(shellToRun));
    await Promise.all(promisesExec);
  },
};

export default DisplayUtils;
