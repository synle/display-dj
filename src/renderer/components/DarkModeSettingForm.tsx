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
import { MonitorBrightnessSettingForm } from 'src/renderer/components/MonitorBrightnessSettingForm';
import { MonitorBrightnessSetting } from 'src/renderer/components/MonitorBrightnessSetting';
import { AllMonitorBrightnessSetting } from 'src/renderer/components/AllMonitorBrightnessSetting';
import { MonitorNameInput } from 'src/renderer/components/MonitorNameInput';
import { VolumeSetting } from 'src/renderer/components/VolumeSetting';
import { VolumeIcon } from 'src/renderer/components/VolumeIcon';
import { Slider } from 'src/renderer/components/Slider';

type DarkModeSettingFormProps = {
  darkMode: boolean;
};
export function DarkModeSettingForm(props: DarkModeSettingFormProps) {
  const darkModeFromProps = props.darkMode === true;
  const [darkMode, setDarkMode] = useState<boolean>(darkModeFromProps);
  const { mutateAsync: updateDarkMode } = useToggleDarkMode();

  const onToggleDarkMode = async () => {
    const newDarkMode = !darkMode;

    setDarkMode(newDarkMode);

    await updateDarkMode(newDarkMode);
  };

  useEffect(() => {
    setDarkMode(darkModeFromProps);
  }, [darkModeFromProps]);

  return (
    <button className='btnToggleDarkMode' onClick={onToggleDarkMode} title='Toggle Dark Mode'>
      {darkMode ? (
        <>
          <DarkModeSvg /> <span>Mode: Dark</span>
        </>
      ) : (
        <>
          <LightModeSvg />
          <span>Mode: Light</span>
        </>
      )}
    </button>
  );
}