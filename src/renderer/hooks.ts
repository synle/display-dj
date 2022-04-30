import { QueryClient, useMutation, useQuery, useQueryClient } from 'react-query';
import ApiUtils from 'src/renderer/utils/ApiUtils';
import { Monitor, UIAppState, VolumeInput } from 'src/types.d';
// react query store
export const QUERY_KEY_CONFIGS = 'configs';

export const QUERY_KEY_APP_STATE = 'appState';

export const QUERY_KEY_PREFERENCE = 'preferences';

// app state (not used anymore, but left as is if we want to add new global state to the app)
let _appState: UIAppState = {};

export function useAppState() {
  return useQuery(QUERY_KEY_APP_STATE, () => _appState);
}

export function useUpdateAppState() {
  const queryClient = useQueryClient();

  return useMutation<void, void, UIAppState>(
    async (newAppState: UIAppState) => {
      _appState = { ..._appState, ...newAppState };
    },
    {
      onSuccess: () => queryClient.invalidateQueries(QUERY_KEY_APP_STATE),
    },
  );
}

// preference
export function usePreferences() {
  return useQuery(QUERY_KEY_PREFERENCE, ApiUtils.getPreferences);
}

export function useUpdatePreferences() {
  return useMutation(ApiUtils.updatePreferences);
}

// configs
export function useConfigs() {
  return useQuery(QUERY_KEY_CONFIGS, ApiUtils.getConfigs);
}

export function useUpdateMonitor() {
  return useMutation(ApiUtils.updateMonitor);
}

export function useBatchUpdateMonitors() {
  return useMutation(ApiUtils.batchUpdateMonitors);
}

export function useToggleDarkMode() {
  return useMutation(ApiUtils.updateDarkMode);
}

export function useUpdateVolume() {
  return useMutation<void, void, VolumeInput>(ApiUtils.updateVolume);
}

// misc
export function useUpdateAppPosition() {
  return useMutation(ApiUtils.updateAppPosition);
}
