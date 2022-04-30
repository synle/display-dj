import { useEffect } from 'react';
import { AllMonitorBrightnessSetting } from 'src/renderer/components/AllMonitorBrightnessSetting';
import { DarkModeSettingForm } from 'src/renderer/components/DarkModeSettingForm';
import { Header } from 'src/renderer/components/Header';
import { MonitorBrightnessSetting } from 'src/renderer/components/MonitorBrightnessSetting';
import { MonitorBrightnessSettingForm } from 'src/renderer/components/MonitorBrightnessSettingForm';
import { VolumeSetting } from 'src/renderer/components/VolumeSetting';
import { useConfigs, usePreferences, useUpdateAppPosition } from 'src/renderer/hooks';
import { Monitor, Volume } from 'src/types.d';
import { ipcRenderer } from 'electron';

// react components
type HomeProps = {};

export function Home(props: HomeProps) {
  const { isLoading: loadingConfigs, data: configs, refetch: refetchConfigs } = useConfigs();
  const { isLoading: loadingPrefs, data: preference, refetch: refetchPreferences } = usePreferences();
  const { mutateAsync: updateAppPosition } = useUpdateAppPosition();

  useEffect(() => {
    const config = { childList: true, subtree: true };
    const callback = () => updateAppPosition();

    const observer = new MutationObserver(callback);
    observer.observe(document.body, config);

    // update position and refetched and the page is visible
    const onVisibilityChange = (e: any) => {
      if (document.visibilityState !== 'visible') {
        // if the dom is visible, then let's position and update configs
        updateAppPosition();
        refetchConfigs();
        refetchPreferences();
      }
    };
    document.addEventListener('visibilitychange', onVisibilityChange);

    // update states
    ipcRenderer.on('mainAppEvent/refetch', function () {
      refetchConfigs();
      refetchPreferences();
    });


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
