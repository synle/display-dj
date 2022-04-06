import ReactDOM from 'react-dom';
import { QueryClient, QueryClientProvider } from 'react-query';
import React, { useEffect, useMemo, useState } from 'react';
import { LAPTOP_BUILT_IN_DISPLAY_ID } from 'src/constants';
import MonitorSvg from 'src/renderer/svg/monitor.svg';
import LaptopSvg from 'src/renderer/svg/laptop.svg';
import ToggleSvg from 'src/renderer/svg/toggle.svg';
import DarkModeSvg from 'src/renderer/svg/darkMode.svg';
import LightModeSvg from 'src/renderer/svg/lightMode.svg';
import { Monitor, SingleMonitorUpdateInput, Preference, AppConfig, Volume } from 'src/types.d';
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
import { debounce } from 'src/renderer/utils/CommonUtils';
import { Header } from 'src/renderer/components/Header';
import { ToggleAllDisplay } from 'src/renderer/components/ToggleAllDisplay';
import { DarkModeSettingForm } from 'src/renderer/components/DarkModeSettingForm';
import { MonitorBrightnessSettingForm } from 'src/renderer/components/MonitorBrightnessSettingForm';
import { MonitorBrightnessSetting } from 'src/renderer/components/MonitorBrightnessSetting';
import { AllMonitorBrightnessSetting } from 'src/renderer/components/AllMonitorBrightnessSetting';
import { MonitorNameInput } from 'src/renderer/components/MonitorNameInput';
import { VolumeIcon } from 'src/renderer/components/VolumeIcon';
import { Slider } from 'src/renderer/components/Slider';

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
    const newVolume ={
      ...volume,
      muted: !volume.muted,
    }

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
