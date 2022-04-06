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
import { VolumeIcon } from 'src/renderer/components/VolumeIcon';
import { Slider } from 'src/renderer/components/Slider';

// react components
type HomeProps = {};
export function Home(props: HomeProps) {
  const { isLoading: loadingConfigs, data: configs, refetch } = useConfigs();
  const { isLoading: loadingPrefs, data: preference } = usePreferences();
  const { mutateAsync: updateAppPosition } = useUpdateAppPosition();

  useEffect(() => {
    const config = { childList: true, subtree: true };
    const callback = () => updateAppPosition();

    const observer = new MutationObserver(callback);
    observer.observe(document.body, config);

    // update position and refetched and the page is visible
    const onVisibilityChange = () => {
      updateAppPosition();
      refetch();
    };
    document.addEventListener('visibilitychange', onVisibilityChange);

    return () => {
      observer.disconnect();
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, []);

  const isLoading = loadingConfigs || loadingPrefs;
  if (isLoading) {
    return (
      <>
        <Header configs={configs} preference={preference} />
        <h3>Loading... Please wait...</h3>
      </>
    );
  }

  if (!configs || !preference) {
    // TODO: add message for no data state
    return (
      <>
        <Header configs={configs} preference={preference} />
        <h3>Errors... Failed to get data...</h3>
      </>
    );
  }

  return (
    <>
      <Header configs={configs} preference={preference} />
      {preference.showIndividualDisplays ? (
        <MonitorBrightnessSettingForm monitors={configs.monitors} />
      ) : (
        <AllMonitorBrightnessSetting monitors={configs.monitors} />
      )}
      {configs.volume.isDisabled === false && <VolumeSetting volume={configs.volume} />}
      <DarkModeSettingForm darkMode={configs.darkMode} />
    </>
  );
}
