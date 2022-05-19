import DisplayUtils from 'src/main/utils/DisplayUtils';
import PositionUtils from 'src/main/utils/PositionUtils';
import PreferenceUtils from 'src/main/utils/PreferenceUtils';
import SoundUtils from 'src/main/utils/SoundUtils';

const electronEndpointHandlers = [];

function addDataEndpoint(method, url, incomingHandler) {
  const handlerToUse = async (req, res, cache) => {
    try {
      res.header('api-call-session-id', req.headers['api-call-session-id']);
      await incomingHandler(req, res, cache);
    } catch (err) {
      res
        .status(500)
        .json({ error: `Failed to addDataEndpoint: ` + JSON.stringify(err), stack: err.stack });
    }
  };

  electronEndpointHandlers.push([method, url, handlerToUse]);
}

export function getEndpointHandlers() {
  return electronEndpointHandlers;
}

export function setUpDataEndpoints() {
  addDataEndpoint('get', '/api/preferences', async (req, res) => {
    try {
      const preferences = await PreferenceUtils.get();
      res.status(200).json(preferences);
    } catch (err) {
      res
        .status(500)
        .json({ error: `Failed to get preferences: ` + JSON.stringify(err), stack: err.stack });
    }
  });

  addDataEndpoint('put', '/api/preferences', async (req, res) => {
    try {
      res.status(200).json(await PreferenceUtils.patch(req.body));
    } catch (err) {
      res.status(500).json({
        error: `Failed to update preference: ` + JSON.stringify(err),
        stack: err.stack,
      });
    }
  });

  addDataEndpoint('get', '/api/configs', async (req, res) => {
    try {
      let volume = {
        isDisabled: true
      };

      try{
        volume = {
          ...(await SoundUtils.getVolume()),
          isDisabled: false,
        }
      } catch(err1){}

      res.status(200).json({
        darkMode: (await DisplayUtils.getDarkMode()) === true,
        monitors: await DisplayUtils.getMonitors(),
        volume,
        env: process.env.APP_MODE,
        version: process.env.APP_VERSION,
        platform: process.platform,
      });
    } catch (err) {
      res
        .status(500)
        .json({ error: `Failed to get configs: ` + JSON.stringify(err), stack: err.stack });
    }
  });

  addDataEndpoint('put', '/api/configs/appPosition', async (req, res) => {
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
  addDataEndpoint('put', '/api/configs/monitors/:monitorId', async (req, res) => {
    try {
      const monitor = {
        id: req.body.id,
        name: (req.body.name || '').trim(),
        brightness: parseInt(req.body.brightness),
      };

      if (isNaN(monitor.brightness)) {
        delete monitor.brightness;
      }

      console.trace(`updateMonitor`, monitor);

      res.status(200).json(await DisplayUtils.updateMonitor(monitor));
    } catch (err) {
      res.status(500).json({
        error: `Failed to update monitor config: ` + JSON.stringify(err),
        stack: err.stack,
      });
    }
  });

  // update all displays (only brightness)
  addDataEndpoint('put', '/api/configs/monitors', async (req, res) => {
    try {
      const newBrightness = parseInt(req.body.brightness) || 0;

      console.trace(`batchUpdateBrightness`, newBrightness);

      res.status(200).json(await DisplayUtils.batchUpdateBrightness(newBrightness));
    } catch (err) {
      res.status(500).json({
        error: `Failed to update monitor config: ` + JSON.stringify(err),
        stack: err.stack,
      });
    }
  });

  addDataEndpoint('put', '/api/configs/darkMode', async (req, res) => {
    try {
      const isDarkModeOn = req.body.darkMode === true;

      let preferredBrightness;
      const preferences = await PreferenceUtils.get();
      for(const brightnessPreset of preferences.brightnessPresets){
        if(isDarkModeOn){
          if(brightnessPreset.syncedWithMode === 'dark'){
            preferredBrightness = brightnessPreset.level;
            break;
          }
        } else {
          if(brightnessPreset.syncedWithMode === 'light'){
            preferredBrightness = brightnessPreset.level;
            break;
          }
        }
      }

      console.trace(`Update darkMode`, isDarkModeOn, preferredBrightness);

      const promisesUpdates = [
        DisplayUtils.updateDarkMode(isDarkModeOn),
        preferredBrightness ? DisplayUtils.batchUpdateBrightness(preferredBrightness) : Promise.resolve(),
      ];

      res.status(200).json(await Promise.all(promisesUpdates));
    } catch (err) {
      res.status(500).json({
        error: `Failed to update darkMode config: ` + JSON.stringify(err),
        stack: err.stack,
      });
    }
  });
  addDataEndpoint('put', '/api/configs/volume', async (req, res) => {
    try {
      let muted = req.body.muted;
      let volume = parseInt(req.body.value);

      console.trace(`Update volume`, volume, muted);

      if(muted !== undefined && !isNaN(volume)){
        const promises = [];
        promises.push(SoundUtils.setMuted(muted === true));
        promises.push(SoundUtils.setVolume(volume));

        await Promise.all(promises);
        res.status(204).send();
      } else {
        res.status(400).send('This API requires volume or isMuted in the body');
      }
    } catch (err) {
      res.status(500).json({
        error: `Failed to update darkmode config: ` + JSON.stringify(err),
        stack: err.stack,
      });
    }
  });
}
