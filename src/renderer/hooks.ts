import { QueryClient, useMutation, useQuery, useQueryClient } from 'react-query';
import ApiUtils from 'src/renderer/utils/ApiUtils';
import { AppConfig, Monitor, Preference, UIAppState, VolumeInput } from 'src/types.d';
// react query store
export const QUERY_KEY_CONFIGS = 'configs';

export const QUERY_KEY_APP_STATE = 'appState';

export const QUERY_KEY_PREFERENCE = 'preferences';

// app state (not used anymore, but left as is if we want to add new global state to the app)
let _appState: UIAppState;
let _config : AppConfig;
let _preferences : Preference;

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
  return useQuery(QUERY_KEY_PREFERENCE, async () => {
    if(_preferences === undefined){
      _preferences = await ApiUtils.getPreferences();
    }
    return _preferences;
  });
}

export function useUpdatePreferences() {
  const refetchPreferences = useRefetchPreferences();
  return useMutation(ApiUtils.updatePreferences, {
    onSuccess: async () => {
      await refetchPreferences();
    }
  });
}

// configs
export function useConfigs() {
  return useQuery(QUERY_KEY_CONFIGS, async () => {
    if(_config === undefined){
      _config = await ApiUtils.getConfigs();
    }
    return _config;
  });
}

export function useUpdateMonitor() {
  const refetchConfigs = useRefetchConfigs();
  return useMutation(ApiUtils.updateMonitor, {
    onSuccess: async () => {
      await refetchConfigs();
    }
  });
}

export function useBatchUpdateMonitors() {
  const refetchConfigs = useRefetchConfigs();
  return useMutation(ApiUtils.batchUpdateMonitors, {
    onSuccess: async () => {
      await refetchConfigs();
    }
  });
}

export function useToggleDarkMode() {
  const refetchConfigs = useRefetchConfigs();
  return useMutation(ApiUtils.updateDarkMode, {
    onSuccess: async () => {
      await refetchConfigs();
    }
  });
}

export function useUpdateVolume() {
  const refetchConfigs = useRefetchConfigs();
  return useMutation<void, void, VolumeInput>(ApiUtils.updateVolume, {
    onSuccess: async () => {
      await refetchConfigs();
    }
  });
}

// misc
export function useUpdateAppPosition() {
  return useMutation(ApiUtils.updateAppPosition);
}

// refetch
export function useRefetchConfigs(){
  const queryClient = useQueryClient();
  return async () => {
    console.log('>> Refetch Configs')
    _config = await ApiUtils.getConfigs();
    queryClient.invalidateQueries(QUERY_KEY_CONFIGS)
    queryClient.setQueryData(QUERY_KEY_CONFIGS, _config);
  };
}

export function useRefetchPreferences(){
  const queryClient = useQueryClient();
  return async () => {
    console.log('>> Refetch preferences')
    _preferences = await ApiUtils.getPreferences();
    queryClient.invalidateQueries(QUERY_KEY_PREFERENCE)
    queryClient.setQueryData(QUERY_KEY_PREFERENCE, _preferences);
  };
}