import DisplayUtils from './DisplayUtils';

const electronEndpointHandlers: any[] = [];

function addDataEndpoint(
  method: 'get' | 'post' | 'put' | 'delete',
  url: string,
  incomingHandler: (req: any, res: any, cache: any) => void,
) {
  const handlerToUse = async (req: any, res: any, cache: any) => {
    try {
      res.header('api-call-session-id', req.headers['api-call-session-id']);
      await incomingHandler(req, res, cache);
    } catch (err: any) {
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

  addDataEndpoint('put', '/api/monitors/:monitorId', async (req, res) => {
    try {
      res.status(200).json(
        await DisplayUtils.updateMonitor({
          id: req.params.monitorId,
          name: req.body?.name,
          brightness: req.body?.brightness,
        }),
      );
    } catch (err) {
      res.status(500).json({ error: `Failed to save monitor`, stack: err });
    }
  });

  addDataEndpoint('put', '/api/darkMode', async (req, res) => {
    try {
      const isDarkModeOn = req.body?.darkMode === true;

      res.status(200).json(await DisplayUtils.toggleDarkMode(isDarkModeOn));
    } catch (err) {
      res.status(500).json({ error: `Failed to update darkMode`, stack: err });
    }
  });
}
