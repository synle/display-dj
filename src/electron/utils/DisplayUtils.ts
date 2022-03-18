import { executePowershell } from 'src/electron/utils/ShellUtils';
import StorageUtils, { MONITOR_CONFIG_FILE_DIR } from 'src/electron/utils/StorageUtils';
import DisplayAdapterWin32 from 'src/electron/utils/DisplayAdapter.Win32';
import DisplayAdapterDarwin from 'src/electron/utils/DisplayAdapter.Darwin';
import { Monitor, MonitorUpdateInput } from 'src/types.d';

function _getMonitorConfigs(): Record<string, Monitor> {
  try {
    return StorageUtils.readJSON(MONITOR_CONFIG_FILE_DIR);
  } catch (err) {
    console.error('>> Failed to get monitor configs from JSON', err);
    return {};
  }
}

function _setMonitorConfigs(monitors: Monitor[]) {
  // wrap it inside key => monitor
  const res: Record<string, Monitor> = {};
  for (const monitor of monitors) {
    res[monitor.id] = monitor;
  }
  StorageUtils.writeJSON(MONITOR_CONFIG_FILE_DIR, res);
}

let DisplayAdapterToUse;
switch(process.platform){
  case 'win32':
    DisplayAdapterToUse = DisplayAdapterWin32;
    break;
  case 'darwin':
    DisplayAdapterToUse = DisplayAdapterDarwin;
    break;
  default:
    console.error(`Application does not supported for this OS - ${process.platform}`);
    process.exit(1);
    break;
}

const DisplayUtils = {
  getMonitors: async () => {
    let monitors: Monitor[] = [];

    const monitorsFromStorage = _getMonitorConfigs();

    let sortOrderToUse: number = 0;
    let nameToUse: string = '';
    let disabledToUse: boolean = false;
    let idToUse: string = '';

    // getting the external monitors
    let monitorCount = 0;
    const monitorIds = await DisplayAdapterToUse.getMonitorList();
    for (let idx = 0; idx < monitorIds.length; idx++) {
      const idToUse = monitorIds[idx];

      try {
        nameToUse = monitorsFromStorage[idToUse].name;
      } catch (err) {
        nameToUse = `Monitor #${++monitorCount}`;
      }

      try {
        sortOrderToUse = monitorsFromStorage[idToUse].sortOrder;
      } catch (err) {}
      sortOrderToUse = sortOrderToUse || idx;

      try {
        disabledToUse = !!monitorsFromStorage[idToUse].disabled;
      } catch (err) {}
      disabledToUse = disabledToUse || false;

      monitors.push({
        id: idToUse,
        name: nameToUse,
        type: await DisplayAdapterToUse.getMonitorType(idToUse),
        brightness: await DisplayAdapterToUse.getMonitorBrightness(idToUse) ,
        sortOrder: sortOrderToUse,
        disabled: disabledToUse,
      });
    }

    // handling the sorting based on sortOrder
    monitors = monitors.sort((a, b) => {
      const ca = `${(a.sortOrder || 0).toString().padStart(3, '0')}`;
      const cb = `${(b.sortOrder || 0).toString().padStart(3, '0')}`;

      return ca.localeCompare(cb);
    });

    // persist to storage
    _setMonitorConfigs(monitors);

    return Promise.resolve(monitors);
  },
  updateMonitor: async (monitor: MonitorUpdateInput) => {
    const monitorsFromStorage = _getMonitorConfigs();

    if (!monitor.id) {
      throw `id is required.`;
    }

    if (!monitorsFromStorage[monitor.id]) {
      throw `id not found.`;
    }

    monitorsFromStorage[monitor.id] = {
      ...monitorsFromStorage[monitor.id],
      ...monitor,
    };

    await DisplayAdapterToUse.updateMonitorBrightness(
      monitorsFromStorage[monitor.id].id,
      monitorsFromStorage[monitor.id].brightness,
    );

    // persist to storage
    _setMonitorConfigs(Object.values(monitorsFromStorage));
  },
  getAllMonitorsBrightness: async (): Promise<number> => {
    try {
      const monitors = await DisplayUtils.getMonitors();

      // get the min brightness of them all
      return Math.min.apply(null, [...monitors.map((monitor) => monitor.brightness || 0)]) || 0;
    } catch (err) {
      return 0;
    }
  },
  updateAllBrightness: async (newBrightness: number, delta: number = 0) => {
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
      promisesChangeBrightness.push(DisplayAdapterToUse.updateMonitorBrightness(monitor.id, monitor.brightness));
    }

    // persist to storage
    _setMonitorConfigs(monitors);

    await Promise.all(promisesChangeBrightness);

    return newBrightness;
  },
  getDarkMode: DisplayAdapterToUse.getDarkMode,
  updateDarkMode:  DisplayAdapterToUse.updateDarkMode
};

export default DisplayUtils;
