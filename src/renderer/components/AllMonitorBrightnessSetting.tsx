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
import { MonitorNameInput } from 'src/renderer/components/MonitorNameInput';
import { VolumeSetting } from 'src/renderer/components/VolumeSetting';
import { VolumeIcon } from 'src/renderer/components/VolumeIcon';
import { Slider } from 'src/renderer/components/Slider';

type AllMonitorBrightnessSettingProps = {
  monitors: Monitor[];
};
export function AllMonitorBrightnessSetting(props: AllMonitorBrightnessSettingProps) {
  const { monitors } = props;
  const allBrightnessValueFromProps = Math.min(
    ...monitors.map((monitor) => monitor.brightness),
    100,
  );
  const [allBrightness, setAllBrightness] = useState(allBrightnessValueFromProps);
  const { mutateAsync: batchUpdateMonitors } = useBatchUpdateMonitors();

  const onChange = async (brightness: number) => {
    setAllBrightness(brightness);
    await batchUpdateMonitors({ brightness });
  };

  return (
    <>
      <div className='field'>
        <div className='field__value field__value-readonly' title='All Monitors'>
          All Monitors ({monitors.length})
        </div>
      </div>
      <div className='field'>
        <span className='field__icon' title='All Monitor Brightness'>
          🔆
        </span>
        <Slider
          className='field__value'
          placeholder='brightness'
          value={allBrightness}
          onInput={(e) => onChange(parseInt((e.target as HTMLInputElement).value) || 0)}
        />
      </div>
    </>
  );
}