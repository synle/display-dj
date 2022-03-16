import './fetchPolyfill';

function _fetch(input, initOptions) {
  let { headers, ...restInput } = initOptions || {};

  headers = headers || {};
  headers = {
    ...headers,
    ...{
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
  };

  restInput = restInput || {};

  return fetch(input, {
    ...restInput,
    headers,
  }).then(async (r) => {
    const response = await r.text();

    let responseToUse;
    try {
      responseToUse = JSON.parse(response);
    } catch (err) {
      responseToUse = response;
    }

    return r.ok ? responseToUse : Promise.reject(responseToUse);
  });
}

const ApiUtils = {
  getMonitors: () => _fetch(`/api/monitors`),
  updateMonitor: (monitor) =>
    _fetch(`/api/monitors`, {
      method: 'put',
      body: JSON.stringify(monitor),
    }),
  getConfigs: () => _fetch(`/api/configs`),
  toggleDarkMode: (darkMode) =>
    _fetch(`/api/darkMode`, {
      method: 'put',
      body: JSON.stringify({
        darkMode,
      }),
    }),
};

export default ApiUtils;
