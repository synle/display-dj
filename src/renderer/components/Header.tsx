import Backdrop from '@mui/material/Backdrop';
import Typography from '@mui/material/Typography';
import { useState } from 'react';
import { Loading } from 'src/renderer/components/Loading';
import { ToggleAllDisplay } from 'src/renderer/components/ToggleAllDisplay';
import { useRefetchConfigs, useRefetchPreferences } from 'src/renderer/hooks';
import { AppConfig, Preference } from 'src/types.d';

type HeaderProps = {
  configs?: AppConfig;
  preference?: Preference;
};

export function Header(props: HeaderProps) {
  const { configs, preference } = props;
  const [disabled, setDisabled] = useState(false);

  const refetchConfigs = useRefetchConfigs();
  const refetchPreferences = useRefetchPreferences();

  const onRefresh = async () => {
    setDisabled(true);
    await Promise.all([refetchConfigs(), refetchPreferences()]);
    setDisabled(false);
  };

  return (
    <>
      <header>
        <Typography variant='h5' className='flexAlignItems' onClick={onRefresh}>
          <strong>
            Display-DJ {configs?.version} {configs?.env !== 'production' ? configs?.env : ''}
          </strong>
        </Typography>
        {!preference ? null : <ToggleAllDisplay preference={preference} />}
      </header>
      {/*backdrop*/}
      <Backdrop
        sx={{
          background: (theme) => theme.palette.background.paper,
          color: (theme) => theme.palette.text.primary,
          zIndex: (theme) => theme.zIndex.drawer + 1,
        }}
        open={disabled}>
        <Typography variant='h6' className='flexAlignItems'>
          <Loading style={{ marginRight: '10px' }} />
          <strong>Refreshing...</strong>
        </Typography>
      </Backdrop>
    </>
  );
}
