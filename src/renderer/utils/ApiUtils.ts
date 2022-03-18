import 'src/renderer/utils/fetchPolyfill';
import { Monitor, MonitorUpdateInput } from 'src/types.d';


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
  }).then(async (r) => {
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

const ApiUtils = {
  getConfigs: () => _fetch<Monitor[]>(`/api/configs`),
  updateMonitor: (monitor: MonitorUpdateInput) =>
    _fetch(`/api/configs/monitors`, {
      method: 'put',
      body: JSON.stringify(monitor),
    }),
  updateDarkMode: (darkMode: boolean) =>
    _fetch(`/api/configs/darkMode`, {
      method: 'put',
      body: JSON.stringify({
        darkMode,
      }),
    }),
  updateAppHeight: () =>
    _fetch(`/api/configs/positionWindow`, {
      method: 'put',
      body: JSON.stringify({
        height: document.body.clientHeight + 20,
      }),
    }),
};

export default ApiUtils;
