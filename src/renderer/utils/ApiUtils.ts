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
    fetch<void>(`/api/preferences`, {
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
    fetch<void>(`/api/configs/monitors/${monitor.id}`, {
      method: 'put',
      body: JSON.stringify(monitor),
    }),
  batchUpdateMonitors: (monitor: BatchMonitorUpdateInput) =>
    fetch<void>(`/api/configs/monitors`, {
      method: 'put',
      body: JSON.stringify(monitor),
    }),
  updateDarkMode: (darkMode: boolean) =>
    fetch<void>(`/api/configs/darkMode`, {
      method: 'put',
      body: JSON.stringify({
        darkMode,
      }),
    }),
  updateAppPosition: () =>
    fetch<void>(`/api/configs/appPosition`, {
      method: 'put',
      body: JSON.stringify({
        height: document.body.clientHeight + 20,
      }),
    }),
};

export default ApiUtils;
