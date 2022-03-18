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
  getConfigs: () => _fetch(`/api/configs`),
  updateMonitor: (monitor) =>
    _fetch(`/api/configs/monitors`, {
      method: 'put',
      body: JSON.stringify(monitor),
    }),
  updateDarkMode: (darkMode) =>
    _fetch(`/api/configs/darkMode`, {
      method: 'put',
      body: JSON.stringify({
        darkMode,
      }),
    }),
  updateAppHeight: () =>
    _fetch(`/api/configs/appHeight`, {
      method: 'put',
      body: JSON.stringify({
        height: document.body.clientHeight,
      }),
    }),
};

export default ApiUtils;
