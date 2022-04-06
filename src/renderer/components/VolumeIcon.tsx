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
import { VolumeSetting } from 'src/renderer/components/VolumeSetting';
import { Slider } from 'src/renderer/components/Slider';

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
