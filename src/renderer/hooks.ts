import { QueryClient, useMutation, useQuery, useQueryClient } from 'react-query';
import ApiUtils from 'src/renderer/utils/ApiUtils';
import { Monitor, UIAppState, VolumeInput } from 'src/types.d';
import { LAPTOP_BUILT_IN_DISPLAY_ID } from 'src/constants';
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
      onSuccess: () => queryClient.invalidateQueries(QUERY_KEY_CONFIGS),
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
    onSuccess: () => queryClient.invalidateQueries(QUERY_KEY_PREFERENCE),
  });
}
// configs
export function useConfigs() {
  return useQuery(QUERY_KEY_CONFIGS, ApiUtils.getConfigs);
}

export function useUpdateMonitor() {
  const queryClient = useQueryClient();
  return useMutation(ApiUtils.updateMonitor, {
    onSuccess: () => queryClient.invalidateQueries(QUERY_KEY_CONFIGS),
  });
}

export function useBatchUpdateMonitors() {
  const queryClient = useQueryClient();
  return useMutation(ApiUtils.batchUpdateMonitors, {
    onSuccess: () => queryClient.invalidateQueries(QUERY_KEY_CONFIGS),
  });
}

export function useToggleDarkMode() {
  const queryClient = useQueryClient();
  return useMutation(ApiUtils.updateDarkMode, {
    onSuccess: () => queryClient.invalidateQueries(QUERY_KEY_CONFIGS),
  });
}

export function useUpdateVolume() {
  const queryClient = useQueryClient();
  return useMutation<void, void, VolumeInput>(ApiUtils.updateVolume, {
    onSuccess: () => queryClient.invalidateQueries(QUERY_KEY_CONFIGS),
  });
}

// misc
export function useUpdateAppPosition() {
  return useMutation(ApiUtils.updateAppPosition);
}