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
  toggleDarkMode: async (isDarkModeOn) => {
    let shellToRun =
      `Set-ItemProperty -Path HKCU:/SOFTWARE/Microsoft/Windows/CurrentVersion/Themes/Personalize -Name AppsUseLightTheme -Value `.replace(
        /\//g,
        '\\',
      );
    shellToRun += isDarkModeOn ? '1' : '0';
    shellToRun = `powershell.exe -Command "${shellToRun}"`;

    var spawn = require('child_process').spawn;
    var child = spawn('powershell.exe', ['-Command', shellToRun]);
    return shellToRun;
  },
};

export default DisplayUtils;
