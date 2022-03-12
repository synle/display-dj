//@ts-nocheck
const ddcci = require('@hensm/ddcci');
import StorageUtils from './StorageUtils';
import { Monitor, MonitorUpdateInput } from '../index.d';

const MONITOR_CONFIG_FILE_DIR = 'monitor-configs';

const DisplayUtils = {
  getMonitors: async () => {
    const monitors: Monitor[] = [];

    const monitorIds = ddcci.getMonitorList();

    const monitorsFromStorage = StorageUtils.readJSON(MONITOR_CONFIG_FILE_DIR);

    for (const id of monitorIds) {
      monitors.push({
        id,
        // TODO: get the name of the monitor
        name: monitorsFromStorage[id].name || id,
        brightness: await ddcci.getBrightness(id),
      });
    }

    // persist to storage
    StorageUtils.writeJSON(MONITOR_CONFIG_FILE_DIR, monitors)

    return Promise.resolve(monitors);
  },
  updateMonitor: async (monitor: MonitorUpdateInput) => {
    const monitorsFromStorage = StorageUtils.readJSON(MONITOR_CONFIG_FILE_DIR);

    if (!monitor.id) {
      throw `id is required.`;
    }

    if (monitor.name) {
      // TODO: update the name of the id
    }

    if (monitor.brightness) {
      await ddcci.setBrightness(monitor.id, monitor.brightness);
    }

    // TODO: do the call that saves the values
  },
  toggleDarkMode: async (isDarkModeOn: boolean) => {
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
