import { useEffect, useState } from 'react';
import { Slider } from 'src/renderer/components/Slider';
import { VolumeIcon } from 'src/renderer/components/VolumeIcon';
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

export function VolumeSetting(props: VolumeSettingProps) {
  const [volume, setVolume] = useState<Volume>(props.volume);

  const { mutateAsync: updateVolume } = useUpdateVolume();

  const onChange = async (newValue: number) => {
    const newVolume = {
      ...volume,
      value: newValue,
      muted: newValue === 0,
    };

    setVolume(newVolume);

    await updateVolume(newVolume);
  };

  const onSetMuted = async () => {
    const newVolume = {
      ...volume,
      muted: !volume.muted,
    };

    setVolume(newVolume);

    await updateVolume(newVolume);
  };

  useEffect(() => {
    if (JSON.stringify(volume) !== JSON.stringify(props.volume)) {
      setVolume(props.volume);
    }
  }, [props.volume]);

  return (
    <>
      <div className='field'>
        <div className='field__value field__value-readonly' title='Volume'>
          Volume
        </div>
      </div>
      <div className='field'>
        <span className='field__icon iconBtn' title='Toggle Muted' onClick={onSetMuted}>
          <VolumeIcon volume={volume} />
        </span>
        <Slider
          className='field__value'
          placeholder='Volume'
          value={volume.value}
          onInput={(e) => onChange(parseInt((e.target as HTMLInputElement).value) || 0)}
        />
      </div>
    </>
  );
}
