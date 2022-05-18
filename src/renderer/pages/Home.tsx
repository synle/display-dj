import Typography from '@mui/material/Typography';
import { ipcRenderer } from 'electron';
import { useEffect } from 'react';
import { AllMonitorBrightnessSetting } from 'src/renderer/components/AllMonitorBrightnessSetting';
import { DarkModeSettingForm } from 'src/renderer/components/DarkModeSettingForm';
import { Header } from 'src/renderer/components/Header';
import { Loading } from 'src/renderer/components/Loading';
import { MonitorBrightnessSetting } from 'src/renderer/components/MonitorBrightnessSetting';
import { MonitorBrightnessSettingForm } from 'src/renderer/components/MonitorBrightnessSettingForm';
import { VolumeSetting } from 'src/renderer/components/VolumeSetting';
import { useConfigs, usePreferences, useUpdateAppPosition } from 'src/renderer/hooks';
import { Monitor, Volume } from 'src/types.d';
import Alert from '@mui/material/Alert';
import Backdrop from '@mui/material/Backdrop';
import CircularProgress from '@mui/material/CircularProgress';

type HomeProps = {};

export function Home(props: HomeProps) {
  const { isLoading: loadingConfigs, data: configs, refetch: refetchConfigs } = useConfigs();
  const {
    isLoading: loadingPrefs,
    data: preference,
    refetch: refetchPreferences,
  } = usePreferences();
  const { mutateAsync: updateAppPosition } = useUpdateAppPosition();

  useEffect(() => {
    const config = { childList: true, subtree: true };
    const callback = () => updateAppPosition();

    const observer = new MutationObserver(callback);
    observer.observe(document.body, config);

    // update position and refetched and the page is visible
    const _onVisibilityChange = () => {
      if (document.visibilityState !== 'visible') {
        // if the dom is visible, then let's position and update configs
        updateAppPosition();
        _onRefetch(null, { type: 'all' });
      }
    };
    document.addEventListener('visibilitychange', _onVisibilityChange);

    // update states
    const _onRefetch = (e: any, msg: any) => {
      console.log('[ipcRenderer] [Event] mainAppEvent/refetch', msg);

      switch (msg.type) {
        case 'configs':
          refetchConfigs();
          break;
        case 'preferences':
          refetchPreferences();
          break;
        case 'all':
        default:
          refetchConfigs();
          refetchPreferences();
          break;
      }
    };
    ipcRenderer.on('mainAppEvent/refetch', _onRefetch);
    return () => {
      observer.disconnect();
      document.removeEventListener('visibilitychange', _onVisibilityChange);
      ipcRenderer.removeListener('mainAppEvent/refetch', _onRefetch);
    };
  }, []);

  const isLoading = loadingConfigs || loadingPrefs;
  if (isLoading) {
    return (
      <>
        <Backdrop
          sx={{ zIndex: (theme) => theme.zIndex.drawer + 1 }}
          open={true}
        >
          <Typography variant='subtitle1' className='flexAlignItems'>
            <Loading style={{ marginRight: '10px' }} />
            <strong>Loading, please wait...</strong>
          </Typography>
        </Backdrop>
      </>
    );
  }

  if (!configs || !preference) {
    return null;
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
