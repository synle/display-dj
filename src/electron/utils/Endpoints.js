import { screen } from 'electron';
import DisplayUtils from './DisplayUtils';

const electronEndpointHandlers = [];

// TODO: read this from the package.json
const version = '1.0.1';

function addDataEndpoint(method, url, incomingHandler) {
  const handlerToUse = async (req, res, cache) => {
    try {
      res.header('api-call-session-id', req.headers['api-call-session-id']);
      await incomingHandler(req, res, cache);
    } catch (err) {
      console.log('err', err);
      res.status(500).send(err);
    }
  };

  electronEndpointHandlers.push([method, url, handlerToUse]);
}

export function getEndpointHandlers() {
  return electronEndpointHandlers;
}

export function setUpDataEndpoints() {
  addDataEndpoint('get', '/api/configs', async (req, res) => {
    try {
      res.status(200).json({
        darkMode: (await DisplayUtils.getDarkMode()) === true,
        monitors: await DisplayUtils.getMonitors(),
        env: process.env.APPLICATION_MODE,
        version,
      });
    } catch (err) {
      res.status(500).json({ error: `Failed to get configs`, stack: err.stack });
    }
  });

  addDataEndpoint('put', '/api/configs/positionWindow', async (req, res) => {
    try {
      const width = 300;
      const height = req.body.height;

      const trayBound = tray.getBounds();

      var mainScreen = screen.getPrimaryDisplay();
      const mainScreenSize = mainScreen.size;
      let x = trayBound.x, y = trayBound.y;

      if (x > mainScreenSize.width / 2) {
        // right
        x = Math.floor(trayBound.x - width + 50);
      } else {
        // left
        x = Math.floor(trayBound.x + 50);
      }

      if (y > mainScreenSize.height / 2) {
        // bottom
        y = Math.floor(trayBound.y - height - 20);
      } else {
        // top
        y = Math.floor(trayBound.y - height + 20);
      }
      mainWindow.setPosition(x, y);

      res.status(200).json({ height });
    } catch (err) {
      res.status(500).json({ error: `Failed to save monitor`, stack: err.stack });
    }
  });

  addDataEndpoint('put', '/api/configs/monitors', async (req, res) => {
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
      res.status(500).json({ error: `Failed to save monitor`, stack: err.stack });
    }
  });

  addDataEndpoint('put', '/api/configs/darkMode', async (req, res) => {
    try {
      const isDarkModeOn = req.body.darkMode === true;

      res.status(200).json(await DisplayUtils.toggleDarkMode(isDarkModeOn));
    } catch (err) {
      res.status(500).json({ error: `Failed to update darkMode`, stack: err.stack });
    }
  });
}
