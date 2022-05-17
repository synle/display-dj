import { useMemo } from 'react';
import { VolumeSetting } from 'src/renderer/components/VolumeSetting';
import { Volume } from 'src/types.d';
import VolumeUpIcon from '@mui/icons-material/VolumeUp';
import VolumeMuteIcon from '@mui/icons-material/VolumeMute';

type VolumeSettingProps = {
  volume: Volume;
};

export function VolumeIcon(props: VolumeSettingProps) {
  const { volume } = props;

  const icon = useMemo(() => {
    if (volume.muted) {
      return <VolumeMuteIcon />
    }

    const { value } = volume;
    // if (value < 30) {
    //   return <VolumeUpIcon />;
    // }
    // if (value < 60) {
    //   return <VolumeUpIcon />;
    // }
    return <VolumeUpIcon />;
  }, [volume.muted, volume.value]);

  return <>{icon}</>;
}
