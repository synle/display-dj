import { useMemo } from 'react';
import { VolumeSetting } from 'src/renderer/components/VolumeSetting';
import { Volume } from 'src/types.d';
import { LAPTOP_BUILT_IN_DISPLAY_ID } from 'src/constants';
import {
  useBatchUpdateMonitors,
  usePreferences,
  useUpdatePreferences,
  useConfigs,
  useUpdateMonitor,
  useToggleDarkMode,
  useUpdateAppPosition,
  useUpdateVolume,
  QUERY_KEY_CONFIGS,
  QUERY_KEY_APP_STATE,
} from 'src/renderer/hooks';

type VolumeSettingProps = {
  volume: Volume;
};

export function VolumeIcon(props: VolumeSettingProps) {
  const { volume } = props;

  const icon = useMemo(() => {
    if (volume.muted) {
      return '🔇';
    }

    const { value } = volume;
    if (value < 30) {
      return '🔈';
    }
    if (value < 60) {
      return '🔉';
    }
    return '🔊';
  }, [volume.muted, volume.value]);

  return <>{icon}</>;
}
