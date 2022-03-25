import { fetch } from 'src/renderer/utils/FetchUtils';
import {
  AppConfig,
  SingleMonitorUpdateInput,
  BatchMonitorUpdateInput,
  Preference,
} from 'src/types.d';

const ApiUtils = {
  // preferences
  getPreferences: () => fetch<Preference>(`/api/preferences`),
  updatePreferences: (preference: Preference) =>
    fetch(`/api/preferences`, {
      method: 'put',
      body: JSON.stringify(preference),
    }),

  // configs
  getConfigs: () => fetch<AppConfig>(`/api/configs`),
  updateMonitorSortOrder: (sortOrders: number[]) =>{
    const [fromIdx, toIdx] = sortOrders;
      return fetch<void>(`/api/configs/monitors`, {
        method: 'put',
        body: JSON.stringify({
          fromIdx,
          toIdx
        }),
      })},
  updateMonitor: (monitor: SingleMonitorUpdateInput) =>
    fetch(`/api/configs/monitors/${monitor.id}`, {
      method: 'put',
      body: JSON.stringify(monitor),
    }),
  batchUpdateMonitors: (monitor: BatchMonitorUpdateInput) =>
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
