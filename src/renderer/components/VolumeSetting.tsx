import { useEffect, useState } from 'react';
import { Slider } from 'src/renderer/components/Slider';
import { VolumeIcon } from 'src/renderer/components/VolumeIcon';
import { useUpdateVolume } from 'src/renderer/hooks';
import { Volume } from 'src/types.d';

type VolumeSettingProps = {
  volume: Volume;
};

export function VolumeSetting(props: VolumeSettingProps) {
  const [volume, setVolume] = useState<Volume>(props.volume);
  const [disabled, setDisabled] = useState(false);

  const { mutateAsync: updateVolume } = useUpdateVolume();

  const onChange = async (newValue: number) => {
    const newVolume = {
      ...volume,
      value: newValue,
      muted: newValue === 0,
    };

    setVolume(newVolume);

    setDisabled(true);
    await updateVolume(newVolume);
    setDisabled(false);
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
    setDisabled(false);
  }, [props.volume]);

  return (
    <>
      <div className='field'>
        <span className='field__icon field__button' title='Toggle Muted' onClick={onSetMuted}>
          <VolumeIcon volume={volume} />
        </span>
        <span className='field__slider'>
          <Slider
            className='field__value'
            placeholder='Volume'
            value={volume.value}
            onInput={onChange}
            disabled={disabled}
          />
        </span>
      </div>
    </>
  );
}
