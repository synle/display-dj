import DisplayUtils from './DisplayUtils';

const electronEndpointHandlers = [];

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
  // query endpoints
  addDataEndpoint('get', '/api/monitors', async (req, res) => {
    const monitors = await DisplayUtils.getMonitors();
    res.status(200).json(monitors);
  });

  addDataEndpoint('put', '/api/monitors', async (req, res) => {
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

  addDataEndpoint('put', '/api/darkMode', async (req, res) => {
    try {
      const isDarkModeOn = req.body.darkMode === true;

      res.status(200).json(await DisplayUtils.toggleDarkMode(isDarkModeOn));
    } catch (err) {
      res.status(500).json({ error: `Failed to update darkMode`, stack: err.stack });
    }
  });
}
