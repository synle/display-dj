import Backdrop from '@mui/material/Backdrop';
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
import {
  useConfigs,
  usePreferences,
  useRefetchConfigs,
  useRefetchPreferences,
  useUpdateAppPosition,
} from 'src/renderer/hooks';
import { Monitor, Volume } from 'src/types.d';

type HomeProps = {};

export function Home(props: HomeProps) {
  const { isLoading: loadingConfigs, data: configs } = useConfigs();
  const { isLoading: loadingPrefs, data: preference } = usePreferences();
  const { mutateAsync: updateAppPosition } = useUpdateAppPosition();

  const refetchConfigs = useRefetchConfigs();
  const refetchPreferences = useRefetchPreferences();

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
        <div style={{ height: '100px' }}></div>
        <Backdrop
          sx={{
            background: (theme) => theme.palette.background.paper,
            color: (theme) => theme.palette.text.primary,
            zIndex: (theme) => theme.zIndex.drawer + 1,
          }}
          open={true}>
          <Typography variant='h6' className='flexAlignItems'>
            <Loading style={{ marginRight: '10px' }} />
            Loading, please wait...
          </Typography>
        </Backdrop>
      </>
    );
  }

  if (!configs || !preference) {
    return null;
  }

  const isM1Mac = preference.mode === 'm1_mac';
  if(isM1Mac){
    return (
      <>
        <Header configs={configs} preference={preference} />
        <AllMonitorBrightnessSetting monitors={configs.monitors} />
        <DarkModeSettingForm darkMode={configs.darkMode} />
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
