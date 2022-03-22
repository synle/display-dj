import DisplayAdapter from 'src/electron/utils/DisplayAdapter';
import StorageUtils, {
  MONITOR_CONFIG_FILE_PATH,
  PREFERENCE_FILE_PATH,
} from 'src/electron/utils/StorageUtils';
import { Monitor, SingleMonitorUpdateInput } from 'src/types.d';

function _getMonitorConfigs(): Record<string, Monitor> {
  return StorageUtils.readJSON(MONITOR_CONFIG_FILE_PATH) || {};
}

/**
 * This routine will sync the monitor configs such as name and brightness
 *
 * Here we will concat what's in storage with what's active by the api.
 * The reasons here are that sometimes you have different monitor setup.
 * One set of monitors for work and another set for home. So it's a good
 * practice to store the configs and merge them instead of using what's active
 * @param {Monitor[]} monitors active monitors returned by the API
 */
function _syncMonitorConfigs(monitors: Monitor[]) {
  // this section will merge the monitors from input with what's in the storage
  const monitorsFromStorage = _getMonitorConfigs();

  for (const monitor of monitors) {
    delete monitorsFromStorage[monitor.id];
  }

  // adding the remaining missing monitor from storage
  monitors = [...monitors, ...Object.values(monitorsFromStorage)];

  StorageUtils.writeJSON(MONITOR_CONFIG_FILE_PATH, _serializeMonitorConfigs(monitors));
}

function _serializeMonitorConfigs(monitors: Monitor[]) {
  const res: Record<string, Monitor> = {};
  for (const monitor of monitors) {
    res[monitor.id] = monitor;
  }
  return res;
}

const DisplayUtils = {
  reset: async () => {
    await Promise.all([
      StorageUtils.delete(MONITOR_CONFIG_FILE_PATH),
      StorageUtils.delete(PREFERENCE_FILE_PATH),
    ]);
  },
  getMonitorsFromStorage: async () => {
    const monitorsFromStorage = _getMonitorConfigs();
    return Object.values(monitorsFromStorage);
  },
  getMonitors: async () => {
    let monitors: Monitor[] = [];

    const monitorsFromStorage = _getMonitorConfigs();

    let shouldSync = false;

    // construct the active displays
    let monitorCount = 0;
    const monitorIds = await DisplayAdapter.getMonitorList();

    console.debug(`Monitor List: ${monitorIds.length}`, monitorIds);

    for (let idx = 0; idx < monitorIds.length; idx++) {
      const idToUse = monitorIds[idx];

      const type = await DisplayAdapter.getMonitorType(idToUse);

      let brightness: number = monitorsFromStorage?.[idToUse]?.brightness || 50; // set default brightness to 50%
      try {
        brightness = await DisplayAdapter.getMonitorBrightness(idToUse);
      } catch (err) {}
      let name = monitorsFromStorage?.[idToUse]?.name || '';
      if (!name) {
        name = type === 'laptop_monitor' ? `Laptop Display` : `External Monitor #${++monitorCount}`;
      }

      monitors.push({
        id: idToUse,
        name: name,
        type,
        brightness,
        sortOrder: monitorsFromStorage?.[idToUse]?.sortOrder || idx,
        disabled: !!monitorsFromStorage?.[idToUse]?.disabled || false,
      });
    }

    // handling the sorting based on sortOrder
    monitors = monitors.sort((a, b) => {
      const ca = `${(a.sortOrder || 0).toString().padStart(3, '0')}`;
      const cb = `${(b.sortOrder || 0).toString().padStart(3, '0')}`;

      return ca.localeCompare(cb);
    });

    // syncing if needed aka the configs are different from what's there in the database
    if (JSON.stringify(Object.values(monitorsFromStorage)) !== JSON.stringify(monitors)) {
      _syncMonitorConfigs(monitors);
    }

    return Promise.resolve(monitors);
  },
  updateMonitor: async (monitor: SingleMonitorUpdateInput) => {
    const monitorsFromStorage = _getMonitorConfigs();

    if (!monitor.id) {
      throw `ID is required.`;
    }

    if (!monitorsFromStorage[monitor.id]) {
      throw `ID=${monitor.id} not found.`;
    }

    monitorsFromStorage[monitor.id] = {
      ...monitorsFromStorage[monitor.id],
      ...monitor,
    };

    await DisplayAdapter.updateMonitorBrightness(
      monitorsFromStorage[monitor.id].id,
      monitorsFromStorage[monitor.id].brightness,
    );

    // persist to storage
    _syncMonitorConfigs(Object.values(monitorsFromStorage));
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
  batchUpdateBrightness: async (newBrightness: number, delta: number = 0) => {
    newBrightness += delta;

    // making sure the range is 0 to 100
    if (newBrightness < 0 || isNaN(newBrightness)) {
      newBrightness = 0;
    } else if (newBrightness > 100) {
      newBrightness = 100;
    }

    const monitors = (await DisplayUtils.getMonitors()).map((monitor) => ({
      ...monitor,
      brightness: newBrightness,
    }));


    console.trace(`batchUpdateBrightness`, newBrightness, delta, monitors.length + ' monitors');

    // persist to storage
    _syncMonitorConfigs(monitors);

    await DisplayAdapter.batchUpdateMonitorBrightness(monitors);

    return newBrightness;
  },
  updateMonitorBrightness: DisplayAdapter.updateMonitorBrightness,
  getDarkMode: DisplayAdapter.getDarkMode,
  updateDarkMode: DisplayAdapter.updateDarkMode,
};

export default DisplayUtils;
