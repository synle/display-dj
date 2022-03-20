import { QueryClient, useMutation, useQuery, useQueryClient } from 'react-query';
import ApiUtils from 'src/renderer/utils/ApiUtils';
import { LAPTOP_BUILT_IN_DISPLAY_ID } from 'src/constants';
import MonitorSvg from 'src/renderer/svg/monitor.svg';
import LaptopSvg from 'src/renderer/svg/laptop.svg';
import ToggleSvg from 'src/renderer/svg/toggle.svg';
import './index.scss';
import { Monitor, SingleMonitorUpdateInput } from 'src/types.d';

// react query store
export const QUERY_KEY_CONFIGS = 'configs';

export const QUERY_KEY_APP_STATE = 'appState';
export const QUERY_KEY_PREFERENCE = 'preferences';

// app state (not used anymore, but left as is if we want to add new global state to the app)
type AppState = {};

let _appState: AppState = {};

export function useAppState() {
  return useQuery(QUERY_KEY_APP_STATE, () => _appState);
}

export function useUpdateAppState() {
  const queryClient = useQueryClient();

  return useMutation<void, void, AppState>(
    async (newAppState: AppState) => {
      _appState = { ..._appState, ...newAppState };
    },
    {
      onSuccess: () => {
        queryClient.invalidateQueries(QUERY_KEY_CONFIGS);
      },
    },
  );
}

// preference
export function usePreferences() {
  return useQuery(QUERY_KEY_PREFERENCE, ApiUtils.getPreferences);
}

export function useUpdatePreferences() {
  const queryClient = useQueryClient();

  return useMutation(ApiUtils.updatePreferences, {
    onSuccess: () => {
      queryClient.invalidateQueries(QUERY_KEY_PREFERENCE);
    },
  });
}
// configs
export function useConfigs() {
  return useQuery(QUERY_KEY_CONFIGS, ApiUtils.getConfigs);
}

export function useUpdateMonitor() {
  const queryClient = useQueryClient();
  return useMutation(ApiUtils.updateMonitor, {
    onSuccess: () => {
      queryClient.invalidateQueries(QUERY_KEY_CONFIGS);
    },
  });
}
export function useBatchUpdateMonitors() {
  const queryClient = useQueryClient();
  return useMutation(ApiUtils.batchUpdateMonitors, {
    onSuccess: () => {
      queryClient.invalidateQueries(QUERY_KEY_CONFIGS);
    },
  });
}

export function useToggleDarkMode() {
  const queryClient = useQueryClient();
  return useMutation(ApiUtils.updateDarkMode, {
    onSuccess: () => {
      queryClient.invalidateQueries(QUERY_KEY_CONFIGS);
    },
  });
}
// misc
export function useUpdateAppPosition() {
  return useMutation(ApiUtils.updateAppPosition);
}
