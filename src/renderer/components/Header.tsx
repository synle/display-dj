import Typography from '@mui/material/Typography';
import { ToggleAllDisplay } from 'src/renderer/components/ToggleAllDisplay';
import { useRefetchConfigs, useRefetchPreferences } from 'src/renderer/hooks';
import { AppConfig, Preference } from 'src/types.d';

type HeaderProps = {
  configs?: AppConfig;
  preference?: Preference;
};

export function Header(props: HeaderProps) {
  const { configs, preference } = props;

  const refetchConfigs = useRefetchConfigs();
  const refetchPreferences = useRefetchPreferences();

  const onRefresh = () => {
    refetchConfigs();
    refetchPreferences();
  };

  return (
    <header>
      <Typography variant='h5' className='flexAlignItems' onClick={onRefresh}>
        <strong>
          Display-DJ {configs?.version} {configs?.env !== 'production' ? configs?.env : ''}
        </strong>
      </Typography>
      {!preference ? null : <ToggleAllDisplay preference={preference} />}
    </header>
  );
}
