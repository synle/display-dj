// @ts-nocheck
const { ipcRenderer } = require('electron');

const origFetch = window.fetch;
window.fetch = (url, options) => {
  if (url.indexOf('/api') !== 0) {
    // if not /api/, then use the original fetch
    return origFetch(url, options);
  }

  options = options || {};
  options.headers = options.headers || {};

  return new Promise((resolve, reject) => {
    const requestId = `fetch.requestId.${Date.now()}.${Math.floor(
      Math.random() * 10000000000000000,
    )}`;
    ipcRenderer.once(requestId, (event, data) => {
      const { ok, text, status, headers } = data;

      let returnedData = text;

      try {
        returnedData = JSON.parse(text);
      } catch (err) {}

      console.log(
        '>> Network',
        ok ? 'Success' : 'Error:',
        status,
        options.method || 'get',
        url,
        returnedData,
        headers,
      );

      resolve({
        ok,
        text: () => text,
        headers,
      });
    });
    ipcRenderer.send('mainAppEvent/fetch', { requestId, url, options });
  });
};

function _fetch<T>(input: RequestInfo, initOptions?: RequestInit) {
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
  })
    .then(async (r) => {
      const response = await r.text();

      let responseToUse;
      try {
        responseToUse = JSON.parse(response);
      } catch (err) {
        responseToUse = response;
      }

      return r.ok ? responseToUse : Promise.reject(responseToUse);
    })
    .then((r) => {
      const res: T = r;
      return res;
    });
}

window.ApiUtils = {
  getMonitors: () => _fetch(`/api/monitors`),
  updateMonitor: (monitor) =>
    _fetch(`/api/monitors/${monitor.id}`, {
      method: 'post',
      body: JSON.stringify({
        id: monitor.id,
        name: monitor.name,
        brightness: monitor.brightness,
      }),
    }),
  toggleDarkMode: (darkMode: boolean) =>
    _fetch(`/api/darkMode`, {
      method: 'put',
      body: JSON.stringify({
        darkMode,
      }),
    }),
};

ApiUtils.getMonitors().then(console.log);
ApiUtils.toggleDarkMode().then(console.log);
ApiUtils.toggleDarkMode(true).then(console.log);
