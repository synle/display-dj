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
import { VolumeSetting } from 'src/renderer/components/VolumeSetting';
import { VolumeIcon } from 'src/renderer/components/VolumeIcon';
import { Slider } from 'src/renderer/components/Slider';

type MonitorBrightnessSettingProps = {
  monitor: Monitor;
  idx: number;
};
export function MonitorNameInput(props: MonitorBrightnessSettingProps) {
  const [mode, setMode] = useState<string>('mode/read');
  const [name, setName] = useState('');
  const { monitor } = props;
  const { mutateAsync: updateMonitor } = useUpdateMonitor();

  const isSavingName = mode === 'mode/saving';

  const onDisplayNameChange = async (e: React.SyntheticEvent) => {
    e.preventDefault();
    setMode('mode/saving');

    let nameToUse = (name || '').trim() || `Monitor #${props.idx}`;

    await updateMonitor({
      id: monitor.id,
      name: nameToUse,
    });

    setMode('mode/read');
  };

  useEffect(() => {
    setName(monitor.name);
  }, [monitor.name]);

  return mode !== 'mode/read' ? (
          <form onSubmit={onDisplayNameChange}>
            <input
              className='field__value'
              value={name}
              placeholder='Enter a display name'
              autoFocus={true}
              onInput={(e) => setName((e.target as HTMLInputElement).value)}
              onBlur={onDisplayNameChange}
              required
              disabled={isSavingName}
              type='text'
            />
          </form>
        ) : (
          <div className='field__value field__value-readonly'>
            <a onClick={() => setMode('mode/edit')} title='Monitor Name' href='#'>
              {monitor.name}
            </a>
          </div>
        )
}
