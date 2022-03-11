// @ts-nocheck
import { ipcRenderer } from 'electron';

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
        options.headers['api-call-session-id'],
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
