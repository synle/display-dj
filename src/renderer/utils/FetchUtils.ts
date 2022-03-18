import { ipcRenderer } from 'electron';

const origFetch = window.fetch;
function _doFetch(input: string, options: RequestInit) {
  const url = input as string;
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
        options?.method || 'get',
        url,
        returnedData,
        headers,
      );

      resolve({
        ok,
        text: () => text,
        headers,
      } as Response);
    });
    ipcRenderer.send('mainAppEvent/fetch', { requestId, url, options });
  });
}

export function fetch<T>(input: string, initOptions?: RequestInit): Promise<T> {
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
  initOptions = {
    ...restInput,
    headers,
  };

  return _doFetch(input, initOptions)
    .then(async (resp: any) => {
      const r = resp as Response;
      const response = await r.text();

      let responseToUse;
      try {
        responseToUse = JSON.parse(response);
      } catch (err) {
        responseToUse = response;
      }

      return r.ok ? responseToUse : Promise.reject(responseToUse);
    })
    .then((r: any) => {
      const res: T = r;
      return res;
    });
}
