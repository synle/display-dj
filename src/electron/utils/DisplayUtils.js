const ddcci = require('@hensm/ddcci');
import StorageUtils from './StorageUtils';

const MONITOR_CONFIG_FILE_DIR = 'monitor-configs.json';

function _getMonitorConfigs() {
  return StorageUtils.readJSON(MONITOR_CONFIG_FILE_DIR);
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

    console.log(monitorsFromStorage);

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

    console.log('>>> save this', monitorsFromStorage[monitor.id]);

    // persist to storage
    _setMonitorConfigs(Object.values(monitorsFromStorage));
  },
  toggleDarkMode: async (isDarkModeOn) => {
    let shellToRun =
      `Set-ItemProperty -Path HKCU:/SOFTWARE/Microsoft/Windows/CurrentVersion/Themes/Personalize -Name AppsUseLightTheme -Value `.replace(
        /\//g,
        '\\',
      );
    shellToRun += isDarkModeOn ? '1' : '0';

    console.log(shellToRun);

    return shellToRun;
  },
};

export default DisplayUtils;
