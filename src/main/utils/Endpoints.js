import DisplayUtils from 'src/main/utils/DisplayUtils';
import PositionUtils from 'src/main/utils/PositionUtils';
import PreferenceUtils from 'src/main/utils/PreferenceUtils';

const electronEndpointHandlers = [];

function _addDataEndpoint(method, url, incomingHandler) {
  const handlerToUse = async (req, res, cache) => {
    try {
      res.header('api-call-session-id', req.headers['api-call-session-id']);
      await incomingHandler(req, res, cache);
    } catch (err) {
      res
        .status(500)
        .json({ error: `Failed to _addDataEndpoint: ` + JSON.stringify(err), stack: err.stack });
    }
  };

  electronEndpointHandlers.push([method, url, handlerToUse]);
}

function _getUpdatedOrdersForList(items, from, to) {
  // ordering will move the tab from the old index to the new index
  // and push everything from that point out
  const targetItem = items[from];
  let leftHalf;
  let rightHalf;

  if (from > to) {
    leftHalf = items.filter((q, idx) => idx < to && idx !== from);
    rightHalf = items.filter((q, idx) => idx >= to && idx !== from);
  } else {
    leftHalf = items.filter((q, idx) => idx <= to && idx !== from);
    rightHalf = items.filter((q, idx) => idx > to && idx !== from);
  }

  return [...leftHalf, targetItem, ...rightHalf];
}

export function getEndpointHandlers() {
  return electronEndpointHandlers;
}

export function setUpDataEndpoints() {
  _addDataEndpoint('get', '/api/preferences', async (req, res) => {
    try {
      const preferences = await PreferenceUtils.get();
      res.status(200).json(preferences);
    } catch (err) {
      res
        .status(500)
        .json({ error: `Failed to get preferences: ` + JSON.stringify(err), stack: err.stack });
    }
  });

  _addDataEndpoint('put', '/api/preferences', async (req, res) => {
    try {
      res.status(200).json(await PreferenceUtils.patch(req.body));
    } catch (err) {
      res.status(500).json({
        error: `Failed to update preference: ` + JSON.stringify(err),
        stack: err.stack,
      });
    }
  });

  _addDataEndpoint('get', '/api/configs', async (req, res) => {
    try {
      res.status(200).json({
        darkMode: (await DisplayUtils.getDarkMode()) === true,
        monitors: await DisplayUtils.getMonitors(),
        env: process.env.APPLICATION_MODE,
        version: process.env.APP_VERSION,
        platform: process.platform,
      });
    } catch (err) {
      res
        .status(500)
        .json({ error: `Failed to get configs: ` + JSON.stringify(err), stack: err.stack });
    }
  });

  _addDataEndpoint('put', '/api/configs/appPosition', async (req, res) => {
    try {
      await PositionUtils.updateTrayPosition(global.mainWindow, req.body.height);
      res.status(204).send();
    } catch (err) {
      res.status(500).json({
        error: `Failed to adjust tray position: ` + JSON.stringify(err),
        stack: err.stack,
      });
    }
  });

  // update a single display
  _addDataEndpoint('put', '/api/configs/monitors/:monitorId', async (req, res) => {
    try {
      const monitor = {
        id: req.body.id,
        name: (req.body.name || '').trim(),
        brightness: parseInt(req.body.brightness),
      };

      if (isNaN(monitor.brightness)) {
        delete monitor.brightness;
      }

      res.status(200).json(await DisplayUtils.updateMonitor(monitor));
    } catch (err) {
      res.status(500).json({
        error: `Failed to update monitor config: ` + JSON.stringify(err),
        stack: err.stack,
      });
    }
  });

  // update all displays (only brightness)
  _addDataEndpoint('put', '/api/configs/monitors', async (req, res) => {
    try {
      const {fromIdx, toIdx} = req.body;
      if(fromIdx >= 0 && toIdx >= 0){
        // doing sort order updates
        const monitors = await DisplayUtils.getMonitors()
        const newMonitors = _getUpdatedOrdersForList(monitors, fromIdx, toIdx);
        for(let i = 0; i < newMonitors.length; i++){
          newMonitors[i].sortOrder = i;
          console.debug('fixed', newMonitors[i]);
          await DisplayUtils.updateMonitor(newMonitors[i]);
        }

        return res.status(200).json(newMonitors);
      }

      // doing brightness update
      const newBrightness = parseInt(req.body.brightness) || 0;
      res.status(200).json(await DisplayUtils.batchUpdateBrightness(newBrightness));
    } catch (err) {
      res.status(500).json({
        error: `Failed to update monitor config: ` + JSON.stringify(err),
        stack: err.stack,
      });
    }
  });

  _addDataEndpoint('put', '/api/configs/darkMode', async (req, res) => {
    try {
      const isDarkModeOn = req.body.darkMode === true;

      res.status(200).json(await DisplayUtils.updateDarkMode(isDarkModeOn));
    } catch (err) {
      res.status(500).json({
        error: `Failed to update darkmode config: ` + JSON.stringify(err),
        stack: err.stack,
      });
    }
  });
}
