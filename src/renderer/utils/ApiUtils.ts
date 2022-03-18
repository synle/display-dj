import { fetch } from 'src/renderer/utils/FetchUtils';
import { AppConfig, MonitorUpdateInput } from 'src/types.d';

const ApiUtils = {
  getConfigs: () => fetch<AppConfig>(`/api/configs`),
  updateMonitor: (monitor: MonitorUpdateInput) =>
    fetch(`/api/configs/monitors`, {
      method: 'put',
      body: JSON.stringify(monitor),
    }),
  updateDarkMode: (darkMode: boolean) =>
    fetch(`/api/configs/darkMode`, {
      method: 'put',
      body: JSON.stringify({
        darkMode,
      }),
    }),
  updateAppPosition: () =>
    fetch(`/api/configs/appPosition`, {
      method: 'put',
      body: JSON.stringify({
        height: document.body.clientHeight + 20,
      }),
    }),
};

export default ApiUtils;
