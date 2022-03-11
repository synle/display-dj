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
  addDataEndpoint('get', '/api/displays', async (req, res) => {
    res.status(200).json('dummy displays...');
  });

  addDataEndpoint('put', '/api/displays/:displayId', async (req, res) => {
    const displayId = req.params?.displayId;
    const displayProps = {
      name: req.body?.name,
      brightness: req.body?.brightness,
    };
    res.status(200).json({
      displayId,
      displayProps,
    });
  });

  addDataEndpoint('put', '/api/darkMode', async (req, res) => {
    const isDarkModeOn = req.body?.darkMode === true;

    res.status(200).json({
      isDarkModeOn,
    });
  });
}
