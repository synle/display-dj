import { useMemo } from 'react';
import { VolumeSetting } from 'src/renderer/components/VolumeSetting';
import { Volume } from 'src/types.d';

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
