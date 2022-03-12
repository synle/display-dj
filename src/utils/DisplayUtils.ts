//@ts-nocheck
const ddcci = require('@hensm/ddcci');
import { Monitor, MonitorUpdateInput } from '../index.d';

const DisplayUtils = {
  getMonitors: async () => {
    const monitors: Monitor[] = [];

    const monitorIds = ddcci.getMonitorList();

    for (const id of monitorIds) {
      monitors.push({
        id,
        // TODO: get the name of the monitor
        name: id,
        brightness: await ddcci.getBrightness(id),
      });
    }

    return Promise.resolve(monitors);
  },
  updateMonitor: async (monitor: MonitorUpdateInput) => {
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
